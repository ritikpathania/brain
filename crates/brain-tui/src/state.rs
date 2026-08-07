use crate::ui::interaction::MessageId;
use brain_domain::SessionId;
use serde::{Deserialize, Serialize};
use std::cmp::{max, min};
use std::collections::VecDeque;
use std::time::{Instant, SystemTime};

/// Active rendering connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Connected to UDS daemon.
    Daemon,
    /// Direct in-process engine.
    Embedded,
    /// Disconnected state.
    Disconnected,
    /// Attempting to establish socket hook.
    Connecting,
}

/// Viewport and scroll offset parameters.
pub struct ViewportState {
    /// Monotonic scroll offset position.
    pub scroll_offset: usize,
    /// Lock scroll to follow incoming responses tail.
    pub follow_tail: bool,
    /// Flag indicating whether command selection modal overlay is open.
    pub is_command_palette_open: bool,
}

/// Extensible, semantic presentation token representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderToken {
    /// Standard semantic text block.
    Text(String),
    /// Formatting code snippet block.
    Code(String),
}

/// Lifecycle state machine of the query response generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationState {
    /// Ready to accept new user prompts.
    Idle,
    /// Generation request submitted, waiting for initial packets.
    Starting,
    /// Stream is active and typewriter animation is progression.
    Streaming {
        /// System time when the streaming session started.
        started_at: SystemTime,
    },
    /// Generation completed successfully.
    Finished,
    /// Generation was explicitly cancelled by the user.
    Cancelled(Option<String>),
    /// Generation terminated due to a transport or model error.
    Error(String),
}

/// Focusable input panel partitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    /// Focus inside the editor prompt input buffer.
    Editor,
    /// Focus inside the sidebar session listing browser.
    Sidebar,
    /// Focus inside the chat timeline references.
    Timeline,
    /// Focus inside the active knowledge inspector panel.
    Inspector,
}

/// Monotonic identifier tracking asynchronous session load invocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LoadRequestId(pub u64);

/// Grouped value object representing an uncompleted database query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLoad {
    /// Unique target session.
    pub session_id: SessionId,
    /// Unique target request version.
    pub request_id: LoadRequestId,
}

/// State machine tracking lazy message loading.
#[derive(Debug, Clone)]
pub enum SessionLoadState {
    /// No lazy load is currently active.
    NotLoaded,
    /// Active asynchronous load running.
    Loading,
    /// Lazy load completed successfully.
    Loaded(Vec<brain_domain::Message>),
    /// Lazy load failed with diagnostic error details.
    Error(String),
}

impl PartialEq for SessionLoadState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NotLoaded, Self::NotLoaded) => true,
            (Self::Loading, Self::Loading) => true,
            (Self::Loaded(_), Self::Loaded(_)) => true,
            (Self::Error(e1), Self::Error(e2)) => e1 == e2,
            _ => false,
        }
    }
}

impl Eq for SessionLoadState {}

/// Client-facing presentation view model representing a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionViewModel {
    /// Unique identifier.
    pub id: SessionId,
    /// Descriptive title.
    pub title: String,
    /// Time of last update checkpoint.
    pub updated_at: SystemTime,
    /// Is this session the active session.
    pub active: bool,
    /// Optional text preview summary of the final thread messages.
    pub preview: Option<String>,
    /// Whether the session is pinned.
    pub pinned: bool,
    /// Whether the session is archived.
    pub archived: bool,
}

/// Individual visible row in a virtual scroll viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    /// Monotonic index of the row.
    pub index: usize,
    /// String content of the row.
    pub content: String,
    /// Whether the row is highlighted.
    pub is_highlighted: bool,
}

/// Sliced viewport presentation model for virtual scrolling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationModel {
    /// Vector of rows currently visible within the viewport.
    pub visible_rows: Vec<VisibleRow>,
    /// Total count of rows in the dataset.
    pub total_rows: usize,
    /// Current scroll offset position.
    pub scroll_offset: usize,
    /// Height of the visible viewport.
    pub viewport_height: usize,
    /// Formatted scroll indicator text.
    pub scroll_indicator: String,
}

/// Semantic outcome of a typewriter queue tick drain cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrainResult {
    /// Newly emitted tokens to append to the presentation screen.
    pub emitted: Vec<RenderToken>,
    /// Indication that the queue has finished draining all buffered tokens.
    pub finished: bool,
}

/// Bounded pacing queue for typewriter-style token animations.
pub struct TypewriterQueue {
    tokens: VecDeque<RenderToken>,
    last_drained_at: Option<Instant>,
    backend_finished: bool,
}

impl TypewriterQueue {
    /// Creates a new empty `TypewriterQueue`.
    pub fn new() -> Self {
        Self {
            tokens: VecDeque::new(),
            last_drained_at: None,
            backend_finished: false,
        }
    }

    /// Appends a render-ready token to the buffer.
    pub fn push(&mut self, token: RenderToken) {
        self.tokens.push_back(token);
    }

    /// Signals that the backend stream has finished sending tokens.
    pub fn finish_backend(&mut self) {
        self.backend_finished = true;
    }

    /// Flushes all remaining buffered tokens and resets pacing metrics.
    pub fn clear(&mut self) {
        self.tokens.clear();
        self.last_drained_at = None;
        self.backend_finished = false;
    }

    /// Returns true if the token buffer is currently empty.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Returns true if the backend stream has completed and all buffered tokens have been drained.
    pub fn is_finished(&self) -> bool {
        self.backend_finished && self.tokens.is_empty()
    }

    /// Drains paced tokens from the queue based on elapsed time since the last drain.
    ///
    /// Invariant: `last_drained_at` advances only when tokens are actually emitted.
    /// This ensures elapsed time accumulates across idle ticks so the 30ms threshold
    /// is reached even when the event loop ticks faster than the drain rate.
    ///
    /// When the backend has finished sending, all remaining tokens are flushed
    /// immediately — there is no value in pacing tokens that have already arrived.
    ///
    /// During active streaming, the drain rate adapts to queue depth:
    /// - queue ≤ 5:  30ms/token — smooth typewriter for LLM-style drip
    /// - queue ≤ 20: 10ms/token — faster for moderate bursts
    /// - queue > 20: flush all  — bulk retrieval data, no pacing
    pub fn drain_for_tick(&mut self, now: Instant) -> DrainResult {
        let is_first = self.last_drained_at.is_none();
        let last = self.last_drained_at.unwrap_or(now);

        if self.tokens.is_empty() {
            return DrainResult {
                emitted: Vec::new(),
                finished: self.is_finished(),
            };
        }

        // When the backend stream has completed, flush everything immediately.
        // Pacing only makes sense while tokens are still arriving.
        let count = if self.backend_finished {
            self.tokens.len()
        } else {
            let queue_len = self.tokens.len();
            // Adaptive pacing: large queues mean bulk data arrived in a burst.
            // Pacing bulk data at 30ms/token produces multi-minute delays for
            // retrieval responses that are already fully in memory.
            if queue_len > 20 {
                // Bulk data — flush everything now.
                queue_len
            } else {
                let rate_ms = if queue_len > 5 { 10 } else { 30 };
                let elapsed = now.duration_since(last);
                if elapsed.as_millis() >= rate_ms {
                    let num = (elapsed.as_millis() / rate_ms) as usize;
                    std::cmp::min(num, queue_len)
                } else if is_first {
                    // Emit the first token immediately on the very first drain call.
                    1
                } else {
                    0
                }
            }
        };

        let mut emitted = Vec::new();
        for _ in 0..count {
            if let Some(tok) = self.tokens.pop_front() {
                emitted.push(tok);
            }
        }

        // Advance the drain timer only when progress is made.
        if !emitted.is_empty() {
            self.last_drained_at = Some(now);
        }

        DrainResult {
            emitted,
            finished: self.is_finished(),
        }
    }
}

impl Default for TypewriterQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulator parsing streaming text chunks into semantic tokens.
pub struct IncrementalTokenizer {
    buffer: String,
}

impl IncrementalTokenizer {
    /// Creates a new `IncrementalTokenizer`.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Alias for `push_chunk` to feed text chunks to the tokenizer.
    pub fn feed(&mut self, chunk: &str) -> Vec<RenderToken> {
        self.push_chunk(chunk)
    }

    /// Processes raw chunks, buffering split segments and returning completed semantic tokens.
    pub fn push_chunk(&mut self, chunk: &str) -> Vec<RenderToken> {
        self.buffer.push_str(chunk);
        let mut tokens = Vec::new();

        if self.buffer.contains(|c: char| c.is_whitespace()) {
            let mut parts: Vec<String> = self
                .buffer
                .split_inclusive(|c: char| c.is_whitespace())
                .map(|s| s.to_string())
                .collect();

            self.buffer.clear();
            if let Some(last) = parts.last() {
                if !last.ends_with(|c: char| c.is_whitespace()) {
                    self.buffer = last.clone();
                    parts.pop();
                }
            }
            for part in parts {
                tokens.push(RenderToken::Text(part));
            }
        }
        tokens
    }

    /// Flushes any remaining split buffer segments into final tokens.
    pub fn flush(&mut self) -> Vec<RenderToken> {
        let mut tokens = Vec::new();
        if !self.buffer.is_empty() {
            tokens.push(RenderToken::Text(self.buffer.clone()));
            self.buffer.clear();
        }
        tokens
    }
}

impl Default for IncrementalTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent bounded command prompt history store.
pub struct HistoryStore {
    entries: Vec<String>,
    index: Option<usize>,
    draft: Option<String>,
    capacity: usize,
}

impl HistoryStore {
    /// Creates a new `HistoryStore` with the specified capacity limit.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            index: None,
            draft: None,
            capacity,
        }
    }

    /// Appends a prompt entry to history, resetting active navigation session.
    pub fn push(&mut self, entry: String) {
        if entry.trim().is_empty() {
            return;
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
        self.reset_navigation();
    }

    /// Resets the current history browsing cursor and draft cache.
    pub fn reset_navigation(&mut self) {
        self.index = None;
        self.draft = None;
    }

    /// Browses to the previous historical entry, caching the active prompt draft if starting.
    pub fn previous_entry(&mut self, current_prompt: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        match self.index {
            None => {
                self.draft = Some(current_prompt.to_string());
                let idx = self.entries.len() - 1;
                self.index = Some(idx);
                Some(self.entries[idx].clone())
            }
            Some(idx) => {
                if idx > 0 {
                    let next_idx = idx - 1;
                    self.index = Some(next_idx);
                    Some(self.entries[next_idx].clone())
                } else {
                    Some(self.entries[0].clone())
                }
            }
        }
    }

    /// Browses to the next historical entry, restoring the uncommitted draft if reaching the end.
    pub fn next_entry(&mut self) -> Option<String> {
        let idx = self.index?;
        if idx < self.entries.len() - 1 {
            let next_idx = idx + 1;
            self.index = Some(next_idx);
            Some(self.entries[next_idx].clone())
        } else {
            self.index = None;
            self.draft.take()
        }
    }
}

/// Interactive input prompt editing buffer.
pub struct EditorState {
    chars: Vec<char>,
    cursor: usize,
    history: HistoryStore,
}

impl EditorState {
    /// Creates a new `EditorState`.
    pub fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            history: HistoryStore::new(500),
        }
    }

    /// Creates a new `EditorState` with custom history capacity.
    pub fn with_history_capacity(capacity: usize) -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            history: HistoryStore::new(capacity),
        }
    }

    /// Returns the active text buffer as a String.
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Returns the active cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Inserts a character at the current cursor position.
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Removes the character immediately preceding the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    /// Removes the character at the current cursor position.
    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    /// Moves the cursor position leftwards by one character cell.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Moves the cursor position rightwards by one character cell.
    pub fn move_right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    /// Validates and submits the active prompt buffer, clearing it and pushing to history.
    pub fn submit(&mut self) -> Option<String> {
        let content = self.text();
        if content.trim().is_empty() {
            return None;
        }
        self.history.push(content.clone());
        self.chars.clear();
        self.cursor = 0;
        Some(content)
    }

    /// Clears the active text buffer and resets cursor position.
    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }

    /// Recalls the previous history entry.
    pub fn recall_up(&mut self) {
        let current = self.text();
        if let Some(prev) = self.history.previous_entry(&current) {
            self.chars = prev.chars().collect();
            self.cursor = self.chars.len();
        }
    }

    /// Recalls the next history entry.
    pub fn recall_down(&mut self) {
        if let Some(next) = self.history.next_entry() {
            self.chars = next.chars().collect();
            self.cursor = self.chars.len();
        }
    }

    /// Sets the text buffer directly and positions cursor at the end.
    pub fn set_text(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Active interface interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiMode {
    /// Conversational message and prompt entry screen.
    Conversation,
    /// Detailed node context reference browsing.
    Exploration,
    /// Live runtime operational observability dashboard screen.
    RuntimeDashboard,
    /// Causal concept explainability timeline screen.
    Explainability,
    /// Actionable reflection proposal review and execution screen.
    InteractiveReflection,
    /// Knowledge Evolution governance policy and planning screen.
    KnowledgeEvolution,
    /// Knowledge Automation scheduled orchestration screen.
    KnowledgeAutomation,
}

/// Active interface overlay state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiOverlay {
    /// No modal overlay open.
    None,
    /// Modal list showing pinned nodes.
    PinnedContext,
}

/// A pinned knowledge node stored in-memory during the active session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedNode {
    /// Unique identifier of the pinned node.
    pub node_id: brain_domain::NodeId,
    /// Cached display name/label of the pinned node.
    pub label: String,
    /// Domain node type category.
    pub node_type: brain_domain::NodeType,
    /// Monotonic session pinning order index.
    pub pinned_at: usize,
}

/// Loading status of the focused Inspector entity query.
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorLoadState {
    /// In-flight RPC request loading.
    Loading,
    /// Successfully loaded model details.
    Loaded(Box<brain_domain::query::inspector::InspectorModel>),
    /// Failed query error.
    Error(String),
}

/// Live browsing context parameters for the knowledge graph explorer.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectorSession {
    /// Currently focused Node ID.
    pub node_id: brain_domain::NodeId,
    /// Loading or completed data payload.
    pub load_state: InspectorLoadState,
    /// Scoped volatile history history.
    pub breadcrumbs: Vec<brain_domain::NodeId>,
    /// Scroll row offset of the inspector list view.
    pub scroll_offset: usize,
    /// Currently highlighted relation connection index.
    pub selected_relation_idx: usize,
}

