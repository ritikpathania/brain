use brain_domain::SessionId;
use serde::{Deserialize, Serialize};
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

    /// Drains paced tokens from the queue based on elapsed time since the last tick.
    pub fn drain_for_tick(&mut self, now: Instant) -> DrainResult {
        let is_first = self.last_drained_at.is_none();
        let last = self.last_drained_at.unwrap_or(now);
        self.last_drained_at = Some(now);

        if self.tokens.is_empty() {
            return DrainResult {
                emitted: Vec::new(),
                finished: self.is_finished(),
            };
        }

        let elapsed = now.duration_since(last);
        let rate_ms = 30; // 30ms typewriter speed
        let count = if elapsed.as_millis() >= rate_ms {
            let num = (elapsed.as_millis() / rate_ms) as usize;
            std::cmp::min(num, self.tokens.len())
        } else if is_first {
            // Default first tick: emit 1 token immediately
            1
        } else {
            0
        };

        let mut emitted = Vec::new();
        for _ in 0..count {
            if let Some(tok) = self.tokens.pop_front() {
                emitted.push(tok);
            }
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
        Self { buffer: String::new() }
    }

    /// Processes raw chunks, buffering split segments and returning completed semantic tokens.
    pub fn push_chunk(&mut self, chunk: &str) -> Vec<RenderToken> {
        self.buffer.push_str(chunk);
        let mut tokens = Vec::new();

        if self.buffer.contains(|c: char| c.is_whitespace()) {
            let mut parts: Vec<String> = self.buffer
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
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Central application layout and editor context.
pub struct UiState {
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
    /// Exit main interactive loop.
    Exit,
}

impl UiState {
    /// Creates a default `UiState` with random Session ID.
    pub fn new() -> Self {
        Self {
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
            pending_load: None,

            session_load_state: SessionLoadState::NotLoaded,
            active_messages: Vec::new(),
            message_revisions: std::collections::HashMap::new(),
            active_response_revision: 0,
        }
    }

    /// Creates a new `UiState` with custom history capacity.
    pub fn with_history_capacity(capacity: usize) -> Self {
        Self {
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
            pending_load: None,

            session_load_state: SessionLoadState::NotLoaded,
            active_messages: Vec::new(),
            message_revisions: std::collections::HashMap::new(),
            active_response_revision: 0,
        }
    }

    /// Idempotent helper clearing the pending load target and updating load state if Loading.
    pub fn clear_pending_load(&mut self) {
        self.pending_load = None;
        if matches!(self.session_load_state, SessionLoadState::Loading) {
            self.session_load_state = SessionLoadState::NotLoaded;
        }
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
    pub fn slash_completion_mut(&mut self) -> &mut crate::ui::command::completion::SlashCompletionState {
        &mut self.slash_completion
    }


    /// Returns the filtered and sorted list of sessions that are visible in the sidebar.
    pub fn visible_sessions(&self) -> Vec<SessionViewModel> {
        let mut filtered: Vec<SessionViewModel> = self.sessions
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
            if self.sidebar.browse.filter == crate::ui::interaction::sidebar::SessionFilter::Active {
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

    /// Pure reducer transitioning state based on Action.
    pub fn update(&mut self, action: Action) -> UpdateResult {
        let prev_messages = self.active_messages.clone();
        let res = self.update_internal(action);
        self.update_message_revisions(&prev_messages);
        res
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
            Action::Resize(cols, rows) => {
                let _ = (cols, rows);
                UpdateResult::Changed
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
                    if self.is_generating() {
                        return UpdateResult::NoChange;
                    }
                    let prompt = self.editor.text();
                    if prompt.trim().is_empty() {
                        return UpdateResult::NoChange;
                    }
                    self.editor.submit();
                    self.generation_state = GenerationState::Starting;
                    self.active_response = String::new();
                    self.active_response_revision += 1;
                    self.typewriter.clear();
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
                self.generation_state = GenerationState::Starting;
                UpdateResult::Changed
            }
            Action::ReceiveToken(token) => {
                self.typewriter.push(token);
                if self.generation_state == GenerationState::Starting {
                    self.generation_state = GenerationState::Streaming {
                        started_at: SystemTime::now(),
                    };
                }
                UpdateResult::Changed
            }
            Action::TypewriterTick(now) => {
                let res = self.typewriter.drain_for_tick(now);
                if !res.emitted.is_empty() {
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
                }
                UpdateResult::Changed
            }
            Action::FinishStream => {
                self.typewriter.finish_backend();
                if self.typewriter.is_finished() {
                    self.generation_state = GenerationState::Finished;
                }
                UpdateResult::Changed
            }
            Action::CancelStream => {
                self.typewriter.clear();
                self.generation_state = GenerationState::Cancelled(None);
                UpdateResult::Changed
            }
            Action::ReportError(msg) => {
                self.typewriter.clear();
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
                self.focus = match self.focus {
                    FocusRegion::Editor => FocusRegion::Sidebar,
                    FocusRegion::Sidebar => FocusRegion::Editor,
                };
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
            Action::ActivateSession { session_id, request_id } => {
                self.pending_load = Some(PendingLoad { session_id, request_id });
                self.session_load_state = SessionLoadState::Loading;
                UpdateResult::Changed
            }
            Action::SessionLoaded { session_id, request_id, messages } => {
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
                        self.session_load_state = SessionLoadState::Loaded(messages);
                        self.clear_pending_load();
                        return UpdateResult::Changed;
                    }
                }
                UpdateResult::NoChange
            }
            Action::SessionLoadFailed { session_id, request_id, error } => {
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
                    if self.selected_session_idx >= self.sessions.len() && !self.sessions.is_empty() {
                        self.selected_session_idx = self.sessions.len() - 1;
                    }
                    UpdateResult::Changed
                }
            }
        }
    }
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

    #[test]
    fn test_reducer_streaming_lifecycle_invariants() {
        let mut state = UiState::new();
        assert_eq!(state.generation_state, GenerationState::Idle);

        // Start stream
        state.update(Action::StartStream);
        assert_eq!(state.generation_state, GenerationState::Starting);

        // Submitting prompt while generating is rejected
        let res = state.update(Action::SubmitPrompt);
        assert_eq!(res, UpdateResult::NoChange);

        // First token received
        state.update(Action::ReceiveToken(RenderToken::Text("A ".to_string())));
        assert!(matches!(state.generation_state, GenerationState::Streaming { .. }));

        // Backend finishes but queue is not empty
        state.update(Action::FinishStream);
        // GenerationState must remain Streaming because typewriter still contains "A "
        assert!(matches!(state.generation_state, GenerationState::Streaming { .. }));
        assert_eq!(state.active_response, "");

        // TypewriterTick drains and finishes
        let now = Instant::now();
        state.update(Action::TypewriterTick(now));
        assert_eq!(state.active_response, "A ");
        assert_eq!(state.generation_state, GenerationState::Finished);
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
        state.update(Action::ActivateSession { session_id: session_a, request_id: req_id });
        assert!(state.pending_load.is_some());
        state.update(Action::SessionLoaded { session_id: session_a, request_id: req_id, messages: vec![] });
        assert_eq!(state.pending_load, None);

        // Path 2: Failed Load
        state.update(Action::ActivateSession { session_id: session_a, request_id: req_id });
        assert!(state.pending_load.is_some());
        state.update(Action::SessionLoadFailed { session_id: session_a, request_id: req_id, error: "fail".to_string() });
        assert_eq!(state.pending_load, None);

        // Path 3: Deleted Pending Session
        state.update(Action::ActivateSession { session_id: session_a, request_id: req_id });
        assert!(state.pending_load.is_some());
        state.update(Action::DeleteSession(session_a));
        assert_eq!(state.pending_load, None);

        // Path 4: Shutdown / Quit
        state.update(Action::ActivateSession { session_id: session_a, request_id: req_id });
        assert!(state.pending_load.is_some());
        state.update(Action::Quit);
        assert_eq!(state.pending_load, None);
    }
}