/// Rich slash command metadata descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    /// Command name string (e.g., "/memory").
    pub name: String,
    /// Detailed description of the command capability.
    pub description: String,
    /// Semantic category classification (e.g., "Graph", "Planning").
    pub category: String,
}

/// Central application layout and editor context.
pub struct UiState {
    /// Active interface mode
    pub mode: TuiMode,
    /// Active interface overlay
    pub overlay: TuiOverlay,
    /// Active exploration inspector session details
    pub active_inspector: Option<InspectorSession>,
    /// List of pinned nodes stored in-memory during session liveness.
    pub pinned_nodes: Vec<PinnedNode>,
    /// Selected index inside the pinned overlay.
    pub pinned_overlay_cursor: usize,
    /// When true, the next prompt submission will include pinned node IDs as
    /// workspace context sent to the daemon. Resets to false after the request
    /// is confirmed dispatched. Has no effect if the workspace is empty.
    pub submit_with_workspace: bool,
    /// Transient message with timestamp for display timeouts.
    pub transient_message: Option<(String, std::time::Instant)>,
    /// Active engine connection mode.
    pub connection_mode: ConnectionMode,
    /// Current conversation session identifier.
    pub session_id: SessionId,
    /// Title description of the active session.
    pub session_title: String,
    /// Interactive prompt input buffer.
    pub editor: EditorState,
    /// Viewport state variables.
    pub viewport: ViewportState,
    /// Stream Generation status machine.
    pub generation_state: GenerationState,
    /// Time-driven typewriter pacing buffer.
    pub typewriter: TypewriterQueue,
    /// The formatted visible generated text response.
    pub active_response: String,
    /// Focus region layout tracker.
    pub focus: FocusRegion,
    /// List of session metadata view models.
    pub sessions: Vec<SessionViewModel>,
    /// Selected index inside `sessions` list.
    pub selected_session_idx: usize,
    /// Sidebar interaction state manager.
    pub sidebar: crate::ui::interaction::sidebar::SidebarInteraction,
    /// Slash completion state.
    pub slash_completion: crate::ui::command::completion::SlashCompletionState,
    /// Command palette state.
    pub command_palette: crate::ui::command::palette::CommandPaletteState,
    /// Encapsulated presentation view model for search results.
    pub search_results_vm: crate::ui::view_models::SearchResultsViewModel,
    /// Encapsulated presentation view model for memory stewardship results.
    pub memory_results_vm: crate::ui::view_models::MemoryResultsViewModel,
    /// Encapsulated presentation view model for diagnostic reasoning execution plans.
    pub reasoning_plan_vm: Option<crate::ui::view_models::ReasoningPlanViewModel>,
    /// Encapsulated inspection navigation session.
    pub inspection_session: crate::ui::view_models::InspectionSession,
    /// Track pending activations atomically.
    pub pending_load: Option<PendingLoad>,

    /// Track active loading state.
    pub session_load_state: SessionLoadState,
    /// List of historical messages for the active session.
    pub active_messages: Vec<brain_domain::Message>,
    /// Monotonic revision sequence values per message content.
    pub message_revisions: std::collections::HashMap<brain_domain::MessageId, u64>,
    /// Monotonic revision value of the active typewriter response text.
    pub active_response_revision: u64,
    /// Collection of active or completed tool calls in the current generation.
    pub active_tool_calls: Vec<crate::ui::command::tool::ToolExecution>,
    /// FIFO queue of pending tool execution approvals.
    pub pending_approvals: Vec<crate::ui::command::tool::ToolApproval>,
    /// Mapping from message ID to permanently completed tool executions.
    pub message_tool_calls: std::collections::HashMap<
        crate::ui::interaction::MessageId,
        Vec<crate::ui::command::tool::ToolExecution>,
    >,
    /// View state for conversation scroll and select.
    pub conversation_view: crate::ui::interaction::navigation::ConversationViewState,
    /// Retrieval metadata objects by ID.
    pub retrievals: std::collections::HashMap<
        brain_domain::bkf::retrieval::RetrievalId,
        brain_domain::bkf::retrieval::RetrievalInfo,
    >,
    /// Mapping from message ID to context retrievals.
    pub message_retrievals: std::collections::HashMap<
        crate::ui::interaction::MessageId,
        Vec<brain_domain::bkf::retrieval::RetrievalId>,
    >,
    /// Chronological list of events in the session.
    pub timeline: Vec<(
        crate::ui::interaction::timeline::EventOrdinal,
        crate::ui::interaction::timeline::TimelineItem,
    )>,
    /// Next monotonic EventOrdinal index.
    pub next_ordinal: u64,
    /// Toggle state for showing/hiding reflection logs.
    pub enable_reflection_logs: bool,
    /// Current terminal window columns.
    pub terminal_width: u16,
    /// Current terminal window rows.
    pub terminal_height: u16,
    /// In-memory store of conversation histories per Session ID.
    pub session_histories: std::collections::HashMap<SessionId, Vec<brain_domain::Message>>,
    /// Stateful container for RuntimeDashboard cursor navigation.
    pub runtime_dashboard_state: crate::ui::widgets::runtime_dashboard::RuntimeDashboardState,
    /// Latest atomic point-in-time runtime diagnostics snapshot report.
    pub diagnostics_report: Option<brain_integrations::dto::v1::RuntimeDiagnosticsReport>,
    /// Stateful container for KnowledgeExplorer session navigation.
    pub knowledge_explorer_state: crate::ui::widgets::knowledge_explorer::KnowledgeExplorerState,
    /// Concept catalog list DTOs for KnowledgeExplorer.
    pub explorer_concepts: Vec<brain_integrations::dto::v1::ConceptSummaryDto>,
    /// Currently focused concept detail report DTO for KnowledgeExplorer.
    pub explorer_concept_detail: Option<brain_integrations::dto::v1::ConceptDetailReport>,
    /// Stateful container for Explainability session navigation.
    pub explainability_state: crate::ui::widgets::explainability::ExplainabilityState,
    /// Latest concept causal explanation report DTO.
    pub explanation_report: Option<brain_integrations::dto::v1::ExplanationReport>,
    /// Stateful container for InteractiveReflection session navigation.
    pub interactive_reflection_state:
        crate::ui::widgets::interactive_reflection::InteractiveReflectionState,
    /// Catalog of reviewable reflection proposal DTOs.
    pub reflection_proposals: Vec<brain_integrations::dto::v1::ReflectionProposalDto>,
    /// Stateful container for KnowledgeEvolution session navigation.
    pub knowledge_evolution_state: crate::ui::widgets::knowledge_evolution::KnowledgeEvolutionState,
    /// Catalog of governance evolution policies.
    pub evolution_policies: Vec<brain_integrations::dto::v1::EvolutionPolicyDto>,
    /// Active draft/executed evolution plan proposal.
    pub active_evolution_plan: Option<brain_integrations::dto::v1::EvolutionPlanDto>,
    /// Separate side-effect-free evolution simulation report.
    pub evolution_simulation_report: Option<brain_integrations::dto::v1::EvolutionSimulationReport>,
    /// Historical audit records log.
    pub evolution_audit_history: Vec<brain_integrations::dto::v1::EvolutionAuditRecordDto>,
    /// Stateful container for KnowledgeAutomation session navigation.
    pub knowledge_automation_state:
        crate::ui::widgets::knowledge_automation::KnowledgeAutomationState,
    /// Catalog of automation rules.
    pub automation_rules: Vec<brain_integrations::dto::v1::AutomationRuleDto>,
    /// Scheduled background execution queue items.
    pub automation_queue: Vec<brain_integrations::dto::v1::AutomationQueueItemDto>,
    /// Automation execution history log records.
    pub automation_execution_logs: Vec<brain_integrations::dto::v1::AutomationExecutionLogDto>,
    /// Set of collapsed result group indices for progressive disclosure.
    pub collapsed_groups: std::collections::HashSet<usize>,
}

/// TimelineBlock is a pure presentation model. It wraps AST-parsed markdown visual lines along with structural headers
/// (e.g. "Brain", "You") and sizing information. It exists solely to facilitate virtualized layout wrapping,
/// index-based viewport slicing, and render-time column constraints. It does not carry any domain invariants or persist
/// beyond the rendering tick.
#[derive(Debug, Clone)]
pub struct TimelineBlock {
    /// Optional sender header name (e.g. "You", "Brain").
    pub header: Option<String>,
    /// Chronological list of visual lines wrapped to the column width.
    pub visual_lines: Vec<crate::ui::interaction::markdown::VisualLine>,
}

/// Structured user action triggering pure state transitions.
pub enum Action {
    /// Append a text character to input.
    InsertChar(char),
    /// Move prompt editor cursor left.
    MoveCursorLeft,
    /// Move prompt editor cursor right.
    MoveCursorRight,
    /// Trigger backspace text deletion.
    Backspace,
    /// Trigger delete text deletion.
    Delete,
    /// Force screen dimension recalculation.
    Resize(u16, u16),
    /// Request TUI termination.
    Quit,
    /// Modify connection mode.
    SetConnectionMode(ConnectionMode),
    /// Submit the active prompt buffer.
    SubmitPrompt,
    /// Recall the previous prompt history.
    RecallPrevious,
    /// Recall the next prompt history.
    RecallNext,
    /// Signals the start of query response streaming.
    StartStream,
    /// Receive a presentation token into typewriter queue.
    ReceiveToken(RenderToken),
    /// Paced clock pulse updating the typewriter animation.
    TypewriterTick(Instant),
    /// Signals that backend has completed sending token chunks.
    FinishStream,
    /// User requested cancellation of active query generation.
    CancelStream,
    /// Report model or socket communication error.
    ReportError(String),
    /// Loaded list of session metadata summaries.
    LoadSessions(Vec<crate::client::SessionSummary>),
    /// Tool call request from backend.
    ToolCallRequested {
        /// Message ID that triggered the tool call.
        message: MessageId,
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// Target tool ID.
        tool_id: brain_core::events::ToolId,
        /// JSON string representation of arguments.
        arguments: String,
        /// True if approval is required.
        requires_approval: bool,
    },
    /// Tool execution progress update.
    ToolProgressReceived {
        /// Message ID.
        message: MessageId,
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// Monotonic sequence within tool call lifecycle.
        sequence: u64,
        /// Determinate or indeterminate progress metrics.
        detail: brain_core::events::ToolProgressDetail,
        /// Diagnostic progress text message.
        log_message: String,
    },
    /// Final result output of a tool call.
    ToolResultReceived {
        /// Message ID.
        message: MessageId,
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// Output result text.
        result: String,
        /// True if execution resulted in error.
        is_error: bool,
    },
    /// Approve or deny tool call.
    ApproveToolCall {
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// True if approved.
        approved: bool,
    },
    /// Retrieval phase has started.
    RetrievalStarted {
        /// Message requesting retrieval.
        message: MessageId,
        /// The query text being searched.
        query: String,
    },
    /// A retrieved context block has been received.
    RetrievalReceived {
        /// Message requesting retrieval.
        message: MessageId,
        /// Detailed user-facing retrieval entry.
        info: brain_domain::bkf::retrieval::RetrievalInfo,
    },
    /// Retrieval phase completed successfully.
    RetrievalCompleted {
        /// Message requesting retrieval.
        message: MessageId,
    },

    /// Tab cycles focus region.
    ToggleFocus,
    /// Move selected sidebar list cursor up.
    MoveSidebarCursorUp,
    /// Move selected sidebar list cursor down.
    MoveSidebarCursorDown,
    /// Enter activates highlighted session, spawning async history query.
    ActivateSession {
        /// Selected session ID to load.
        session_id: SessionId,
        /// Monotonic version tracking load sequence.
        request_id: LoadRequestId,
    },
    /// Asynchronous lazy-load of session messages completes successfully.
    SessionLoaded {
        /// Loaded session ID.
        session_id: SessionId,
        /// Original matching request ID version.
        request_id: LoadRequestId,
        /// Full list of historical messages.
        messages: Vec<brain_domain::Message>,
    },
    /// Asynchronous lazy-load fails with diagnostic error details.
    SessionLoadFailed {
        /// Target session ID.
        session_id: SessionId,
        /// Matching request ID.
        request_id: LoadRequestId,
        /// Diagnostic error description.
        error: String,
    },
    /// Delete selected session thread permanently.
    DeleteSession(SessionId),
    /// Toggle the visibility of KPP reflection logs in TUI.
    ToggleReflectionLogs,
    /// Toggle group expansion state for result groups.
    ToggleGroupExpand(usize),
    /// Create a fresh conversation thread.
    NewSession,
    /// Scroll the viewport up by a specified line count.
    ScrollUp(usize),
    /// Scroll the viewport down by a specified line count.
    ScrollDown(usize),
    /// Jump scroll offset to the top of the viewport.
    JumpToTop,
    /// Jump scroll offset to the bottom of the viewport given total row count.
    JumpToBottom(usize),
    /// Navigate/inspect node details.
    InspectNode(brain_domain::NodeId),
    /// Inspector details loaded successfully.
    NodeDetailsLoaded(Box<brain_domain::query::inspector::InspectorModel>),
    /// Inspector details query failed.
    NodeDetailsFailed(String),
    /// Backspace pops breadcrumb navigation history.
    PopBreadcrumb,
    /// Close Inspector and return to Conversation Mode.
    CloseInspector,
    /// Scroll the Inspector details panel up.
    ScrollInspectorUp(usize),
    /// Scroll the Inspector details panel down.
    ScrollInspectorDown(usize),
    /// Highlight next connection.
    NextInspectorRelation,
    /// Highlight previous connection.
    PrevInspectorRelation,
    /// Inspect the highlighted relation connection.
    TraverseToRelation,
    /// Pin/unpin the currently open Inspector node.
    PinCurrentNode,
    /// Unpin a specific node.
    UnpinNode(brain_domain::NodeId),
    /// Clear all pinned context nodes.
    ClearAllPins,
    /// Open the pinned context overlay.
    OpenPinnedOverlay,
    /// Close the pinned context overlay.
    ClosePinnedOverlay,
    /// Scroll the pinned overlay cursor up.
    PinnedOverlayUp,
    /// Scroll the pinned overlay cursor down.
    PinnedOverlayDown,
    /// Inspect the pinned node at the given index.
    InspectPinnedNode(usize),
    /// Toggle whether the next prompt submission includes workspace context.
    /// No-op if the workspace is empty; no-op if already toggled on with no
    /// change in workspace contents.
    ToggleSubmitWithWorkspace,
    /// Clear submit_with_workspace after a query was successfully dispatched.
    /// Must be called by the caller only after client.execute() returns Ok.
    ResetSubmitWithWorkspace,
    /// Set a transient status-bar message (clears on next tick if stale).
    SetTransientMessage(String),
    /// Loaded list of memory summaries for stewardship.
    LoadMemories(Vec<brain_domain::MemorySummary>),
    /// Memory list load failed.
    MemoryListFailed(String),
    /// Pin a specific memory item into context.
    PinMemory(String),
    /// Unpin a specific memory item from context.
    UnpinMemory(String),
    /// Archive a specific memory item into cold storage.
    ArchiveMemory(String),
    /// Restore an archived memory item back to active stewardship.
    RestoreMemory(String),
    /// Rollback an optimistic memory mutation on reconciliation failure.
    RollbackMemoryMutation(crate::ui::view_models::MemoryItemViewModel),
    /// Diagnostic reasoning plan generated.
    ReasoningPlanGenerated(brain_domain::ExecutionPlan),
}

/// Pure status indicator returning from state updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateResult {
    /// Redraw not needed.
    NoChange,
    /// Trigger layout update and redraw.
    Changed,
    /// Request execution of submitted prompt.
    PromptSubmitted(String),
    /// Request loading of the specified session history.
    LoadSession(SessionId),
    /// Spawn async UDS call to inspect node details.
    InspectNode(brain_domain::NodeId),
    /// Exit main interactive loop.
    Exit,
}

impl UiState {
    /// Creates a default `UiState` with random Session ID.
    pub fn new() -> Self {
        Self {
            mode: TuiMode::Conversation,
            overlay: TuiOverlay::None,
            active_inspector: None,
            pinned_nodes: Vec::new(),
            pinned_overlay_cursor: 0,
            submit_with_workspace: false,
            transient_message: None,
            connection_mode: ConnectionMode::Disconnected,
            session_id: SessionId::new(),
            session_title: "New Conversation".to_string(),
            editor: EditorState::new(),
            viewport: ViewportState {
                scroll_offset: 0,
                follow_tail: true,
                is_command_palette_open: false,
            },
            generation_state: GenerationState::Idle,
            typewriter: TypewriterQueue::new(),
            active_response: String::new(),
            focus: FocusRegion::Editor,
            sessions: Vec::new(),
            selected_session_idx: 0,
            sidebar: crate::ui::interaction::sidebar::SidebarInteraction::new(),
            slash_completion: crate::ui::command::completion::SlashCompletionState::new(),
            command_palette: crate::ui::command::palette::CommandPaletteState::new(),
            search_results_vm: crate::ui::view_models::SearchResultsViewModel::default(),
            memory_results_vm: crate::ui::view_models::MemoryResultsViewModel::default(),
            reasoning_plan_vm: None,
            inspection_session: crate::ui::view_models::InspectionSession::default(),
            pending_load: None,

            session_load_state: SessionLoadState::NotLoaded,
            active_messages: Vec::new(),
            message_revisions: std::collections::HashMap::new(),
            active_response_revision: 0,
            active_tool_calls: Vec::new(),
            pending_approvals: Vec::new(),
            message_tool_calls: std::collections::HashMap::new(),
            conversation_view: crate::ui::interaction::navigation::ConversationViewState::default(),
            retrievals: std::collections::HashMap::new(),
            message_retrievals: std::collections::HashMap::new(),
            timeline: Vec::new(),
            next_ordinal: 1,
            enable_reflection_logs: false,
            terminal_width: 80,
            terminal_height: 24,
            session_histories: std::collections::HashMap::new(),
            runtime_dashboard_state:
                crate::ui::widgets::runtime_dashboard::RuntimeDashboardState::default(),
            diagnostics_report: None,
            knowledge_explorer_state:
                crate::ui::widgets::knowledge_explorer::KnowledgeExplorerState::default(),
            explorer_concepts: Vec::new(),
            explorer_concept_detail: None,
            explainability_state: crate::ui::widgets::explainability::ExplainabilityState::default(
            ),
            explanation_report: None,
            interactive_reflection_state:
                crate::ui::widgets::interactive_reflection::InteractiveReflectionState::default(),
            reflection_proposals: Vec::new(),
            knowledge_evolution_state:
                crate::ui::widgets::knowledge_evolution::KnowledgeEvolutionState::default(),
            evolution_policies: Vec::new(),
            active_evolution_plan: None,
            evolution_simulation_report: None,
            evolution_audit_history: Vec::new(),
            knowledge_automation_state:
                crate::ui::widgets::knowledge_automation::KnowledgeAutomationState::default(),
            automation_rules: Vec::new(),
            automation_queue: Vec::new(),
            automation_execution_logs: Vec::new(),
            collapsed_groups: std::collections::HashSet::new(),
        }
    }

    /// Creates a new `UiState` with custom history capacity.
    pub fn with_history_capacity(capacity: usize) -> Self {
        Self {
            mode: TuiMode::Conversation,
            overlay: TuiOverlay::None,
            active_inspector: None,
            pinned_nodes: Vec::new(),
            pinned_overlay_cursor: 0,
            submit_with_workspace: false,
            transient_message: None,
            connection_mode: ConnectionMode::Disconnected,
            session_id: SessionId::new(),
            session_title: "New Conversation".to_string(),
            editor: EditorState::with_history_capacity(capacity),
            viewport: ViewportState {
                scroll_offset: 0,
                follow_tail: true,
                is_command_palette_open: false,
            },
            generation_state: GenerationState::Idle,
            typewriter: TypewriterQueue::new(),
            active_response: String::new(),
            focus: FocusRegion::Editor,
            sessions: Vec::new(),
            selected_session_idx: 0,
            sidebar: crate::ui::interaction::sidebar::SidebarInteraction::new(),
            slash_completion: crate::ui::command::completion::SlashCompletionState::new(),
            command_palette: crate::ui::command::palette::CommandPaletteState::new(),
            search_results_vm: crate::ui::view_models::SearchResultsViewModel::default(),
            memory_results_vm: crate::ui::view_models::MemoryResultsViewModel::default(),
            reasoning_plan_vm: None,
            inspection_session: crate::ui::view_models::InspectionSession::default(),
            pending_load: None,

            session_load_state: SessionLoadState::NotLoaded,
            active_messages: Vec::new(),
            message_revisions: std::collections::HashMap::new(),
            active_response_revision: 0,
            active_tool_calls: Vec::new(),
            pending_approvals: Vec::new(),
            message_tool_calls: std::collections::HashMap::new(),
            conversation_view: crate::ui::interaction::navigation::ConversationViewState::default(),
            retrievals: std::collections::HashMap::new(),
            message_retrievals: std::collections::HashMap::new(),
            timeline: Vec::new(),
            next_ordinal: 1,
            enable_reflection_logs: false,
            terminal_width: 80,
            terminal_height: 24,
            session_histories: std::collections::HashMap::new(),
            runtime_dashboard_state:
                crate::ui::widgets::runtime_dashboard::RuntimeDashboardState::default(),
            diagnostics_report: None,
            knowledge_explorer_state:
                crate::ui::widgets::knowledge_explorer::KnowledgeExplorerState::default(),
            explorer_concepts: Vec::new(),
            explorer_concept_detail: None,
            explainability_state: crate::ui::widgets::explainability::ExplainabilityState::default(
            ),
            explanation_report: None,
            interactive_reflection_state:
                crate::ui::widgets::interactive_reflection::InteractiveReflectionState::default(),
            reflection_proposals: Vec::new(),
            knowledge_evolution_state:
                crate::ui::widgets::knowledge_evolution::KnowledgeEvolutionState::default(),
            evolution_policies: Vec::new(),
            active_evolution_plan: None,
            evolution_simulation_report: None,
            evolution_audit_history: Vec::new(),
            knowledge_automation_state:
                crate::ui::widgets::knowledge_automation::KnowledgeAutomationState::default(),
            automation_rules: Vec::new(),
            automation_queue: Vec::new(),
            automation_execution_logs: Vec::new(),
            collapsed_groups: std::collections::HashSet::new(),
        }
    }

    /// Idempotent helper clearing the pending load target and updating load state if Loading.
    pub fn clear_pending_load(&mut self) {
        self.pending_load = None;
        if matches!(self.session_load_state, SessionLoadState::Loading) {
            self.session_load_state = SessionLoadState::NotLoaded;
        }
    }

    /// Returns a virtualized `PresentationModel` slicing total rows to viewport bounds.
    pub fn presentation_model(&self, total_rows: usize, viewport_height: usize) -> PresentationModel {
        let v_height = max(1, viewport_height);
        let offset = min(self.viewport.scroll_offset, total_rows.saturating_sub(1));
        let end = min(offset + v_height, total_rows);

        let mut visible = Vec::new();
        for i in offset..end {
            visible.push(VisibleRow {
                index: i,
                content: format!("Row {}", i + 1),
                is_highlighted: false,
            });
        }

        let indicator = if total_rows == 0 {
            "0 results".to_string()
        } else {
            format!("Showing {}-{} of {}", offset + 1, end, total_rows)
        };

        PresentationModel {
            visible_rows: visible,
            total_rows,
            scroll_offset: offset,
            viewport_height: v_height,
            scroll_indicator: indicator,
        }
    }

    /// Returns the static registry of available slash commands and their metadata.
    pub fn slash_commands() -> Vec<SlashCommand> {
        vec![
            SlashCommand { name: "/memory".into(), description: "Query relational memory graph".into(), category: "Graph".into() },
            SlashCommand { name: "/plan".into(), description: "Display execution roadmap".into(), category: "Planning".into() },
            SlashCommand { name: "/pin".into(), description: "Pin item into active context overlay".into(), category: "Context".into() },
            SlashCommand { name: "/unpin".into(), description: "Unpin item from context overlay".into(), category: "Context".into() },
            SlashCommand { name: "/archive".into(), description: "Archive conversation session".into(), category: "Session".into() },
            SlashCommand { name: "/restore".into(), description: "Restore archived conversation session".into(), category: "Session".into() },
        ]
    }

    /// Returns matching slash command suggestions for the current prompt text.
    pub fn get_slash_suggestions(&self) -> Vec<SlashCommand> {
        let text_buf = self.editor.text();
        let text = text_buf.trim();
        if !text.starts_with('/') {
            return Vec::new();
        }
        Self::slash_commands()
            .into_iter()
            .filter(|c| c.name.starts_with(text))
            .collect()
    }

    /// Read-only accessor for CommandPaletteState.
    pub fn command_palette(&self) -> &crate::ui::command::palette::CommandPaletteState {
        &self.command_palette
    }

    /// Mutable accessor for CommandPaletteState.
    pub fn command_palette_mut(&mut self) -> &mut crate::ui::command::palette::CommandPaletteState {
        &mut self.command_palette
    }

    /// Read-only accessor for SlashCompletionState.
    pub fn slash_completion(&self) -> &crate::ui::command::completion::SlashCompletionState {
        &self.slash_completion
    }

    /// Mutable accessor for SlashCompletionState.
    pub fn slash_completion_mut(
        &mut self,
    ) -> &mut crate::ui::command::completion::SlashCompletionState {
        &mut self.slash_completion
    }

    /// Returns the filtered and sorted list of sessions that are visible in the sidebar.
    pub fn visible_sessions(&self) -> Vec<SessionViewModel> {
        let mut filtered: Vec<SessionViewModel> = self
            .sessions
            .iter()
            .filter(|s| {
                // Filter by active/archived state
                let matches_filter = match self.sidebar.browse.filter {
                    crate::ui::interaction::sidebar::SessionFilter::Active => !s.archived,
                    crate::ui::interaction::sidebar::SessionFilter::Archived => s.archived,
                };
                if !matches_filter {
                    return false;
                }

                // Filter by search terms
                if self.sidebar.search.active && !self.sidebar.search.editor.buffer().is_empty() {
                    self.sidebar.search.parsed.matches(&s.title)
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        // Sort:
        // 1. Pinned sessions first (if filter is Active), sorted by updated_at descending
        // 2. Unpinned sessions next, sorted by updated_at descending
        filtered.sort_by(|a, b| {
            if self.sidebar.browse.filter == crate::ui::interaction::sidebar::SessionFilter::Active
            {
                match (a.pinned, b.pinned) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.updated_at.cmp(&a.updated_at),
                }
            } else {
                b.updated_at.cmp(&a.updated_at)
            }
        });

        filtered
    }

    /// Returns true if the query response is actively generating.
    pub fn is_generating(&self) -> bool {
        matches!(
            self.generation_state,
            GenerationState::Starting | GenerationState::Streaming { .. }
        )
    }

    /// Returns the number of tokens currently queued in the typewriter buffer.
    pub fn typewriter_queue_len(&self) -> usize {
        self.typewriter.tokens.len()
    }

    /// Returns whether the typewriter backend has signalled completion.
    pub fn typewriter_backend_finished(&self) -> bool {
        self.typewriter.backend_finished
    }

    /// Returns true if the specified result group index is collapsed.
    pub fn is_group_collapsed(&self, group_idx: usize) -> bool {
        self.collapsed_groups.contains(&group_idx)
    }

    /// Pure reducer transitioning state based on Action.
    pub fn update(&mut self, action: Action) -> UpdateResult {
        let prev_messages = self.active_messages.clone();
        let res = self.update_internal(action);
        self.update_message_revisions(&prev_messages);
        self.sync_slash_completion();
        res
    }

    /// Synchronizes the visibility and query of the slash completion popup based on editor buffer.
    pub fn sync_slash_completion(&mut self) {
        if self.focus == FocusRegion::Editor {
            let text = self.editor.text();
            if text.starts_with('/') && !text.contains(' ') {
                if self.slash_completion.dismissed_query.as_deref() == Some(text.as_str()) {
                    self.slash_completion.visible = false;
                    return;
                }
                self.slash_completion.dismissed_query = None;
                let matches_count =
                    crate::ui::command::completion::SlashCompletionEngine::matches(&text).count();
                if matches_count > 0 {
                    self.slash_completion.visible = true;
                    self.slash_completion.query = text.to_string();
                    if self.slash_completion.selected_index >= matches_count {
                        self.slash_completion.selected_index = 0;
                    }
                } else {
                    self.slash_completion.visible = false;
                }
            } else {
                self.slash_completion.visible = false;
                self.slash_completion.dismissed_query = None;
            }
        } else {
            self.slash_completion.visible = false;
            self.slash_completion.dismissed_query = None;
        }
    }

    fn update_message_revisions(&mut self, prev_messages: &[brain_domain::Message]) {
        for msg in &self.active_messages {
            let mut changed = true;
            if let Some(prev) = prev_messages.iter().find(|m| m.id == msg.id) {
                if prev.content == msg.content {
                    changed = false;
                }
            }
            if changed {
                let entry = self.message_revisions.entry(msg.id).or_insert(0);
                *entry += 1;
            }
        }
    }

    fn update_internal(&mut self, action: Action) -> UpdateResult {
        match action {
            Action::InsertChar(c) => {
                if self.focus == FocusRegion::Editor {
                    self.editor.insert(c);
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::MoveCursorLeft => {
                if self.focus == FocusRegion::Editor {
                    self.editor.move_left();
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::MoveCursorRight => {
                if self.focus == FocusRegion::Editor {
                    self.editor.move_right();
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::Backspace => {
                if self.focus == FocusRegion::Editor {
                    self.editor.backspace();
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::Delete => {
                if self.focus == FocusRegion::Editor {
                    self.editor.delete();
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::Quit => {
                self.clear_pending_load();
                UpdateResult::Exit
            }
            Action::SetConnectionMode(mode) => {
                self.connection_mode = mode;
                UpdateResult::Changed
            }
            Action::SubmitPrompt => {
                if self.focus == FocusRegion::Editor {
                    let prompt = self.editor.text();
                    if prompt.trim().is_empty() {
                        return UpdateResult::NoChange;
                    }
                    self.editor.submit();
                    self.generation_state = GenerationState::Starting;
                    self.active_response = String::new();
                    self.active_response_revision += 1;
                    self.typewriter.clear();

                    // Save User message to active session
                    let user_msg = brain_domain::Message::new(
                        brain_domain::MessageId::new(),
                        brain_domain::MessageRole::User,
                        prompt.clone(),
                    );
                    self.active_messages.push(user_msg.clone());
                    self.session_histories
                        .entry(self.session_id)
                        .or_default()
                        .push(user_msg);

                    let user_msg_id =
                        crate::ui::interaction::MessageId(self.active_messages.len() as u64);
                    self.timeline.push((
                        crate::ui::interaction::timeline::EventOrdinal(self.next_ordinal),
                        crate::ui::interaction::timeline::TimelineItem::Message(user_msg_id),
                    ));
                    self.next_ordinal += 1;

                    // Rename conversation title if it is currently default "New Conversation"
                    if self.session_title == "New Conversation" {
                        let mut new_title = prompt.trim().to_string();
                        if let Some(idx) = new_title.find('\n') {
                            new_title.truncate(idx);
                        }
                        if new_title.chars().count() > 25 {
                            let char_idx = new_title
                                .char_indices()
                                .nth(25)
                                .map(|(i, _)| i)
                                .unwrap_or(25);
                            new_title.truncate(char_idx);
                            new_title.push_str("...");
                        }
                        if !new_title.is_empty() {
                            self.session_title = new_title.clone();
                            if let Some(idx) =
                                self.sessions.iter().position(|s| s.id == self.session_id)
                            {
                                self.sessions[idx].title = new_title;
                            }
                        }
                    }

                    UpdateResult::PromptSubmitted(prompt)
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::RecallPrevious => {
                if self.focus == FocusRegion::Editor {
                    self.editor.recall_up();
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::RecallNext => {
                if self.focus == FocusRegion::Editor {
                    self.editor.recall_down();
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::StartStream => {
                self.typewriter.clear();
                self.active_response.clear();
                self.active_response_revision += 1;
                self.active_tool_calls.clear();
                self.pending_approvals.clear();
                self.generation_state = GenerationState::Starting;
                UpdateResult::Changed
            }
            Action::ReceiveToken(token) => {
                self.typewriter.push(token);
                if self.generation_state == GenerationState::Starting {
                    self.generation_state = GenerationState::Streaming {
                        started_at: SystemTime::now(),
                    };
                    let active_msg_id = crate::ui::interaction::MessageId(0);
                    if !self.timeline.iter().any(|(_, item)| matches!(item, crate::ui::interaction::timeline::TimelineItem::Message(id) if *id == active_msg_id)) {
                        self.timeline.push((
                            crate::ui::interaction::timeline::EventOrdinal(self.next_ordinal),
                            crate::ui::interaction::timeline::TimelineItem::Message(active_msg_id),
                        ));
                        self.next_ordinal += 1;
                    }
                }
                UpdateResult::Changed
            }
            Action::TypewriterTick(now) => {
                let mut changed = false;
                if let Some((_, start)) = self.transient_message {
                    if now.duration_since(start).as_secs() >= 3 {
                        self.transient_message = None;
                        changed = true;
                    }
                }
                let res = self.typewriter.drain_for_tick(now);
                let drained = !res.emitted.is_empty();
                if drained {
                    for tok in res.emitted {
                        match tok {
                            RenderToken::Text(t) => self.active_response.push_str(&t),
                            RenderToken::Code(c) => self.active_response.push_str(&c),
                        }
                    }
                    self.active_response_revision += 1;
                }
                if res.finished {
                    self.generation_state = GenerationState::Finished;
                    self.commit_active_response();
                }
                if drained || res.finished || changed {
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::FinishStream => {
                self.typewriter.finish_backend();
                if self.typewriter.is_finished() {
                    self.generation_state = GenerationState::Finished;
                    self.commit_active_response();
                }
                UpdateResult::Changed
            }
            Action::CancelStream => {
                self.typewriter.clear();
                self.commit_active_response();
                self.generation_state = GenerationState::Cancelled(None);
                UpdateResult::Changed
            }
            Action::ReportError(msg) => {
                self.typewriter.clear();
                self.commit_active_response();
                self.generation_state = GenerationState::Error(msg);
                UpdateResult::Changed
            }
            Action::LoadSessions(summaries) => {
                self.sessions = summaries
                    .into_iter()
                    .map(|s| {
                        let active = s.id == self.session_id;
                        SessionViewModel {
                            id: s.id,
                            title: s.title,
                            updated_at: s.updated_at,
                            active,
                            preview: None,
                            pinned: s.pinned,
                            archived: s.archived,
                        }
                    })
                    .collect();
                if let Some(idx) = self.sessions.iter().position(|s| s.id == self.session_id) {
                    self.selected_session_idx = idx;
                } else {
                    self.selected_session_idx = 0;
                }
                UpdateResult::Changed
            }
            Action::ToggleFocus => {
                if self.mode == TuiMode::Exploration {
                    self.focus = match self.focus {
                        FocusRegion::Inspector => FocusRegion::Timeline,
                        FocusRegion::Timeline => FocusRegion::Inspector,
                        _ => FocusRegion::Inspector,
                    };
                } else if self.terminal_width >= 80 {
                    self.focus = match self.focus {
                        FocusRegion::Editor => FocusRegion::Sidebar,
                        FocusRegion::Sidebar => FocusRegion::Editor,
                        _ => FocusRegion::Editor,
                    };
                } else {
                    self.focus = FocusRegion::Editor;
                }
                UpdateResult::Changed
            }
            Action::MoveSidebarCursorUp => {
                if self.focus == FocusRegion::Sidebar && self.selected_session_idx > 0 {
                    self.selected_session_idx -= 1;
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::MoveSidebarCursorDown => {
                if self.focus == FocusRegion::Sidebar
                    && !self.sessions.is_empty()
                    && self.selected_session_idx < self.sessions.len() - 1
                {
                    self.selected_session_idx += 1;
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::ActivateSession {
                session_id,
                request_id,
            } => {
                // Save current messages to histories first (Session Boundary Sync)
                self.session_histories
                    .insert(self.session_id, self.active_messages.clone());

                self.pending_load = Some(PendingLoad {
                    session_id,
                    request_id,
                });
                self.session_load_state = SessionLoadState::Loading;

                // If we already have the history in-memory, load it immediately for snappy switching.
                if let Some(messages) = self.session_histories.get(&session_id).cloned() {
                    self.session_id = session_id;
                    for s in &mut self.sessions {
                        s.active = s.id == session_id;
                    }
                    if let Some(idx) = self.sessions.iter().position(|s| s.id == session_id) {
                        self.session_title = self.sessions[idx].title.clone();
                    }
                    self.active_messages = messages;
                    self.session_load_state =
                        SessionLoadState::Loaded(self.active_messages.clone());
                    self.active_tool_calls.clear();
                    self.pending_approvals.clear();
                    self.message_tool_calls.clear();
                    self.retrievals.clear();
                    self.message_retrievals.clear();
                    self.timeline.clear();
                    self.next_ordinal = 1;
                    for idx in 0..self.active_messages.len() {
                        let msg_id = crate::ui::interaction::MessageId((idx + 1) as u64);
                        self.timeline.push((
                            crate::ui::interaction::timeline::EventOrdinal(self.next_ordinal),
                            crate::ui::interaction::timeline::TimelineItem::Message(msg_id),
                        ));
                        self.next_ordinal += 1;
                    }
                } else {
                    self.active_tool_calls.clear();
                    self.pending_approvals.clear();
                    self.message_tool_calls.clear();
                    self.retrievals.clear();
                    self.message_retrievals.clear();
                    self.timeline.clear();
                    self.next_ordinal = 1;
                }
                UpdateResult::Changed
            }
            Action::SessionLoaded {
                session_id,
                request_id,
                messages,
            } => {
                if let Some(ref pending) = self.pending_load {
                    if pending.request_id == request_id {
                        debug_assert_eq!(pending.session_id, session_id);
                        debug_assert_eq!(pending.request_id, request_id);

                        self.session_id = session_id;
                        for s in &mut self.sessions {
                            s.active = s.id == session_id;
                        }
                        if let Some(idx) = self.sessions.iter().position(|s| s.id == session_id) {
                            self.session_title = self.sessions[idx].title.clone();
                        }
                        self.active_messages = messages.clone();
                        self.session_histories.insert(session_id, messages.clone());
                        self.session_load_state = SessionLoadState::Loaded(messages);
                        self.active_tool_calls.clear();
                        self.pending_approvals.clear();
                        self.message_tool_calls.clear();
                        self.retrievals.clear();
                        self.message_retrievals.clear();
                        self.timeline.clear();
                        self.next_ordinal = 1;
                        for idx in 0..self.active_messages.len() {
                            let msg_id = crate::ui::interaction::MessageId((idx + 1) as u64);
                            self.timeline.push((
                                crate::ui::interaction::timeline::EventOrdinal(self.next_ordinal),
                                crate::ui::interaction::timeline::TimelineItem::Message(msg_id),
                            ));
                            self.next_ordinal += 1;
                        }
                        self.clear_pending_load();
                        return UpdateResult::Changed;
                    }
                }
                UpdateResult::NoChange
            }
            Action::SessionLoadFailed {
                session_id,
                request_id,
                error,
            } => {
                if let Some(ref pending) = self.pending_load {
                    if pending.request_id == request_id {
                        debug_assert_eq!(pending.session_id, session_id);
                        debug_assert_eq!(pending.request_id, request_id);

                        self.session_load_state = SessionLoadState::Error(error);
                        self.clear_pending_load();
                        return UpdateResult::Changed;
                    }
                }
                UpdateResult::NoChange
            }
            Action::ToggleReflectionLogs => {
                self.enable_reflection_logs = !self.enable_reflection_logs;
                UpdateResult::Changed
            }
            Action::ToggleGroupExpand(group_idx) => {
                if self.collapsed_groups.contains(&group_idx) {
                    self.collapsed_groups.remove(&group_idx);
                } else {
                    self.collapsed_groups.insert(group_idx);
                }
                UpdateResult::Changed
            }
            Action::NewSession => {
                // Save current messages to histories first (Session Boundary Sync)
                self.session_histories
                    .insert(self.session_id, self.active_messages.clone());

                let new_id = SessionId::new();
                let new_sess = SessionViewModel {
                    id: new_id,
                    title: "New Conversation".to_string(),
                    updated_at: std::time::SystemTime::now(),
                    active: true,
                    preview: None,
                    pinned: false,
                    archived: false,
                };

                // Deactivate other sessions
                for s in &mut self.sessions {
                    s.active = false;
                }

                // Add and activate new session
                self.sessions.insert(0, new_sess);
                self.session_id = new_id;
                self.session_title = "New Conversation".to_string();
                self.selected_session_idx = 0;

                // Reset timeline and state
                self.active_messages.clear();
                self.active_tool_calls.clear();
                self.pending_approvals.clear();
                self.message_tool_calls.clear();
                self.retrievals.clear();
                self.message_retrievals.clear();
                self.timeline.clear();
                self.next_ordinal = 1;
                self.session_load_state = SessionLoadState::Loaded(Vec::new());
                self.viewport.scroll_offset = 0;
                self.viewport.follow_tail = true;
                self.editor.clear();
                self.focus = FocusRegion::Editor;

                UpdateResult::Changed
            }
            Action::InspectNode(node_id) => {
                self.mode = TuiMode::Exploration;
                self.focus = FocusRegion::Inspector;

                let mut breadcrumbs = Vec::new();
                if let Some(ref active) = self.active_inspector {
                    breadcrumbs = active.breadcrumbs.clone();
                }
                // Only push if not already at the top of breadcrumbs to avoid duplicate loads
                if breadcrumbs.last() != Some(&node_id) {
                    breadcrumbs.push(node_id);
                }

                self.active_inspector = Some(InspectorSession {
                    node_id,
                    load_state: InspectorLoadState::Loading,
                    breadcrumbs,
                    scroll_offset: 0,
                    selected_relation_idx: 0,
                });

                self.inspection_session.inspect(node_id.0.to_string());

                UpdateResult::InspectNode(node_id)
            }
            Action::NodeDetailsLoaded(model) => {
                if let Some(ref mut active) = self.active_inspector {
                    active.load_state = InspectorLoadState::Loaded(model);
                    active.selected_relation_idx = 0;
                }
                UpdateResult::Changed
            }
            Action::NodeDetailsFailed(err) => {
                if let Some(ref mut active) = self.active_inspector {
                    active.load_state = InspectorLoadState::Error(err);
                }
                UpdateResult::Changed
            }
            Action::PopBreadcrumb => {
                let mut go_back_to = None;
                if let Some(ref mut active) = self.active_inspector {
                    active.breadcrumbs.pop(); // Pop current
                    if let Some(prev) = active.breadcrumbs.last() {
                        go_back_to = Some(*prev);
                    }
                }
                self.inspection_session.go_back();
                if let Some(prev_node_id) = go_back_to {
                    // Update breadcrumbs manually to preserve stack
                    let mut breadcrumbs = Vec::new();
                    if let Some(ref active) = self.active_inspector {
                        breadcrumbs = active.breadcrumbs.clone();
                    }
                    self.active_inspector = Some(InspectorSession {
                        node_id: prev_node_id,
                        load_state: InspectorLoadState::Loading,
                        breadcrumbs,
                        scroll_offset: 0,
                        selected_relation_idx: 0,
                    });
                    UpdateResult::InspectNode(prev_node_id)
                } else {
                    self.mode = TuiMode::Conversation;
                    self.focus = FocusRegion::Editor;
                    self.active_inspector = None;
                    UpdateResult::Changed
                }
            }
            Action::CloseInspector => {
                self.mode = TuiMode::Conversation;
                self.focus = FocusRegion::Editor;
                // Setting active_inspector to None also clears breadcrumbs,
                // which are volatile and scoped to the active Inspector session (RFC-006 §4).
                self.active_inspector = None;
                self.inspection_session = crate::ui::view_models::InspectionSession::default();
                // Resume auto-follow so the timeline scrolls to bottom on next message.
                self.viewport.follow_tail = true;
                self.recalculate_viewport();
                UpdateResult::Changed
            }
            Action::ScrollInspectorUp(lines) => {
                if let Some(ref mut active) = self.active_inspector {
                    active.scroll_offset = active.scroll_offset.saturating_sub(lines);
                }
                UpdateResult::Changed
            }
            Action::ScrollInspectorDown(lines) => {
                if let Some(ref mut active) = self.active_inspector {
                    active.scroll_offset += lines;
                }
                UpdateResult::Changed
            }
            Action::NextInspectorRelation => {
                if let Some(ref mut active) = self.active_inspector {
                    if let InspectorLoadState::Loaded(ref model) = active.load_state {
                        if !model.relationships.is_empty()
                            && active.selected_relation_idx < model.relationships.len() - 1
                        {
                            active.selected_relation_idx += 1;
                        }
                    }
                }
                UpdateResult::Changed
            }
            Action::PrevInspectorRelation => {
                if let Some(ref mut active) = self.active_inspector {
                    if active.selected_relation_idx > 0 {
                        active.selected_relation_idx -= 1;
                    }
                }
                UpdateResult::Changed
            }
            Action::TraverseToRelation => {
                let mut target_id = None;
                if let Some(ref active) = self.active_inspector {
                    if let InspectorLoadState::Loaded(ref model) = active.load_state {
                        if active.selected_relation_idx < model.relationships.len() {
                            target_id = Some(
                                model.relationships[active.selected_relation_idx]
                                    .target_id
                                    .clone(),
                            );
                        }
                    }
                }
                if let Some(uuid_str) = target_id {
                    if let Ok(parsed_uuid) = uuid::Uuid::parse_str(&uuid_str) {
                        return self.update_internal(Action::InspectNode(brain_domain::NodeId(
                            parsed_uuid,
                        )));
                    }
                }
                UpdateResult::NoChange
            }
            Action::ScrollUp(lines) => {
                self.viewport.scroll_offset = self.viewport.scroll_offset.saturating_sub(lines);
                self.viewport.follow_tail = false;
                UpdateResult::Changed
            }
            Action::ScrollDown(lines) => {
                self.viewport.scroll_offset += lines;
                self.viewport.follow_tail = false;
                UpdateResult::Changed
            }
            Action::JumpToTop => {
                self.viewport.scroll_offset = 0;
                self.viewport.follow_tail = false;
                UpdateResult::Changed
            }
            Action::JumpToBottom(total_rows) => {
                self.viewport.scroll_offset = total_rows.saturating_sub(1);
                self.viewport.follow_tail = false;
                UpdateResult::Changed
            }
            Action::Resize(cols, rows) => {
                self.terminal_width = cols;
                self.terminal_height = rows;
                if cols < 80 && self.focus == FocusRegion::Sidebar {
                    self.focus = FocusRegion::Editor;
                }
                UpdateResult::Changed
            }
            Action::DeleteSession(session_id) => {
                if let Some(ref pending) = self.pending_load {
                    if pending.session_id == session_id {
                        self.clear_pending_load();
                    }
                }
                if let Some(idx) = self.sessions.iter().position(|s| s.id == session_id) {
                    self.sessions.remove(idx);
                }
                if self.session_id == session_id {
                    if !self.sessions.is_empty() {
                        if self.selected_session_idx >= self.sessions.len() {
                            self.selected_session_idx = self.sessions.len() - 1;
                        }
                        let next_id = self.sessions[self.selected_session_idx].id;
                        self.session_id = next_id;
                        self.active_messages.clear();
                        self.clear_pending_load();
                        UpdateResult::LoadSession(next_id)
                    } else {
                        self.session_id = SessionId::new();
                        self.session_title = "New Conversation".to_string();
                        self.active_messages.clear();
                        self.selected_session_idx = 0;
                        self.clear_pending_load();
                        UpdateResult::Changed
                    }
                } else {
                    if self.selected_session_idx >= self.sessions.len() && !self.sessions.is_empty()
                    {
                        self.selected_session_idx = self.sessions.len() - 1;
                    }
                    UpdateResult::Changed
                }
            }
            Action::ToolCallRequested {
                message,
                call_id,
                tool_id,
                arguments,
                requires_approval,
            } => {
                if self.active_tool_calls.iter().any(|t| t.call_id == call_id) {
                    return UpdateResult::NoChange;
                }
                if self
                    .message_tool_calls
                    .values()
                    .any(|list| list.iter().any(|t| t.call_id == call_id))
                {
                    return UpdateResult::NoChange;
                }

                let mut new_execution = crate::ui::command::tool::ToolExecution::new(
                    message,
                    call_id.clone(),
                    tool_id.clone(),
                );
                if requires_approval {
                    let approval = crate::ui::command::tool::ToolApproval {
                        message_id: message,
                        call_id: call_id.clone(),
                        tool_id,
                        arguments,
                    };
                    self.pending_approvals.push(approval);
                } else {
                    new_execution.status = crate::ui::command::tool::ToolExecutionStatus::Approved;
                }
                self.active_tool_calls.push(new_execution);
                self.timeline.push((
                    crate::ui::interaction::timeline::EventOrdinal(self.next_ordinal),
                    crate::ui::interaction::timeline::TimelineItem::ToolExecution(call_id),
                ));
                self.next_ordinal += 1;
                UpdateResult::Changed
            }
            Action::ToolProgressReceived {
                message: _,
                call_id,
                sequence,
                detail,
                log_message,
            } => {
                if let Some(tool) = self
                    .active_tool_calls
                    .iter_mut()
                    .find(|t| t.call_id == call_id)
                {
                    if tool.status.is_terminal() {
                        return UpdateResult::NoChange;
                    }
                    if sequence <= tool.protocol_state.last_sequence {
                        return UpdateResult::NoChange;
                    }
                    tool.protocol_state.last_sequence = sequence;
                    tool.status =
                        crate::ui::command::tool::ToolExecutionStatus::Running { progress: detail };
                    if !log_message.is_empty() {
                        tool.logs.push(crate::ui::command::tool::ToolLogEntry {
                            timestamp: SystemTime::now(),
                            message: log_message,
                        });
                    }
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::ToolResultReceived {
                message,
                call_id,
                result,
                is_error,
            } => {
                if let Some(pos) = self
                    .active_tool_calls
                    .iter()
                    .position(|t| t.call_id == call_id)
                {
                    let mut tool = self.active_tool_calls.remove(pos);
                    if is_error {
                        tool.status =
                            crate::ui::command::tool::ToolExecutionStatus::Failed { error: result };
                    } else {
                        tool.status =
                            crate::ui::command::tool::ToolExecutionStatus::Completed { result };
                    }
                    self.message_tool_calls
                        .entry(message)
                        .or_default()
                        .push(tool);
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::ApproveToolCall { call_id, approved } => {
                self.pending_approvals.retain(|a| a.call_id != call_id);
                if let Some(pos) = self
                    .active_tool_calls
                    .iter()
                    .position(|t| t.call_id == call_id)
                {
                    if approved {
                        self.active_tool_calls[pos].status =
                            crate::ui::command::tool::ToolExecutionStatus::Approved;
                    } else {
                        let mut tool = self.active_tool_calls.remove(pos);
                        tool.status = crate::ui::command::tool::ToolExecutionStatus::Denied;
                        let msg_id = tool.message_id;
                        self.message_tool_calls
                            .entry(msg_id)
                            .or_default()
                            .push(tool);
                    }
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::RetrievalStarted { message, query } => {
                let _ = (message, query);
                UpdateResult::Changed
            }
            Action::RetrievalReceived { message, info } => {
                let id = info.id;
                self.retrievals.insert(id, info);
                self.message_retrievals.entry(message).or_default().push(id);
                self.timeline.push((
                    crate::ui::interaction::timeline::EventOrdinal(self.next_ordinal),
                    crate::ui::interaction::timeline::TimelineItem::Retrieval(id),
                ));
                self.next_ordinal += 1;
                UpdateResult::Changed
            }
            Action::RetrievalCompleted { message } => {
                let _ = message;
                UpdateResult::Changed
            }
            Action::PinCurrentNode => {
                if let Some(ref active) = self.active_inspector {
                    if let InspectorLoadState::Loaded(ref model) = active.load_state {
                        let node_id = active.node_id;
                        let node_label = model.entity.label.clone();
                        let node_type: brain_domain::NodeType = model
                            .entity
                            .node_type
                            .parse()
                            .unwrap_or(brain_domain::NodeKind::Unknown);

                        // Deduplication guard: if already pinned, toggle off (unpin).
                        // A second PinCurrentNode on the same node is treated as "unpin",
                        // consistent with the Phase 2 toggle UX. Programmatic callers that
                        // must not unpin should check pinned_nodes before dispatching.
                        if let Some(pos) = self
                            .pinned_nodes
                            .iter()
                            .position(|pn| pn.node_id == node_id)
                        {
                            self.pinned_nodes.remove(pos);
                            self.transient_message = Some((
                                format!("Unpinned: {}", node_label),
                                std::time::Instant::now(),
                            ));
                        } else {
                            self.pinned_nodes.push(PinnedNode {
                                node_id,
                                label: node_label.clone(),
                                node_type,
                                pinned_at: self.pinned_nodes.len(),
                            });
                            self.transient_message = Some((
                                format!("Pinned: {}", node_label),
                                std::time::Instant::now(),
                            ));
                        }
                        UpdateResult::Changed
                    } else {
                        UpdateResult::NoChange
                    }
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::UnpinNode(node_id) => {
                if let Some(pos) = self
                    .pinned_nodes
                    .iter()
                    .position(|pn| pn.node_id == node_id)
                {
                    let label = self.pinned_nodes[pos].label.clone();
                    self.pinned_nodes.remove(pos);
                    self.transient_message =
                        Some((format!("Unpinned: {}", label), std::time::Instant::now()));
                    if !self.pinned_nodes.is_empty()
                        && self.pinned_overlay_cursor >= self.pinned_nodes.len()
                    {
                        self.pinned_overlay_cursor = self.pinned_nodes.len() - 1;
                    }
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::ClearAllPins => {
                self.pinned_nodes.clear();
                self.pinned_overlay_cursor = 0;
                self.transient_message = Some((
                    "Cleared all pinned context".to_string(),
                    std::time::Instant::now(),
                ));
                UpdateResult::Changed
            }
            Action::OpenPinnedOverlay => {
                self.overlay = TuiOverlay::PinnedContext;
                self.pinned_overlay_cursor = 0;
                UpdateResult::Changed
            }
            Action::ClosePinnedOverlay => {
                self.overlay = TuiOverlay::None;
                UpdateResult::Changed
            }
            Action::PinnedOverlayUp => {
                if !self.pinned_nodes.is_empty() && self.pinned_overlay_cursor > 0 {
                    self.pinned_overlay_cursor -= 1;
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::PinnedOverlayDown => {
                if !self.pinned_nodes.is_empty()
                    && self.pinned_overlay_cursor < self.pinned_nodes.len() - 1
                {
                    self.pinned_overlay_cursor += 1;
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::InspectPinnedNode(idx) => {
                if idx < self.pinned_nodes.len() {
                    let node_id = self.pinned_nodes[idx].node_id;
                    self.overlay = TuiOverlay::None;
                    self.mode = TuiMode::Exploration;
                    self.focus = FocusRegion::Inspector;
                    self.active_inspector = Some(InspectorSession {
                        node_id,
                        load_state: InspectorLoadState::Loading,
                        breadcrumbs: vec![],
                        scroll_offset: 0,
                        selected_relation_idx: 0,
                    });
                    UpdateResult::InspectNode(node_id)
                } else {
                    UpdateResult::NoChange
                }
            }
            Action::ToggleSubmitWithWorkspace => {
                // No-op when the workspace is empty — the indicator would be
                // meaningless and sending an empty context_used confuses the protocol.
                if self.pinned_nodes.is_empty() {
                    UpdateResult::NoChange
                } else {
                    self.submit_with_workspace = !self.submit_with_workspace;
                    UpdateResult::Changed
                }
            }
            Action::ResetSubmitWithWorkspace => {
                // Called by lib.rs only after client.execute() returns Ok.
                // Keeps the flag alive through failed dispatches so the user
                // can retry without re-toggling.
                self.submit_with_workspace = false;
                UpdateResult::Changed
            }
            Action::SetTransientMessage(msg) => {
                self.transient_message = Some((msg, std::time::Instant::now()));
                UpdateResult::Changed
            }
            Action::LoadMemories(summaries) => {
                let vms = summaries
                    .iter()
                    .map(crate::ui::view_models::MemoryItemViewModel::from_summary)
                    .collect();
                self.memory_results_vm.update_items(vms);
                self.memory_results_vm.set_active(true);
                UpdateResult::Changed
            }
            Action::MemoryListFailed(err) => {
                self.transient_message = Some((
                    format!("Memory list failed: {}", err),
                    std::time::Instant::now(),
                ));
                UpdateResult::Changed
            }
            Action::PinMemory(id) => {
                self.memory_results_vm.optimistic_pin(&id);
                self.transient_message =
                    Some((format!("Pinned memory: {}", id), std::time::Instant::now()));
                UpdateResult::Changed
            }
            Action::UnpinMemory(id) => {
                self.memory_results_vm.optimistic_unpin(&id);
                self.transient_message = Some((
                    format!("Unpinned memory: {}", id),
                    std::time::Instant::now(),
                ));
                UpdateResult::Changed
            }
            Action::ArchiveMemory(id) => {
                self.memory_results_vm.optimistic_archive(&id);
                self.transient_message = Some((
                    format!("Archived memory: {}", id),
                    std::time::Instant::now(),
                ));
                UpdateResult::Changed
            }
            Action::RestoreMemory(id) => {
                self.memory_results_vm.optimistic_restore(&id);
                self.transient_message = Some((
                    format!("Restored memory: {}", id),
                    std::time::Instant::now(),
                ));
                UpdateResult::Changed
            }
            Action::RollbackMemoryMutation(previous) => {
                let id = previous.id.clone();
                self.memory_results_vm.rollback_item(previous);
                self.transient_message = Some((
                    format!("Reconciled memory mutation failure for {}", id),
                    std::time::Instant::now(),
                ));
                UpdateResult::Changed
            }
            Action::ReasoningPlanGenerated(plan) => {
                let vm = crate::ui::view_models::ReasoningPlanViewModel::from_domain(&plan);
                self.transient_message = Some((
                    format!(
                        "Generated reasoning plan '{}' ({} steps)",
                        vm.plan_id, vm.total_steps
                    ),
                    std::time::Instant::now(),
                ));
                self.reasoning_plan_vm = Some(vm);
                UpdateResult::Changed
            }
        }
    }
    /// Commits the active response typewriter buffer into active_messages history.
    pub fn commit_active_response(&mut self) {
        if !self.active_response.is_empty() {
            let final_content = self.active_response.clone();
            let assistant_msg = brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::Assistant,
                final_content,
            );
            self.active_messages.push(assistant_msg.clone());
            self.session_histories
                .entry(self.session_id)
                .or_default()
                .push(assistant_msg);

            let assistant_msg_id =
                crate::ui::interaction::MessageId(self.active_messages.len() as u64);

            // Replace MessageId(0) placeholder in timeline with committed assistant ID
            for (_, item) in &mut self.timeline {
                if let crate::ui::interaction::timeline::TimelineItem::Message(id) = item {
                    if id.0 == 0 {
                        *id = assistant_msg_id;
                    }
                }
            }

            // Move tools and retrievals from MessageId(0) key to assistant_msg_id key
            if let Some(tools) = self
                .message_tool_calls
                .remove(&crate::ui::interaction::MessageId(0))
            {
                let mut updated_tools = tools;
                for t in &mut updated_tools {
                    t.message_id = assistant_msg_id;
                }
                self.message_tool_calls
                    .insert(assistant_msg_id, updated_tools);
            }
            if let Some(retrievals) = self
                .message_retrievals
                .remove(&crate::ui::interaction::MessageId(0))
            {
                self.message_retrievals.insert(assistant_msg_id, retrievals);
            }
        } else {
            // Prune stranded MessageId(0) timeline placeholder
            self.timeline.retain(|(_, item)| {
                !matches!(item, crate::ui::interaction::timeline::TimelineItem::Message(id) if id.0 == 0)
            });
        }
        self.active_response.clear();
        self.active_response_revision += 1;
    }

    /// Computes the responsive layouts and returns (sidebar_width, chat_width, inspector_width).
    pub fn panel_widths(&self) -> (u16, u16, u16) {
        let c = self.terminal_width;
        match self.mode {
            TuiMode::Conversation => {
                if c > 70 {
                    (25, c.saturating_sub(25), 0)
                } else {
                    (0, c, 0)
                }
            }
            TuiMode::Exploration => {
                if c >= 105 {
                    (20, 50, c.saturating_sub(70))
                } else if c >= 85 {
                    (0, c.saturating_sub(35), 35)
                } else {
                    (0, 0, c)
                }
            }
            TuiMode::RuntimeDashboard => (0, c, 0),
            TuiMode::Explainability => (0, c, 0),
            TuiMode::InteractiveReflection => (0, c, 0),
            TuiMode::KnowledgeEvolution => (0, c, 0),
            TuiMode::KnowledgeAutomation => (0, c, 0),
        }
    }

    /// Recalculates the scroll viewport boundaries and clamps or scrolls to bottom based on follow_tail.
    pub fn recalculate_viewport(&mut self) {
        if self.terminal_width == 0 || self.terminal_height == 0 {
            return;
        }
        let (_, chat_w, _) = self.panel_widths();
        let chat_width = chat_w.saturating_sub(2) as usize; // 2 borders

        let mid_height = self.terminal_height.saturating_sub(3 + 3 + 1); // 3 header + 3 editor + 1 status
        let viewport_height = mid_height.saturating_sub(2) as usize; // 2 borders

        // Build the blocks list to find total height
        let blocks = self.build_timeline_blocks(chat_width);

        let mut total_height = 0;
        for block in &blocks {
            let h = if block.header.is_some() {
                1 + block.visual_lines.len() + 1
            } else {
                block.visual_lines.len() + 1
            };
            total_height += h;
        }

        let max_scroll = total_height.saturating_sub(viewport_height);

        if self.viewport.follow_tail {
            self.viewport.scroll_offset = max_scroll;
        } else {
            if self.viewport.scroll_offset >= max_scroll {
                self.viewport.scroll_offset = max_scroll;
                self.viewport.follow_tail = true;
            }
        }
    }

    /// Build timeline blocks formatted to the current column width.
    pub fn build_timeline_blocks(&self, chat_width: usize) -> Vec<TimelineBlock> {
        let highlighter = crate::ui::interaction::markdown::KeywordSyntaxHighlighter::new();
        let mut blocks = Vec::new();

        for (_, item) in &self.timeline {
            match item {
                crate::ui::interaction::timeline::TimelineItem::Message(msg_id) => {
                    if msg_id.0 == 0 {
                        if !self.active_response.is_empty() || self.is_generating() {
                            let ast = crate::ui::interaction::markdown::MarkdownParser::parse(
                                &self.active_response,
                            );
                            let visual_lines =
                                crate::ui::interaction::markdown::MarkdownLayout::layout(
                                    &ast,
                                    chat_width,
                                    &highlighter,
                                );
                            blocks.push(TimelineBlock {
                                header: Some("Brain".to_string()),
                                visual_lines,
                            });
                        }
                    } else {
                        let idx = (msg_id.0 - 1) as usize;
                        if idx < self.active_messages.len() {
                            let msg = &self.active_messages[idx];
                            let sender = match msg.role {
                                brain_domain::MessageRole::User => "You".to_string(),
                                brain_domain::MessageRole::Assistant => "Brain".to_string(),
                                brain_domain::MessageRole::System => "System".to_string(),
                            };
                            let ast = crate::ui::interaction::markdown::MarkdownParser::parse(
                                &msg.content,
                            );
                            let visual_lines =
                                crate::ui::interaction::markdown::MarkdownLayout::layout(
                                    &ast,
                                    chat_width,
                                    &highlighter,
                                );
                            blocks.push(TimelineBlock {
                                header: Some(sender),
                                visual_lines,
                            });
                        }
                    }
                }
                crate::ui::interaction::timeline::TimelineItem::ToolExecution(ref call_id) => {
                    let tool_opt = self
                        .active_tool_calls
                        .iter()
                        .find(|t| t.call_id == *call_id)
                        .or_else(|| {
                            self.message_tool_calls
                                .values()
                                .flatten()
                                .find(|t| &t.call_id == call_id)
                        });
                    if let Some(tool) = tool_opt {
                        let expanded = self
                            .conversation_view
                            .expanded_tool_sections
                            .get(&tool.call_id);
                        blocks.push(TimelineBlock {
                            header: None,
                            visual_lines: format_tool_execution(tool, expanded),
                        });
                    } else {
                        blocks.push(TimelineBlock {
                            header: None,
                            visual_lines: vec![crate::ui::interaction::markdown::VisualLine {
                                kind: crate::ui::interaction::markdown::VisualLineKind::Text,
                                spans: vec![crate::ui::interaction::markdown::VisualSpan::new(
                                    "🔧 Tool: [Loading...]".to_string(),
                                    crate::ui::interaction::markdown::VisualStyle::Normal,
                                )],
                            }],
                        });
                    }
                }
                crate::ui::interaction::timeline::TimelineItem::Retrieval(ref retrieval_id) => {
                    if let Some(retrieval) = self.retrievals.get(retrieval_id) {
                        blocks.push(TimelineBlock {
                            header: None,
                            visual_lines: format_retrieval_info(retrieval),
                        });
                    } else {
                        blocks.push(TimelineBlock {
                            header: None,
                            visual_lines: vec![crate::ui::interaction::markdown::VisualLine {
                                kind: crate::ui::interaction::markdown::VisualLineKind::Text,
                                spans: vec![crate::ui::interaction::markdown::VisualSpan::new(
                                    "🧠 Memory: [Loading...]".to_string(),
                                    crate::ui::interaction::markdown::VisualStyle::Normal,
                                )],
                            }],
                        });
                    }
                }
            }
        }
        blocks
    }
}

fn format_tool_execution(
    tool: &crate::ui::command::tool::ToolExecution,
    expanded_sections: Option<
        &std::collections::HashSet<crate::ui::interaction::navigation::ToolSection>,
    >,
) -> Vec<crate::ui::interaction::markdown::VisualLine> {
    use crate::ui::command::tool::ToolExecutionStatus;
    use crate::ui::interaction::markdown::{VisualLine, VisualLineKind, VisualSpan, VisualStyle};
    use crate::ui::interaction::navigation::ToolSection;

    let mut lines = Vec::new();

    // Header line
    let tool_name = &tool.tool_id.0;
    let (status_text, style) = match &tool.status {
        ToolExecutionStatus::PendingApproval => ("Awaiting Approval", VisualStyle::Bold),
        ToolExecutionStatus::Approved => ("Approved", VisualStyle::Bold),
        ToolExecutionStatus::Denied => ("Denied", VisualStyle::Normal),
        ToolExecutionStatus::Running { .. } => ("Running", VisualStyle::Bold),
        ToolExecutionStatus::Completed { .. } => ("Completed", VisualStyle::Bold),
        ToolExecutionStatus::Failed { .. } => ("Failed", VisualStyle::Normal),
    };

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![VisualSpan::new(
            format!("🔧 Tool: {} [ {} ]", tool_name, status_text),
            style,
        )],
    });

    // Progress/details line
    match &tool.status {
        ToolExecutionStatus::Running { progress } => {
            let progress_text = match progress {
                brain_core::events::ToolProgressDetail::Determinate {
                    completed,
                    total,
                    unit: _,
                } => {
                    let pct = if *total > 0 {
                        ((*completed as f64) / (*total as f64)) * 100.0
                    } else {
                        0.0
                    };
                    let fill = ((pct / 10.0) as usize).min(10);
                    let empty = 10 - fill;
                    let bar = format!("[{}{}]", "█".repeat(fill), "░".repeat(empty));
                    format!("  Progress: {} {:.1}%", bar, pct)
                }
                brain_core::events::ToolProgressDetail::Indeterminate => "  Running...".to_string(),
            };

            lines.push(VisualLine {
                kind: VisualLineKind::Text,
                spans: vec![VisualSpan::new(progress_text, VisualStyle::Normal)],
            });
        }
        ToolExecutionStatus::Failed { error } => {
            lines.push(VisualLine {
                kind: VisualLineKind::Text,
                spans: vec![VisualSpan::new(
                    format!("  Error: {}", error),
                    VisualStyle::Normal,
                )],
            });
        }
        _ => {}
    }

    // Lazy logs rendering
    let show_logs = expanded_sections
        .map(|s| s.contains(&ToolSection::Logs))
        .unwrap_or(false);
    if show_logs {
        for log in &tool.logs {
            lines.push(VisualLine {
                kind: VisualLineKind::Text,
                spans: vec![VisualSpan::new(
                    format!("    • {}", log.message),
                    VisualStyle::CodeComment,
                )],
            });
        }
    } else if !tool.logs.is_empty() {
        lines.push(VisualLine {
            kind: VisualLineKind::Text,
            spans: vec![VisualSpan::new(
                format!("    ▶ Logs collapsed ({} entries)", tool.logs.len()),
                VisualStyle::Normal,
            )],
        });
    }

    lines
}

fn format_retrieval_info(
    retrieval: &brain_domain::bkf::retrieval::RetrievalInfo,
) -> Vec<crate::ui::interaction::markdown::VisualLine> {
    use crate::ui::interaction::markdown::{VisualLine, VisualLineKind, VisualSpan, VisualStyle};
    use brain_domain::bkf::retrieval::RetrievalWeight;

    let mut lines = Vec::new();

    let weight_badge = match retrieval.explanation.weight {
        RetrievalWeight::Critical => "CRITICAL",
        RetrievalWeight::High => "HIGH",
        RetrievalWeight::Normal => "NORMAL",
    };

    let weight_style = match retrieval.explanation.weight {
        RetrievalWeight::Critical => VisualStyle::Bold,
        RetrievalWeight::High => VisualStyle::Bold,
        RetrievalWeight::Normal => VisualStyle::Normal,
    };

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![VisualSpan::new(
            format!("🧠 Memory: {} [ {} ]", retrieval.title, weight_badge),
            weight_style,
        )],
    });

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![VisualSpan::new(
            format!("  > {}", retrieval.excerpt),
            VisualStyle::Italic,
        )],
    });

    let keywords_str = retrieval.explanation.matched_keywords.join(", ");
    let recency_str = if retrieval.explanation.recency_boost {
        " (+recency boost)"
    } else {
        ""
    };

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![VisualSpan::new(
            format!(
                "  Matches: [{}] Similarity: {:?}{}",
                keywords_str, retrieval.explanation.semantic_similarity, recency_str
            ),
            VisualStyle::CodeComment,
        )],
    });

    let provenance_line = format!(
        "  Source: {:?} at {}",
        retrieval.explanation.provenance.kind, retrieval.explanation.provenance.location
    );
    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![VisualSpan::new(provenance_line, VisualStyle::Citation)],
    });

    lines
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presentation_model_visible_rows_slice() {
        let mut state = UiState::new();
        state.viewport.scroll_offset = 10;

        let model = state.presentation_model(50, 20);
        assert_eq!(model.visible_rows.len(), 20);
        assert_eq!(model.scroll_indicator, "Showing 11-30 of 50");

        state.update(Action::JumpToTop);
        assert_eq!(state.viewport.scroll_offset, 0);

        state.update(Action::JumpToBottom(50));
        assert_eq!(state.viewport.scroll_offset, 49);
    }

    #[test]
    fn test_editor_input_append() {
        let mut state = UiState::new();
        assert_eq!(state.editor.text(), "");
        assert_eq!(state.editor.cursor(), 0);

        let res = state.update(Action::InsertChar('a'));
        assert_eq!(res, UpdateResult::Changed);
        assert_eq!(state.editor.text(), "a");
        assert_eq!(state.editor.cursor(), 1);

        state.update(Action::InsertChar('b'));
        assert_eq!(state.editor.text(), "ab");
        assert_eq!(state.editor.cursor(), 2);

        state.update(Action::MoveCursorLeft);
        assert_eq!(state.editor.cursor(), 1);

        state.update(Action::InsertChar('c'));
        assert_eq!(state.editor.text(), "acb");
        assert_eq!(state.editor.cursor(), 2);

        state.update(Action::Backspace);
        assert_eq!(state.editor.text(), "ab");
        assert_eq!(state.editor.cursor(), 1);

        state.update(Action::Delete);
        assert_eq!(state.editor.text(), "a");
        assert_eq!(state.editor.cursor(), 1);
    }

    #[test]
    fn test_prompt_history_rotation_and_eviction() {
        let mut state = UiState::with_history_capacity(2);

        // Submit first prompt
        state.update(Action::InsertChar('a'));
        let res1 = state.update(Action::SubmitPrompt);
        assert_eq!(res1, UpdateResult::PromptSubmitted("a".to_string()));
        assert_eq!(state.editor.text(), "");
        state.generation_state = GenerationState::Idle;

        // Submit second prompt
        state.update(Action::InsertChar('b'));
        state.update(Action::SubmitPrompt);
        state.generation_state = GenerationState::Idle;

        // Submit third prompt (evicts 'a' since capacity is 2)
        state.update(Action::InsertChar('c'));
        state.update(Action::SubmitPrompt);
        state.generation_state = GenerationState::Idle;

        // Active typing draft
        state.update(Action::InsertChar('d'));
        state.update(Action::InsertChar('r'));

        // Scroll up once -> gets last history item ('c')
        state.update(Action::RecallPrevious);
        assert_eq!(state.editor.text(), "c");

        // Edit recalled history item
        state.update(Action::InsertChar('x'));
        assert_eq!(state.editor.text(), "cx");

        // Scroll up again -> gets previous item ('b')
        state.update(Action::RecallPrevious);
        assert_eq!(state.editor.text(), "b");

        // Scroll up again -> older items evicted, so stays at oldest ('b')
        state.update(Action::RecallPrevious);
        assert_eq!(state.editor.text(), "b");

        // Scroll down once -> goes next to 'c'
        state.update(Action::RecallNext);
        assert_eq!(state.editor.text(), "c");

        // Scroll down again -> returns to original cached draft "dr", discarding edits "cx"
        state.update(Action::RecallNext);
        assert_eq!(state.editor.text(), "dr");
    }

    #[test]
    fn test_incremental_tokenizer() {
        let mut tok = IncrementalTokenizer::new();
        let t1 = tok.push_chunk("Hello ");
        assert_eq!(t1, vec![RenderToken::Text("Hello ".to_string())]);

        let t2 = tok.push_chunk("world");
        assert!(t2.is_empty()); // Buffered incomplete word

        let t3 = tok.push_chunk("!");
        assert!(t3.is_empty()); // Still buffered

        let t4 = tok.push_chunk("\n");
        assert_eq!(t4, vec![RenderToken::Text("world!\n".to_string())]);

        let t5 = tok.push_chunk("fin");
        assert!(t5.is_empty());
        let t6 = tok.flush();
        assert_eq!(t6, vec![RenderToken::Text("fin".to_string())]);
    }

    #[test]
    fn test_typewriter_queue_pacing() {
        let mut q = TypewriterQueue::new();
        q.push(RenderToken::Text("a".to_string()));
        q.push(RenderToken::Text("b".to_string()));

        let t0 = Instant::now();
        // Initial tick: emits 1 token instantly
        let r1 = q.drain_for_tick(t0);
        assert_eq!(r1.emitted, vec![RenderToken::Text("a".to_string())]);
        assert!(!r1.finished);

        // Immediate subsequent tick with 0 elapsed: emits nothing
        let r2 = q.drain_for_tick(t0);
        assert!(r2.emitted.is_empty());

        // 35ms elapsed (above 30ms rate): emits 1 token
        let t1 = t0 + std::time::Duration::from_millis(35);
        let r3 = q.drain_for_tick(t1);
        assert_eq!(r3.emitted, vec![RenderToken::Text("b".to_string())]);
        assert!(!r3.finished); // Backend not finished yet

        q.finish_backend();
        let r4 = q.drain_for_tick(t1);
        assert!(r4.finished);
    }

    /// Regression: rapid ticks (10ms) between token pushes must not starve the queue.
    ///
    /// Previously `last_drained_at` was reset on every tick, so `elapsed` never
    /// reached the 30ms threshold and only the very first token was ever drained.
    #[test]
    fn test_typewriter_queue_no_starvation_under_rapid_ticks() {
        let mut q = TypewriterQueue::new();
        // Push 5 tokens before any ticks.
        for i in 0..5 {
            q.push(RenderToken::Text(format!("{} ", i)));
        }

        let t0 = Instant::now();
        // Simulate 10ms tick rate — 3 ticks before the first 30ms drain window.
        let r1 = q.drain_for_tick(t0); // is_first → 1 emitted
        let r2 = q.drain_for_tick(t0 + std::time::Duration::from_millis(10)); // 10ms < 30ms → 0
        let r3 = q.drain_for_tick(t0 + std::time::Duration::from_millis(20)); // 20ms < 30ms → 0
        let r4 = q.drain_for_tick(t0 + std::time::Duration::from_millis(32)); // 32ms >= 30ms → 1
        let r5 = q.drain_for_tick(t0 + std::time::Duration::from_millis(42)); // 10ms from last drain → 0
        let r6 = q.drain_for_tick(t0 + std::time::Duration::from_millis(65)); // ~33ms from last drain → 1

        assert_eq!(
            r1.emitted.len(),
            1,
            "first tick must drain 1 token immediately"
        );
        assert_eq!(
            r2.emitted.len(),
            0,
            "10ms tick must not drain (elapsed resets only on emit)"
        );
        assert_eq!(r3.emitted.len(), 0, "20ms tick must not drain");
        assert_eq!(r4.emitted.len(), 1, "32ms tick must drain 1 token");
        assert_eq!(r5.emitted.len(), 0, "10ms after drain must not drain again");
        assert_eq!(
            r6.emitted.len(),
            1,
            "33ms after last drain must drain again"
        );
    }

    /// Regression: queue that empties mid-stream and then receives more tokens
    /// must continue draining correctly after refill.
    #[test]
    fn test_typewriter_queue_intermittent_push() {
        let mut q = TypewriterQueue::new();
        q.push(RenderToken::Text("first ".to_string()));

        let t0 = Instant::now();
        // Drain the only token immediately.
        let r1 = q.drain_for_tick(t0);
        assert_eq!(r1.emitted.len(), 1);
        assert!(q.is_empty());

        // Several ticks pass with an empty queue.
        let r2 = q.drain_for_tick(t0 + std::time::Duration::from_millis(10));
        let r3 = q.drain_for_tick(t0 + std::time::Duration::from_millis(20));
        assert!(r2.emitted.is_empty());
        assert!(r3.emitted.is_empty());

        // New batch of tokens arrives after 50ms from t0.
        let t1 = t0 + std::time::Duration::from_millis(50);
        q.push(RenderToken::Text("second ".to_string()));
        q.push(RenderToken::Text("third ".to_string()));

        // First tick after refill: is_first is false but 50ms > 30ms → should drain.
        let r4 = q.drain_for_tick(t1);
        assert!(
            !r4.emitted.is_empty(),
            "queue refilled after 50ms gap must drain on next tick"
        );

        // Finish backend and flush remaining.
        q.finish_backend();
        let t2 = t1 + std::time::Duration::from_millis(35);
        let r5 = q.drain_for_tick(t2);
        // Any remaining tokens drained, queue empty, finished.
        assert!(
            r5.finished || r4.emitted.len() >= 2,
            "all tokens must eventually drain"
        );
    }

    #[test]
    fn test_reducer_streaming_lifecycle_invariants() {
        let mut state = UiState::new();
        assert_eq!(state.generation_state, GenerationState::Idle);

        // Start stream
        state.update(Action::StartStream);
        assert_eq!(state.generation_state, GenerationState::Starting);

        // Submitting prompt with empty editor while generating returns NoChange
        // (because the editor is empty, not because of a generation guard)
        let res = state.update(Action::SubmitPrompt);
        assert_eq!(res, UpdateResult::NoChange);

        // First token received
        state.update(Action::ReceiveToken(RenderToken::Text("A ".to_string())));
        assert!(matches!(
            state.generation_state,
            GenerationState::Streaming { .. }
        ));

        // Backend finishes but queue is not empty
        state.update(Action::FinishStream);
        // GenerationState must remain Streaming because typewriter still contains "A "
        assert!(matches!(
            state.generation_state,
            GenerationState::Streaming { .. }
        ));
        assert_eq!(state.active_response, "");

        // TypewriterTick drains and finishes.
        // With backend_finished=true, all queued tokens flush in a single tick.
        let now = Instant::now();
        state.update(Action::TypewriterTick(now));
        assert_eq!(state.active_response, "");
        assert_eq!(state.active_messages.len(), 1);
        assert_eq!(state.active_messages[0].content, "A ");
        assert_eq!(state.generation_state, GenerationState::Finished);
    }

    /// Regression: when backend_finished, drain_for_tick flushes all remaining
    /// tokens in one call instead of pacing at 30ms/token.
    #[test]
    fn test_typewriter_backend_finished_flushes_immediately() {
        let mut q = TypewriterQueue::new();
        for i in 0..20 {
            q.push(RenderToken::Text(format!("word{} ", i)));
        }

        let t0 = Instant::now();
        // First tick (is_first): drains 1 token only — backend not finished yet
        let r1 = q.drain_for_tick(t0);
        assert_eq!(r1.emitted.len(), 1);
        assert!(!r1.finished);

        // Mark backend as finished
        q.finish_backend();

        // Next tick: should flush ALL remaining 19 tokens immediately
        let t1 = t0 + std::time::Duration::from_millis(5); // only 5ms later
        let r2 = q.drain_for_tick(t1);
        assert_eq!(
            r2.emitted.len(),
            19,
            "backend_finished must flush all tokens"
        );
        assert!(r2.finished, "queue empty + backend_finished = finished");
    }

    /// Regression: submitting a new prompt while a previous stream is active
    /// must succeed (cancel-and-replace behavior).
    #[test]
    fn test_submit_while_generating_cancels_and_replaces() {
        let mut state = UiState::new();

        // Type and submit first query
        state.update(Action::InsertChar('A'));
        let res1 = state.update(Action::SubmitPrompt);
        assert!(matches!(res1, UpdateResult::PromptSubmitted(ref p) if p == "A"));
        assert_eq!(state.generation_state, GenerationState::Starting);
        assert!(
            state.editor.text().is_empty(),
            "prompt must be cleared on submit"
        );

        // Simulate streaming in progress
        state.update(Action::ReceiveToken(RenderToken::Text(
            "response ".to_string(),
        )));
        assert!(state.is_generating());

        // Type and submit second query while first is still streaming
        state.update(Action::InsertChar('B'));
        let res2 = state.update(Action::SubmitPrompt);
        assert!(
            matches!(res2, UpdateResult::PromptSubmitted(ref p) if p == "B"),
            "must accept new prompt even while generating"
        );
        assert_eq!(state.generation_state, GenerationState::Starting);
        assert!(
            state.editor.text().is_empty(),
            "prompt must be cleared on resubmit"
        );
        assert_eq!(state.active_response, "", "old response must be cleared");
        assert!(
            state.typewriter.is_empty(),
            "old typewriter queue must be cleared"
        );
    }

    #[test]
    fn test_session_switching_invariants() {
        let mut state = UiState::new();
        let session_a = SessionId::new();
        let session_b = SessionId::new();

        let summaries = vec![
            crate::client::SessionSummary {
                id: session_a,
                title: "Session A".to_string(),
                updated_at: SystemTime::now(),
                pinned: false,
                archived: false,
            },
            crate::client::SessionSummary {
                id: session_b,
                title: "Session B".to_string(),
                updated_at: SystemTime::now(),
                pinned: false,
                archived: false,
            },
        ];

        // Load sessions list
        state.update(Action::LoadSessions(summaries));
        assert_eq!(state.sessions.len(), 2);
        assert_eq!(state.selected_session_idx, 0); // selected defaults to first

        // Focus sidebar and navigate selected cursor row
        state.update(Action::ToggleFocus);
        assert_eq!(state.focus, FocusRegion::Sidebar);
        state.update(Action::MoveSidebarCursorDown);
        assert_eq!(state.selected_session_idx, 1);

        // Enter triggers activation on B
        let req_1 = LoadRequestId(1);
        let res = state.update(Action::ActivateSession {
            session_id: session_b,
            request_id: req_1,
        });
        assert_eq!(res, UpdateResult::Changed);
        assert_eq!(
            state.pending_load,
            Some(PendingLoad {
                session_id: session_b,
                request_id: req_1
            })
        );
        assert_eq!(state.session_load_state, SessionLoadState::Loading);

        // Previous session remains active during load
        assert_ne!(state.session_id, session_b);

        // Load completes with matching request id
        let messages = vec![];
        let res2 = state.update(Action::SessionLoaded {
            session_id: session_b,
            request_id: req_1,
            messages,
        });
        assert_eq!(res2, UpdateResult::Changed);
        assert_eq!(state.session_id, session_b);
        assert_eq!(state.session_title, "Session B");
        assert_eq!(state.pending_load, None);
        assert_eq!(state.session_load_state, SessionLoadState::Loaded(vec![]));
    }

    #[test]
    fn test_terminal_paths_clear_pending_load() {
        let session_a = SessionId::new();
        let req_id = LoadRequestId(42);

        // Path 1: Successful Load
        let mut state = UiState::new();
        state.update(Action::ActivateSession {
            session_id: session_a,
            request_id: req_id,
        });
        assert!(state.pending_load.is_some());
        state.update(Action::SessionLoaded {
            session_id: session_a,
            request_id: req_id,
            messages: vec![],
        });
        assert_eq!(state.pending_load, None);

        // Path 2: Failed Load
        state.update(Action::ActivateSession {
            session_id: session_a,
            request_id: req_id,
        });
        assert!(state.pending_load.is_some());
        state.update(Action::SessionLoadFailed {
            session_id: session_a,
            request_id: req_id,
            error: "fail".to_string(),
        });
        assert_eq!(state.pending_load, None);

        // Path 3: Deleted Pending Session
        state.update(Action::ActivateSession {
            session_id: session_a,
            request_id: req_id,
        });
        assert!(state.pending_load.is_some());
        state.update(Action::DeleteSession(session_a));
        assert_eq!(state.pending_load, None);

        // Path 4: Shutdown / Quit
        state.update(Action::ActivateSession {
            session_id: session_a,
            request_id: req_id,
        });
        assert!(state.pending_load.is_some());
        state.update(Action::Quit);
        assert_eq!(state.pending_load, None);
    }

    #[test]
    fn test_immediate_cancellation_preserves_timeline_order() {
        let mut state = UiState::new();

        // 1. Submit prompt
        state.editor.insert('a');
        let res = state.update(Action::SubmitPrompt);
        assert!(matches!(res, UpdateResult::PromptSubmitted(_)));

        // Expect timeline has: User message
        assert_eq!(state.timeline.len(), 1);

        // Receive a token, which creates the placeholder MessageId(0)
        state.update(Action::ReceiveToken(crate::state::RenderToken::Text(
            "hello".to_string(),
        )));
        assert_eq!(state.timeline.len(), 2);
        assert!(matches!(
            state.timeline[1].1,
            crate::ui::interaction::timeline::TimelineItem::Message(
                crate::ui::interaction::MessageId(0)
            )
        ));

        // 2. Cancel stream immediately with empty active_response (simulate cancel before typewriter flushes)
        state.update(Action::CancelStream);

        // Verify the MessageId(0) placeholder is pruned because active_response is empty
        assert_eq!(state.timeline.len(), 1);
        assert!(matches!(
            state.timeline[0].1,
            crate::ui::interaction::timeline::TimelineItem::Message(
                crate::ui::interaction::MessageId(1)
            )
        ));

        // 3. Submit a second prompt
        state.editor.insert('b');
        let res2 = state.update(Action::SubmitPrompt);
        assert!(matches!(res2, UpdateResult::PromptSubmitted(_)));

        // Expect: timeline has MessageId(1) (user prompt 1) and MessageId(2) (user prompt 2)
        assert_eq!(state.timeline.len(), 2);
        assert!(matches!(
            state.timeline[0].1,
            crate::ui::interaction::timeline::TimelineItem::Message(
                crate::ui::interaction::MessageId(1)
            )
        ));
        assert!(matches!(
            state.timeline[1].1,
            crate::ui::interaction::timeline::TimelineItem::Message(
                crate::ui::interaction::MessageId(2)
            )
        ));

        // Receive token for second prompt response
        state.update(Action::ReceiveToken(crate::state::RenderToken::Text(
            "world".to_string(),
        )));
        assert_eq!(state.timeline.len(), 3);
        assert!(matches!(
            state.timeline[2].1,
            crate::ui::interaction::timeline::TimelineItem::Message(
                crate::ui::interaction::MessageId(0)
            )
        ));
    }

    #[test]
    fn test_pin_toggle_current_node() {
        let mut state = UiState::new();
        let node_id = brain_domain::NodeId(uuid::Uuid::new_v4());

        // 1. Trying to pin when no inspector is active should be a no-op
        let res = state.update(Action::PinCurrentNode);
        assert_eq!(res, UpdateResult::NoChange);
        assert!(state.pinned_nodes.is_empty());

        // 2. Set active inspector in Exploration mode and loaded state
        state.mode = TuiMode::Exploration;
        let model = brain_domain::query::inspector::InspectorModel {
            entity: brain_domain::dtos::NodeDTO::new(
                node_id.to_string(),
                "Episodic Memory".to_string(),
                "concept".to_string(),
                serde_json::Value::Null,
            ),
            metadata: std::collections::HashMap::new(),
            relationships: vec![],
            provenance: brain_domain::query::inspector::ProvenanceDTO {
                source: "Source".to_string(),
                location: "Location".to_string(),
                timestamp: 0,
                extra_info: std::collections::HashMap::new(),
            },
            retrieval_explanation: None,
            recent_activity: vec![],
        };
        state.active_inspector = Some(InspectorSession {
            node_id,
            load_state: InspectorLoadState::Loaded(Box::new(model)),
            breadcrumbs: vec![],
            scroll_offset: 0,
            selected_relation_idx: 0,
        });

        // 3. Pin it
        let res = state.update(Action::PinCurrentNode);
        assert_eq!(res, UpdateResult::Changed);
        assert_eq!(state.pinned_nodes.len(), 1);
        assert_eq!(state.pinned_nodes[0].node_id, node_id);
        assert_eq!(state.pinned_nodes[0].label, "Episodic Memory");
        assert_eq!(
            state.pinned_nodes[0].node_type,
            brain_domain::NodeKind::Concept
        );
        assert!(state.transient_message.is_some());
        assert!(state
            .transient_message
            .as_ref()
            .unwrap()
            .0
            .contains("Pinned: Episodic Memory"));

        // 4. Pin again -> should toggle off (unpin)
        let res = state.update(Action::PinCurrentNode);
        assert_eq!(res, UpdateResult::Changed);
        assert!(state.pinned_nodes.is_empty());
        assert!(state
            .transient_message
            .as_ref()
            .unwrap()
            .0
            .contains("Unpinned: Episodic Memory"));
    }

    #[test]
    fn test_pinned_overlay_transitions() {
        let mut state = UiState::new();
        assert_eq!(state.overlay, TuiOverlay::None);
        assert_eq!(state.mode, TuiMode::Conversation);

        // Open Pinned Overlay
        let res = state.update(Action::OpenPinnedOverlay);
        assert_eq!(res, UpdateResult::Changed);
        assert_eq!(state.overlay, TuiOverlay::PinnedContext);
        assert_eq!(state.mode, TuiMode::Conversation); // mode is unchanged (Refinement 1)

        // Close Pinned Overlay
        let res = state.update(Action::ClosePinnedOverlay);
        assert_eq!(res, UpdateResult::Changed);
        assert_eq!(state.overlay, TuiOverlay::None);
    }

    #[test]
    fn test_clear_all_pins() {
        let mut state = UiState::new();
        state.pinned_nodes.push(PinnedNode {
            node_id: brain_domain::NodeId(uuid::Uuid::new_v4()),
            label: "Test".to_string(),
            node_type: brain_domain::NodeKind::Tool,
            pinned_at: 0,
        });
        assert_eq!(state.pinned_nodes.len(), 1);

        let res = state.update(Action::ClearAllPins);
        assert_eq!(res, UpdateResult::Changed);
        assert!(state.pinned_nodes.is_empty());
        assert!(state.transient_message.unwrap().0.contains("Cleared"));
    }

    #[test]
    fn test_inspect_pinned_node_starts_fresh() {
        let mut state = UiState::new();
        let target_node_id = brain_domain::NodeId(uuid::Uuid::new_v4());
        state.pinned_nodes.push(PinnedNode {
            node_id: target_node_id,
            label: "Pinned Tool".to_string(),
            node_type: brain_domain::NodeKind::Tool,
            pinned_at: 0,
        });

        // Set state to overlay and some pre-existing active inspector session (e.g. from previous exploration)
        state.overlay = TuiOverlay::PinnedContext;
        state.mode = TuiMode::Exploration;
        state.active_inspector = Some(InspectorSession {
            node_id: brain_domain::NodeId(uuid::Uuid::new_v4()),
            load_state: InspectorLoadState::Loading,
            breadcrumbs: vec![brain_domain::NodeId(uuid::Uuid::new_v4())], // pre-existing history
            scroll_offset: 2,
            selected_relation_idx: 1,
        });

        // Inspect pinned node at index 0
        let res = state.update(Action::InspectPinnedNode(0));
        assert_eq!(res, UpdateResult::InspectNode(target_node_id));

        // Verify overlay is closed, mode is Exploration, and inspector session is fresh without breadcrumbs (Refinement 6)
        assert_eq!(state.overlay, TuiOverlay::None);
        assert_eq!(state.mode, TuiMode::Exploration);
        let active = state.active_inspector.unwrap();
        assert_eq!(active.node_id, target_node_id);
        assert!(active.breadcrumbs.is_empty()); // Fresh session, no restored breadcrumbs
        assert_eq!(active.scroll_offset, 0);
        assert_eq!(active.selected_relation_idx, 0);
    }

    // --- RFC-007 Active Workspace tests ---

    fn make_pinned_state() -> UiState {
        let mut state = UiState::new();
        state.pinned_nodes.push(PinnedNode {
            node_id: brain_domain::NodeId(uuid::Uuid::new_v4()),
            label: "Alpha".to_string(),
            node_type: brain_domain::NodeKind::Concept,
            pinned_at: 0,
        });
        state
    }

    #[test]
    fn test_toggle_submit_with_workspace_empty() {
        // Toggling when workspace is empty must be a no-op.
        let mut state = UiState::new();
        assert!(state.pinned_nodes.is_empty());
        let res = state.update(Action::ToggleSubmitWithWorkspace);
        assert_eq!(res, UpdateResult::NoChange);
        assert!(!state.submit_with_workspace);
    }

    #[test]
    fn test_toggle_submit_with_workspace_with_pins() {
        // Toggling when workspace has nodes must set the flag.
        let mut state = make_pinned_state();
        assert!(!state.submit_with_workspace);
        let res = state.update(Action::ToggleSubmitWithWorkspace);
        assert_eq!(res, UpdateResult::Changed);
        assert!(state.submit_with_workspace);
    }

    #[test]
    fn test_toggle_submit_with_workspace_double() {
        // Toggling twice must return the flag to false.
        let mut state = make_pinned_state();
        state.update(Action::ToggleSubmitWithWorkspace);
        assert!(state.submit_with_workspace);
        let res = state.update(Action::ToggleSubmitWithWorkspace);
        assert_eq!(res, UpdateResult::Changed);
        assert!(!state.submit_with_workspace);
    }

    #[test]
    fn test_reset_submit_with_workspace() {
        // ResetSubmitWithWorkspace must clear the flag regardless of workspace contents.
        let mut state = make_pinned_state();
        state.submit_with_workspace = true;
        let res = state.update(Action::ResetSubmitWithWorkspace);
        assert_eq!(res, UpdateResult::Changed);
        assert!(!state.submit_with_workspace);
    }

    #[test]
    fn test_pin_deduplication_via_toggle() {
        // PinCurrentNode acts as a toggle: pinning the same node twice removes it.
        // Programmatic callers must guard externally if they want idempotent pinning.
        let mut state = UiState::new();
        let node_id = brain_domain::NodeId(uuid::Uuid::new_v4());
        state.mode = TuiMode::Exploration;
        let model = brain_domain::query::inspector::InspectorModel {
            entity: brain_domain::dtos::NodeDTO::new(
                node_id.to_string(),
                "Beta Node".to_string(),
                "concept".to_string(),
                serde_json::Value::Null,
            ),
            metadata: std::collections::HashMap::new(),
            relationships: vec![],
            provenance: brain_domain::query::inspector::ProvenanceDTO {
                source: "s".to_string(),
                location: "l".to_string(),
                timestamp: 0,
                extra_info: std::collections::HashMap::new(),
            },
            retrieval_explanation: None,
            recent_activity: vec![],
        };
        state.active_inspector = Some(InspectorSession {
            node_id,
            load_state: InspectorLoadState::Loaded(Box::new(model)),
            breadcrumbs: vec![],
            scroll_offset: 0,
            selected_relation_idx: 0,
        });

        // First pin
        state.update(Action::PinCurrentNode);
        assert_eq!(state.pinned_nodes.len(), 1);

        // Second pin on same node = unpin (toggle behaviour)
        state.update(Action::PinCurrentNode);
        assert_eq!(state.pinned_nodes.len(), 0);
    }

    #[test]
    fn test_set_transient_message() {
        // SetTransientMessage must update the transient_message field.
        let mut state = UiState::new();
        assert!(state.transient_message.is_none());
        let res = state.update(Action::SetTransientMessage(
            "📌 Context used: Alpha".to_string(),
        ));
        assert_eq!(res, UpdateResult::Changed);
        let (msg, _) = state.transient_message.unwrap();
        assert_eq!(msg, "📌 Context used: Alpha");
    }

    #[test]
    fn test_report_error_sets_generation_state() {
        let mut state = UiState::new();
        state.update(Action::StartStream);
        state.update(Action::ReportError("daemon rejected".to_string()));
        assert!(
            matches!(&state.generation_state, GenerationState::Error(msg) if msg == "daemon rejected"),
            "Expected Error state, got {:?}",
            state.generation_state
        );
    }

    #[test]
    fn test_rich_slash_command_suggestions() {
        let mut state = UiState::new();
        state.editor.set_text("/p");
        let suggestions = state.get_slash_suggestions();
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].name, "/plan");
        assert_eq!(suggestions[1].name, "/pin");
    }

    #[test]
    fn test_collapsible_result_groups() {
        let mut state = UiState::new();
        assert!(!state.is_group_collapsed(0));

        state.update(Action::ToggleGroupExpand(0));
        assert!(state.is_group_collapsed(0));

        state.update(Action::ToggleGroupExpand(0));
        assert!(!state.is_group_collapsed(0));
    }
}
