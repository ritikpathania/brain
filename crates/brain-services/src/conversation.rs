use std::sync::Arc;
use std::time::SystemTime;

use brain_core::errors::BrainError;
use brain_core::extensibility::DecisionEngine;
use brain_core::repositories::RepositorySet;
use brain_core::services::{
    ExtractionRequest, ExtractionResult, MemoryExtractor, RetrievalService,
};
use brain_domain::{
    ConversationId, MemoryDTO, Message, Node, Session, SessionId, SessionTimestamp, SessionTitle,
};
use brain_session::SessionCacheManager;
use brain_storage::SqliteStorage;

/// Budget boundaries for context window construction.
#[derive(Debug, Clone, Copy)]
pub struct ContextBudget {
    /// Maximum total tokens allowed in the context window.
    pub max_tokens: usize,
    /// Reserved token count for system prompt templates.
    pub reserved_system_tokens: usize,
    /// Reserved token count for the LLM output completion response.
    pub reserved_completion_tokens: usize,
}

/// Immutable context window holding prompt messages, summaries, and retrieved memories.
#[derive(Debug, Clone)]
pub struct ContextWindow {
    messages: Vec<Message>,
    summary: Option<ConversationSummary>,
    retrieved_memories: Vec<MemoryDTO>,
}

impl ContextWindow {
    /// Creates a new immutable `ContextWindow`.
    pub fn new(
        messages: Vec<Message>,
        summary: Option<ConversationSummary>,
        retrieved_memories: Vec<MemoryDTO>,
    ) -> Self {
        Self {
            messages,
            summary,
            retrieved_memories,
        }
    }

    /// Exposes prompt messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Exposes the summary if present.
    pub fn summary(&self) -> Option<&ConversationSummary> {
        self.summary.as_ref()
    }

    /// Exposes retrieved memories.
    pub fn retrieved_memories(&self) -> &[MemoryDTO] {
        &self.retrieved_memories
    }
}

/// Versioned, timestamped summary of a historical segment of a conversation thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationSummary {
    /// Unique sequential version index of the summary.
    pub version: u64,
    /// Timestamp when the summary was generated.
    pub created_at: SystemTime,
    /// Starting message index in the conversation history covered by this summary.
    pub start_message_idx: usize,
    /// Ending message index in the conversation history covered by this summary.
    pub end_message_idx: usize,
    /// The text summary content.
    pub text: String,
}

/// Ingestion settings controlling the routing of messages.
#[derive(Debug, Clone, Copy)]
pub struct IngestionPolicy {
    /// If true, writes are stored only in STM and not directly to LTM.
    pub stm_only: bool,
}

/// Decoupled interface for token budget calculations.
pub trait TokenCounter: Send + Sync {
    /// Counts tokens for a raw string.
    fn count_tokens(&self, text: &str) -> usize;
    /// Counts tokens for a sequence of chat messages.
    fn count_message_tokens(&self, messages: &[Message]) -> usize;
}

// Removed local MemoryExtractor trait (defined in brain-core)

/// Contextual information provided to the promotion policies during evaluation.
pub struct PromotionContext<'a> {
    /// The unique session identifier.
    pub session_id: &'a SessionId,
    /// Read-only slice view of STM.
    pub stm: &'a dyn StmView,
    /// Dynamic session metadata.
    pub metadata: &'a SessionMetadata,
    /// Monotonic timestamp for time-based evaluation.
    pub now: std::time::Instant,
}

/// Read-only abstraction over volatile short-term memory (STM) cached state.
pub trait StmView {
    /// Returns a vector of the current short-term memory nodes.
    fn get_nodes(&self) -> Vec<brain_session::StmNode>;
    /// Returns the count of nodes currently residing in STM cache.
    fn len(&self) -> usize;
    /// Returns true if STM cache is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl StmView for brain_session::SessionContext {
    fn get_nodes(&self) -> Vec<brain_session::StmNode> {
        self.iter().cloned().collect()
    }
    fn len(&self) -> usize {
        self.len()
    }
}

/// Dynamic and contextual metadata for the session.
#[derive(Debug, Clone, Default)]
pub struct SessionMetadata {
    /// Active goals and objectives configured for the current session.
    pub active_goals: Vec<String>,
}

/// Typed reasons for promoting short-term memory to long-term memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PromotionReason {
    /// Ingestion count exceeds the count limit.
    RecencyThreshold,
    /// Time elapsed since insertion exceeds the duration limit.
    TimeThreshold,
    /// Memory contains elements of high semantic importance.
    HighImportance,
    /// Memory aligns with one or more active session goals.
    GoalMatch,
    /// Memory was explicitly pinned by the user.
    UserPinned,
    /// Composite boolean logic was fully satisfied.
    CompositeSatisfied,
    /// Cumulative weight exceeds the weighted threshold.
    WeightedThresholdExceeded,
}

/// Structured decision containing detailed explanation telemetry for explainability.
#[derive(Debug, Clone, PartialEq)]
pub struct PromotionDecision {
    /// Whether the promotion action should trigger.
    pub promote: bool,
    /// Confidence score of the decision (0.0 to 1.0).
    pub confidence: f32,
    /// Detailed reasons explaining the decision.
    pub reasons: Vec<PromotionReason>,
}

/// Decoupled interface deciding when to promote STM memory to LTM.
pub trait PromotionPolicy: Send + Sync {
    /// Evaluates context capability signals and returns a promotion decision.
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError>;
}

/// Evaluates context capability signals via a root policy.
pub trait PromotionEngine: Send + Sync {
    /// Evaluates the context and returns a promotion decision.
    fn evaluate(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError>;
}

/// Pure evaluator implementing the PromotionEngine.
pub struct PromotionEngineImpl<P> {
    root: P,
}

impl<P: PromotionPolicy> PromotionEngineImpl<P> {
    /// Creates a new `PromotionEngineImpl` generic over the concrete policy.
    pub fn new(root: P) -> Self {
        Self { root }
    }
}

impl<P: PromotionPolicy> PromotionEngine for PromotionEngineImpl<P> {
    fn evaluate(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        self.root.should_promote(ctx)
    }
}

impl<'a, P: PromotionPolicy> DecisionEngine<PromotionContext<'a>, PromotionDecision>
    for PromotionEngineImpl<P>
{
    fn evaluate(&self, context: &PromotionContext<'a>) -> Result<PromotionDecision, BrainError> {
        self.root.should_promote(context)
    }
}

/// Abstraction matching nodes against goal tags or strings.
pub trait GoalMatcher: Send + Sync {
    /// Returns true if the node is relevant to the target goal string.
    fn matches(&self, goal: &str, node: &brain_session::StmNode) -> bool;
}

/// Default exact-string matches against label or properties["goal_tags"].
pub struct ExactStringGoalMatcher;

impl GoalMatcher for ExactStringGoalMatcher {
    fn matches(&self, goal: &str, node: &brain_session::StmNode) -> bool {
        let goal_lower = goal.to_lowercase();
        // Check label
        if node.node.label.to_lowercase().contains(&goal_lower) {
            return true;
        }
        // Check goal_tags in properties
        if let Some(serde_json::Value::Array(arr)) = node.node.properties.get("goal_tags") {
            for val in arr {
                if let serde_json::Value::String(s) = val {
                    if s.to_lowercase().contains(&goal_lower) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Abstraction scoring the semantic importance of nodes.
pub trait ImportanceScorer: Send + Sync {
    /// Returns the semantic importance weight of the target node.
    fn score(&self, node: &Node) -> f32;
}

/// Default scorer reading properties["importance"] float values.
pub struct PropertyImportanceScorer;

impl ImportanceScorer for PropertyImportanceScorer {
    fn score(&self, node: &Node) -> f32 {
        if let Some(val) = node.properties.get("importance") {
            if let Some(f) = val.as_f64() {
                return f as f32;
            }
            if let Some(i) = val.as_i64() {
                return i as f32;
            }
        }
        0.0
    }
}

/// Policy deciding when to summarize conversation history.
pub trait SummaryPolicy: Send + Sync {
    /// Returns true if message count triggers a summary request.
    fn should_summarize(&self, session_id: &SessionId, message_count: usize) -> bool;
}

/// Decoupled interface for saving and restoring conversation snapshots.
pub trait CheckpointStore: Send + Sync {
    /// Saves an immutable snapshot of session history.
    fn save(
        &self,
        session_id: &SessionId,
        checkpoint_id: &ConversationId,
        label: &str,
        history: &Session,
    ) -> Result<(), BrainError>;

    /// Restores a session snapshot.
    fn restore(
        &self,
        session_id: &SessionId,
        checkpoint_id: &ConversationId,
    ) -> Result<Session, BrainError>;
}

/// Central facade orchestrating conversation lifecycle and memory promotion.
pub trait ConversationManager: Send + Sync {
    /// Ingests a new user interaction, routing it to caches and committing messages.
    fn ingest_interaction(
        &self,
        session_id: &SessionId,
        prompt: &str,
        response: &str,
        policy: IngestionPolicy,
    ) -> Result<(), BrainError>;

    /// Builds a deterministic, token-budgeted prompt context window.
    fn build_context_window(
        &self,
        session_id: &SessionId,
        budget: ContextBudget,
    ) -> Result<ContextWindow, BrainError>;

    /// Promotes short-term interactions from volatile STM cache into persistent LTM storage.
    fn promote_memories(&self, session_id: &SessionId) -> Result<(), BrainError>;

    /// Generates and stores a versioned summary of the conversation history.
    fn summarize_conversation(
        &self,
        session_id: &SessionId,
    ) -> Result<ConversationSummary, BrainError>;

    /// Creates an immutable checkpoint snapshot.
    fn create_checkpoint(
        &self,
        session_id: &SessionId,
        label: &str,
    ) -> Result<ConversationId, BrainError>;

    /// Restores the active conversation state to a permanent checkpoint.
    fn restore_checkpoint(
        &self,
        session_id: &SessionId,
        checkpoint_id: &ConversationId,
    ) -> Result<(), BrainError>;

    /// Prunes low-weight decayed edges/nodes from long-term memory.
    fn prune_memories(&self, session_id: &SessionId) -> Result<usize, BrainError>;

    /// Archives the conversation, preventing further modifications and publishing event.
    fn archive_conversation(&self, session_id: &SessionId) -> Result<(), BrainError>;
}

/// Helper component compiling deterministic prompt assemblies.
pub struct ContextBuilder;

impl ContextBuilder {
    /// Combines history, summaries, and memories deterministically within a token budget.
    pub fn build(
        counter: &dyn TokenCounter,
        budget: ContextBudget,
        history: &[Message],
        summary: Option<ConversationSummary>,
        retrieved_memories: Vec<MemoryDTO>,
    ) -> ContextWindow {
        let start = std::time::Instant::now();
        let input_memories_count = retrieved_memories.len();
        let mut budget_remaining = budget.max_tokens;

        // Reserve completion and system tokens
        budget_remaining = budget_remaining
            .saturating_sub(budget.reserved_completion_tokens)
            .saturating_sub(budget.reserved_system_tokens);

        let system_messages: Vec<Message> = history
            .iter()
            .filter(|m| matches!(m.role, brain_domain::MessageRole::System))
            .cloned()
            .collect();

        // Inc 19: Thinking messages carry persisted reasoning envelopes and
        // are transcript-only — never generation input.
        let non_system_messages: Vec<Message> = history
            .iter()
            .filter(|m| {
                !matches!(
                    m.role,
                    brain_domain::MessageRole::System | brain_domain::MessageRole::Thinking
                )
            })
            .cloned()
            .collect();

        // 1. Budget system messages
        let system_tokens = counter.count_message_tokens(&system_messages);
        budget_remaining = budget_remaining.saturating_sub(system_tokens);

        // 2. Budget summary if present
        let mut included_summary = None;
        if let Some(sum) = summary {
            let summary_tokens = counter.count_tokens(&sum.text);
            if budget_remaining >= summary_tokens {
                budget_remaining -= summary_tokens;
                included_summary = Some(sum);
            }
        }

        // 3. Budget retrieved memories
        let mut included_memories = Vec::new();
        for memory in retrieved_memories {
            let memory_text = format!("{:?}", memory);
            let memory_tokens = counter.count_tokens(&memory_text);
            if budget_remaining >= memory_tokens {
                budget_remaining -= memory_tokens;
                included_memories.push(memory);
            }
        }

        // 4. Budget active chat history from newest to oldest
        let mut included_history = Vec::new();
        for msg in non_system_messages.iter().rev() {
            let msg_tokens = counter.count_message_tokens(std::slice::from_ref(msg));
            if budget_remaining >= msg_tokens {
                budget_remaining -= msg_tokens;
                included_history.push(msg.clone());
            } else {
                break;
            }
        }
        included_history.reverse();

        let mut final_messages = system_messages;
        final_messages.extend(included_history);

        let duration = start.elapsed();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "context_assembly",
            duration_ms = duration.as_millis(),
            input_history_count = history.len(),
            input_memories_count = input_memories_count,
            output_memories_count = included_memories.len(),
            target_budget = budget.max_tokens,
            "Retrieval stage completed: context assembly"
        );

        ContextWindow::new(final_messages, included_summary, included_memories)
    }
}

/// Simple threshold-based STM promotion policy implementation.
pub struct CountThresholdPromotionPolicy {
    threshold: usize,
}

impl CountThresholdPromotionPolicy {
    /// Creates a new `CountThresholdPromotionPolicy`.
    pub fn new(threshold: usize) -> Self {
        Self { threshold }
    }
}

impl PromotionPolicy for CountThresholdPromotionPolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        let count = ctx.stm.len();
        let promote = count >= self.threshold;
        Ok(PromotionDecision {
            promote,
            confidence: if promote { 1.0 } else { 0.0 },
            reasons: if promote {
                vec![PromotionReason::RecencyThreshold]
            } else {
                vec![]
            },
        })
    }
}

/// Policy based on recency count or age thresholds.
pub struct RecencyPolicy {
    count_threshold: Option<usize>,
    time_threshold_seconds: Option<u64>,
}

impl RecencyPolicy {
    /// Creates a new `RecencyPolicy`.
    pub fn new(count_threshold: Option<usize>, time_threshold_seconds: Option<u64>) -> Self {
        Self {
            count_threshold,
            time_threshold_seconds,
        }
    }
}

impl PromotionPolicy for RecencyPolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        let mut promote = false;
        let mut reasons = Vec::new();

        if let Some(count_limit) = self.count_threshold {
            if ctx.stm.len() >= count_limit {
                promote = true;
                reasons.push(PromotionReason::RecencyThreshold);
            }
        }

        if let Some(time_limit) = self.time_threshold_seconds {
            let nodes = ctx.stm.get_nodes();
            let oldest_timestamp = nodes.iter().map(|n| n.node.updated_at).min();
            if let Some(oldest) = oldest_timestamp {
                let now_unix = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now_unix >= oldest && (now_unix - oldest) >= time_limit {
                    promote = true;
                    reasons.push(PromotionReason::TimeThreshold);
                }
            }
        }

        Ok(PromotionDecision {
            promote,
            confidence: if promote { 1.0 } else { 0.0 },
            reasons,
        })
    }
}

/// Policy checking importance scoring metrics.
pub struct SemanticImportancePolicy {
    min_importance: f32,
    use_average: bool,
    scorer: std::sync::Arc<dyn ImportanceScorer>,
}

impl SemanticImportancePolicy {
    /// Creates a new `SemanticImportancePolicy`.
    pub fn new(
        min_importance: f32,
        use_average: bool,
        scorer: std::sync::Arc<dyn ImportanceScorer>,
    ) -> Self {
        Self {
            min_importance,
            use_average,
            scorer,
        }
    }
}

impl PromotionPolicy for SemanticImportancePolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        let nodes = ctx.stm.get_nodes();
        if nodes.is_empty() {
            return Ok(PromotionDecision {
                promote: false,
                confidence: 0.0,
                reasons: vec![],
            });
        }

        let scores: Vec<f32> = nodes
            .iter()
            .map(|stm| self.scorer.score(&stm.node))
            .collect();
        let promote;
        let confidence;

        if self.use_average {
            let sum: f32 = scores.iter().sum();
            let avg = sum / scores.len() as f32;
            promote = avg >= self.min_importance;
            confidence = if self.min_importance > 0.0 {
                (avg / self.min_importance).clamp(0.0, 1.0)
            } else if promote {
                1.0
            } else {
                0.0
            };
        } else {
            let max_score = scores.iter().copied().fold(0.0f32, f32::max);
            promote = max_score >= self.min_importance;
            confidence = if self.min_importance > 0.0 {
                (max_score / self.min_importance).clamp(0.0, 1.0)
            } else if promote {
                1.0
            } else {
                0.0
            };
        }

        let reasons = if promote {
            vec![PromotionReason::HighImportance]
        } else {
            vec![]
        };

        Ok(PromotionDecision {
            promote,
            confidence,
            reasons,
        })
    }
}

/// Policy matching nodes against active goals.
pub struct GoalAwarePolicy {
    matcher: std::sync::Arc<dyn GoalMatcher>,
}

impl GoalAwarePolicy {
    /// Creates a new `GoalAwarePolicy`.
    pub fn new(matcher: std::sync::Arc<dyn GoalMatcher>) -> Self {
        Self { matcher }
    }
}

impl PromotionPolicy for GoalAwarePolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        let stm_nodes = ctx.stm.get_nodes();
        let mut promote = false;
        let mut reasons = Vec::new();

        for goal in &ctx.metadata.active_goals {
            for node in &stm_nodes {
                if self.matcher.matches(goal, node) {
                    promote = true;
                    reasons.push(PromotionReason::GoalMatch);
                    break;
                }
            }
            if promote {
                break;
            }
        }

        Ok(PromotionDecision {
            promote,
            confidence: if promote { 1.0 } else { 0.0 },
            reasons,
        })
    }
}

/// Policy matching pinned elements or high custom weights.
#[derive(Debug, Clone, Copy, Default)]
pub struct UserPinnedPolicy;

impl UserPinnedPolicy {
    /// Creates a new `UserPinnedPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl PromotionPolicy for UserPinnedPolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        let stm_nodes = ctx.stm.get_nodes();
        let mut promote = false;

        for stm in &stm_nodes {
            if let Some(val) = stm.node.properties.get("pinned") {
                if let Some(b) = val.as_bool() {
                    if b {
                        promote = true;
                        break;
                    }
                }
            }
            if let Some(val) = stm.node.properties.get("weight") {
                if let Some(w) = val.as_f64() {
                    if w >= 1.0 {
                        promote = true;
                        break;
                    }
                }
            }
        }

        let reasons = if promote {
            vec![PromotionReason::UserPinned]
        } else {
            vec![]
        };

        Ok(PromotionDecision {
            promote,
            confidence: if promote { 1.0 } else { 0.0 },
            reasons,
        })
    }
}

/// Logical operators for CompositePolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    /// All sub-policies must trigger.
    And,
    /// At least one sub-policy must trigger.
    Or,
    /// Negation of the first sub-policy's trigger.
    Not,
}

/// Composite policy combining sub-policies with boolean operations.
pub struct CompositePolicy {
    policies: Vec<std::sync::Arc<dyn PromotionPolicy>>,
    operator: LogicalOp,
}

impl CompositePolicy {
    /// Creates a new `CompositePolicy`.
    pub fn new(policies: Vec<std::sync::Arc<dyn PromotionPolicy>>, operator: LogicalOp) -> Self {
        Self { policies, operator }
    }
}

impl PromotionPolicy for CompositePolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        if self.operator == LogicalOp::Not {
            if let Some(first) = self.policies.first() {
                let dec = first.should_promote(ctx)?;
                let promote = !dec.promote;
                return Ok(PromotionDecision {
                    promote,
                    confidence: if promote { 1.0 } else { 0.0 },
                    reasons: if promote {
                        vec![PromotionReason::CompositeSatisfied]
                    } else {
                        vec![]
                    },
                });
            }
            return Ok(PromotionDecision {
                promote: false,
                confidence: 0.0,
                reasons: vec![],
            });
        }

        let mut promote = match self.operator {
            LogicalOp::And => true,
            LogicalOp::Or => false,
            _ => unreachable!(),
        };

        let mut reasons = Vec::new();
        let mut total_confidence = 0.0f32;
        let mut count = 0;

        for policy in &self.policies {
            let dec = policy.should_promote(ctx)?;
            count += 1;
            total_confidence += dec.confidence;

            match (self.operator, dec.promote) {
                (LogicalOp::And, false) => {
                    promote = false;
                    break;
                }
                (LogicalOp::And, true) => {
                    reasons.extend(dec.reasons);
                }
                (LogicalOp::Or, true) => {
                    promote = true;
                    reasons.extend(dec.reasons);
                    break;
                }
                _ => {}
            }
        }

        if self.policies.is_empty() {
            promote = false;
        }

        let mut seen = std::collections::HashSet::new();
        reasons.retain(|r| seen.insert(*r));

        let confidence = if count > 0 {
            (total_confidence / count as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let final_reasons = if promote {
            let mut r = vec![PromotionReason::CompositeSatisfied];
            r.extend(reasons);
            r
        } else {
            vec![]
        };

        Ok(PromotionDecision {
            promote,
            confidence,
            reasons: final_reasons,
        })
    }
}

/// Weighted composite policy triggering promotion if sub-policy weights exceed threshold.
pub struct WeightedCompositePolicy {
    policies: Vec<(std::sync::Arc<dyn PromotionPolicy>, f64)>,
    threshold: f64,
}

impl WeightedCompositePolicy {
    /// Creates a new `WeightedCompositePolicy`.
    pub fn new(policies: Vec<(std::sync::Arc<dyn PromotionPolicy>, f64)>, threshold: f64) -> Self {
        Self {
            policies,
            threshold,
        }
    }
}

impl PromotionPolicy for WeightedCompositePolicy {
    fn should_promote(&self, ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        let mut total_weight = 0.0;
        let mut accumulated_weight = 0.0;
        let mut reasons = Vec::new();

        for (policy, weight) in &self.policies {
            total_weight += weight;
            let dec = policy.should_promote(ctx)?;
            if dec.promote {
                accumulated_weight += weight;
                reasons.extend(dec.reasons);
            }
        }

        let promote = accumulated_weight >= self.threshold;

        let mut seen = std::collections::HashSet::new();
        reasons.retain(|r| seen.insert(*r));

        let confidence = if total_weight > 0.0 {
            (accumulated_weight / total_weight) as f32
        } else {
            0.0
        };

        let final_reasons = if promote {
            let mut r = vec![PromotionReason::WeightedThresholdExceeded];
            r.extend(reasons);
            r
        } else {
            vec![]
        };

        Ok(PromotionDecision {
            promote,
            confidence: confidence.clamp(0.0, 1.0),
            reasons: final_reasons,
        })
    }
}

/// Simple threshold-based conversation summary policy implementation.
pub struct CountThresholdSummaryPolicy {
    threshold: usize,
}

impl CountThresholdSummaryPolicy {
    /// Creates a new `CountThresholdSummaryPolicy`.
    pub fn new(threshold: usize) -> Self {
        Self { threshold }
    }
}

impl SummaryPolicy for CountThresholdSummaryPolicy {
    fn should_summarize(&self, _session_id: &SessionId, message_count: usize) -> bool {
        message_count >= self.threshold
    }
}

/// SQLite-backed implementation of the CheckpointStore.
pub struct SqliteCheckpointStore {
    storage: Arc<SqliteStorage>,
}

impl SqliteCheckpointStore {
    /// Creates a new `SqliteCheckpointStore`.
    pub fn new(storage: Arc<SqliteStorage>) -> Self {
        Self { storage }
    }
}

impl CheckpointStore for SqliteCheckpointStore {
    fn save(
        &self,
        session_id: &SessionId,
        checkpoint_id: &ConversationId,
        label: &str,
        history: &Session,
    ) -> Result<(), BrainError> {
        let history_json = serde_json::to_string(history).map_err(|e| BrainError::Validation {
            message: format!("Failed to serialize history: {}", e),
        })?;
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.storage.save_checkpoint_record(
            &checkpoint_id.to_string(),
            &session_id.to_string(),
            label,
            &history_json,
            now,
        )
    }

    fn restore(
        &self,
        _session_id: &SessionId,
        checkpoint_id: &ConversationId,
    ) -> Result<Session, BrainError> {
        let history_json = self
            .storage
            .restore_checkpoint_record(&checkpoint_id.to_string())
            .map_err(|e| BrainError::Session {
                session_id: *_session_id,
                message: format!("Checkpoint not found: {}", e),
            })?;

        let conversation: Session =
            serde_json::from_str(&history_json).map_err(|e| BrainError::Validation {
                message: format!("Failed to deserialize checkpoint: {}", e),
            })?;

        Ok(conversation)
    }
}

/// Concrete implementation of `ConversationManager`.
pub struct ConversationManagerImpl {
    repos: Arc<dyn RepositorySet>,
    storage: Arc<SqliteStorage>,
    cache_manager: Arc<SessionCacheManager>,
    token_counter: Arc<dyn TokenCounter>,
    memory_extractor: Arc<dyn MemoryExtractor>,
    promotion_engine: Arc<dyn PromotionEngine>,
    summary_policy: Arc<dyn SummaryPolicy>,
    checkpoint_store: Arc<dyn CheckpointStore>,
    retrieval_service: Arc<dyn RetrievalService>,
    chat_agent: Arc<dyn brain_core::agents::ChatAgent>,
    event_publisher: Option<Arc<dyn brain_events::EventPublisher>>,
    registry: Arc<brain_domain::RelationRegistry>,
}

impl ConversationManagerImpl {
    /// Creates a new `ConversationManagerImpl`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repos: Arc<dyn RepositorySet>,
        storage: Arc<SqliteStorage>,
        cache_manager: Arc<SessionCacheManager>,
        token_counter: Arc<dyn TokenCounter>,
        memory_extractor: Arc<dyn MemoryExtractor>,
        promotion_engine: Arc<dyn PromotionEngine>,
        summary_policy: Arc<dyn SummaryPolicy>,
        checkpoint_store: Arc<dyn CheckpointStore>,
        retrieval_service: Arc<dyn RetrievalService>,
        chat_agent: Arc<dyn brain_core::agents::ChatAgent>,
        event_publisher: Option<Arc<dyn brain_events::EventPublisher>>,
        registry: Arc<brain_domain::RelationRegistry>,
    ) -> Self {
        Self {
            repos,
            storage,
            cache_manager,
            token_counter,
            memory_extractor,
            promotion_engine,
            summary_policy,
            checkpoint_store,
            retrieval_service,
            chat_agent,
            event_publisher,
            registry,
        }
    }

    fn get_latest_summary(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<ConversationSummary>, BrainError> {
        if let Some((version, created_at_secs, start_idx, end_idx, text)) = self
            .storage
            .get_latest_session_summary(&session_id.to_string())?
        {
            Ok(Some(ConversationSummary {
                version,
                created_at: std::time::SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(created_at_secs),
                start_message_idx: start_idx,
                end_message_idx: end_idx,
                text,
            }))
        } else {
            Ok(None)
        }
    }

    fn save_summary(
        &self,
        session_id: &SessionId,
        summary: &ConversationSummary,
    ) -> Result<(), BrainError> {
        let now = summary
            .created_at
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.storage.save_session_summary(
            &session_id.to_string(),
            summary.version,
            now,
            summary.start_message_idx,
            summary.end_message_idx,
            &summary.text,
        )
    }
}

impl ConversationManager for ConversationManagerImpl {
    fn ingest_interaction(
        &self,
        session_id: &SessionId,
        prompt: &str,
        response: &str,
        policy: IngestionPolicy,
    ) -> Result<(), BrainError> {
        // 1. Transactionally append messages to active session conversation history
        let events = self.storage.run_transaction(|tx| {
            let repos = tx.repositories();
            let mut conversation =
                repos
                    .sessions()
                    .load_session(session_id)?
                    .unwrap_or_else(|| {
                        Session::new(
                            *session_id,
                            SessionTitle("New Session".to_string()),
                            SessionTimestamp(0),
                        )
                    });

            let user_msg = Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                prompt.to_string(),
            );
            let assistant_msg = Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::Assistant,
                response.to_string(),
            );

            conversation.add_message(user_msg)?;
            conversation.add_message(assistant_msg)?;

            repos.sessions().save_session(session_id, &conversation)?;
            let events = conversation.drain_events().collect::<Vec<_>>();
            Ok(events)
        })?;

        if let Some(ref publ) = self.event_publisher {
            for event in events {
                let envelope = brain_events::EventEnvelope::new(
                    "conversation_service".to_string(),
                    brain_events::DomainEvent::Core(event),
                );
                publ.publish(envelope);
            }
        }

        // 2. Perform volatile cache (STM) update if stm_only policy allows
        let nodes_to_ingest = if policy.stm_only {
            // Write only to STM cache
            let req = ExtractionRequest {
                raw_content: prompt.to_string(),
                context_metadata: std::collections::HashMap::new(),
            };
            let extracted = self.memory_extractor.extract(req)?;
            extracted.nodes
        } else {
            // Write directly to SQLite LTM
            let req = ExtractionRequest {
                raw_content: prompt.to_string(),
                context_metadata: std::collections::HashMap::new(),
            };
            let extracted = self.memory_extractor.extract(req)?;
            let (nodes, edges) = (extracted.nodes, extracted.edges);
            self.storage.run_transaction(|tx| {
                let repos = tx.repositories();
                if !nodes.is_empty() {
                    repos.nodes().save_batch(&nodes)?;
                }
                if !edges.is_empty() {
                    repos.edges().save_batch(&edges)?;
                }
                Ok(())
            })?;
            nodes
        };

        // Cache nodes inside session cache manager
        let cache = self.cache_manager.get_or_create(*session_id);
        {
            let mut guard = cache.write().unwrap();
            for node in nodes_to_ingest {
                guard.ingest(node);
            }
        }

        // 3. Evaluate promotion policy trigger
        let conversation = self
            .repos
            .sessions()
            .load_session(session_id)?
            .unwrap_or_else(Session::new_empty);

        let active_goals = conversation
            .goals
            .iter()
            .map(|g| g.text.clone())
            .collect::<Vec<_>>();

        let metadata = SessionMetadata { active_goals };

        let cache_guard = cache.read().unwrap();
        let ctx = PromotionContext {
            session_id,
            stm: &*cache_guard,
            metadata: &metadata,
            now: std::time::Instant::now(),
        };

        let decision = self.promotion_engine.evaluate(&ctx)?;
        if decision.promote {
            drop(cache_guard);
            self.promote_memories(session_id)?;
        } else {
            drop(cache_guard);
        }

        // 4. Evaluate summary policy trigger
        if self
            .summary_policy
            .should_summarize(session_id, conversation.messages.len())
        {
            self.summarize_conversation(session_id)?;
        }

        Ok(())
    }

    fn build_context_window(
        &self,
        session_id: &SessionId,
        budget: ContextBudget,
    ) -> Result<ContextWindow, BrainError> {
        let conversation = self
            .repos
            .sessions()
            .load_session(session_id)?
            .unwrap_or_else(Session::new_empty);

        let latest_summary = self.get_latest_summary(session_id)?;

        // Retrieve memories
        let retrieved = self.retrieval_service.retrieve(session_id, "", 10)?;

        let window = ContextBuilder::build(
            self.token_counter.as_ref(),
            budget,
            &conversation.messages,
            latest_summary,
            retrieved,
        );

        Ok(window)
    }

    fn promote_memories(&self, session_id: &SessionId) -> Result<(), BrainError> {
        // Drains STM context nodes and commits to LTM
        let cache = self.cache_manager.get_or_create(*session_id);
        let nodes = {
            let guard = cache.read().unwrap();
            guard.iter().map(|stm| stm.node.clone()).collect::<Vec<_>>()
        };

        if nodes.is_empty() {
            return Ok(());
        }

        // Perform semantic graph extraction of relation edges from the nodes context
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();

        for node in &nodes {
            let req = ExtractionRequest {
                raw_content: node.label.clone(),
                context_metadata: std::collections::HashMap::new(),
            };
            match self.memory_extractor.extract(req) {
                Ok(extracted) => {
                    all_nodes.extend(extracted.nodes);
                    all_edges.extend(extracted.edges);
                }
                Err(e) => {
                    // Atomically roll back if one fails
                    return Err(e);
                }
            }
        }

        // Validate and construct via GraphBuilder to enforce ontological constraints
        let mut builder = brain_domain::GraphBuilder::new(&self.registry);
        for node in &nodes {
            builder = builder.add_node(node.clone());
        }
        for node in &all_nodes {
            builder = builder.add_node(node.clone());
        }
        for edge in &all_edges {
            builder = builder
                .add_edge(edge.source, edge.target, edge.relation, edge.weight)
                .map_err(|e| BrainError::Validation {
                    message: format!("Graph construction ontology violation: {}", e),
                })?;
        }
        let validated_graph = builder.build();
        let validated_edges: Vec<brain_domain::Edge> =
            validated_graph.edges.into_values().collect();

        // Save transactionally to LTM
        self.storage.run_transaction(|tx| {
            let repos = tx.repositories();
            repos.nodes().save_batch(&nodes)?;
            if !all_nodes.is_empty() {
                repos.nodes().save_batch(&all_nodes)?;
            }
            if !validated_edges.is_empty() {
                repos.edges().save_batch(&validated_edges)?;
            }
            Ok(())
        })?;

        // Evict promoted nodes from STM cache
        {
            let mut guard = cache.write().unwrap();
            // Evict by recreating session context window
            *guard = brain_session::SessionContext::new(*session_id);
        }

        Ok(())
    }

    fn summarize_conversation(
        &self,
        session_id: &SessionId,
    ) -> Result<ConversationSummary, BrainError> {
        let conversation = self
            .repos
            .sessions()
            .load_session(session_id)?
            .unwrap_or_else(Session::new_empty);

        if conversation.messages.is_empty() {
            return Err(BrainError::Validation {
                message: "Cannot summarize empty conversation".to_string(),
            });
        }

        let latest = self.get_latest_summary(session_id)?;
        let next_version = latest.as_ref().map(|s| s.version + 1).unwrap_or(1);
        let start_idx = latest.as_ref().map(|s| s.end_message_idx).unwrap_or(0);
        let end_idx = conversation.messages.len();

        let prompt = format!(
            "Please summarize this conversation thread segment: {:?}",
            &conversation.messages[start_idx..end_idx]
        );

        let text = self.chat_agent.chat(*session_id, &prompt)?;

        let summary = ConversationSummary {
            version: next_version,
            created_at: SystemTime::now(),
            start_message_idx: start_idx,
            end_message_idx: end_idx,
            text,
        };

        self.save_summary(session_id, &summary)?;

        Ok(summary)
    }

    fn create_checkpoint(
        &self,
        session_id: &SessionId,
        label: &str,
    ) -> Result<ConversationId, BrainError> {
        let conversation = self
            .repos
            .sessions()
            .load_session(session_id)?
            .unwrap_or_else(Session::new_empty);

        let checkpoint_id = ConversationId::new();
        self.checkpoint_store
            .save(session_id, &checkpoint_id, label, &conversation)?;

        Ok(checkpoint_id)
    }

    fn restore_checkpoint(
        &self,
        session_id: &SessionId,
        checkpoint_id: &ConversationId,
    ) -> Result<(), BrainError> {
        let conversation = self.checkpoint_store.restore(session_id, checkpoint_id)?;

        // Update active history transactionally
        self.storage.run_transaction(|tx| {
            let repos = tx.repositories();
            repos.sessions().save_session(session_id, &conversation)?;
            Ok(())
        })?;

        // Warm up / sync cache context
        let cache = self.cache_manager.get_or_create(*session_id);
        {
            let mut guard = cache.write().unwrap();
            *guard = brain_session::SessionContext::new(*session_id);
        }

        Ok(())
    }

    fn prune_memories(&self, _session_id: &SessionId) -> Result<usize, BrainError> {
        self.storage.prune_decayed_edges(0.1)
    }

    fn archive_conversation(&self, session_id: &SessionId) -> Result<(), BrainError> {
        self.storage.run_transaction(|tx| {
            let repos = tx.repositories();
            let mut conversation = repos
                .sessions()
                .load_session(session_id)?
                .unwrap_or_else(Session::new_empty);

            let timestamp = SessionTimestamp(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            conversation.archive(timestamp)?;
            repos.sessions().save_session(session_id, &conversation)?;

            // Publish event
            if let Some(publ) = &self.event_publisher {
                let wrapped_event = brain_events::DomainEvent::Session(
                    brain_events::SessionEvent::ConversationArchived(ConversationId(
                        conversation.id.0,
                    )),
                );
                publ.publish(brain_events::EventEnvelope::new(
                    "conversation_service".to_string(),
                    wrapped_event,
                ));
            }

            Ok(())
        })?;
        Ok(())
    }
}

/// Simple space-based token counter implementation.
pub struct WordSpaceTokenCounter;

impl TokenCounter for WordSpaceTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
    fn count_message_tokens(&self, messages: &[Message]) -> usize {
        messages.iter().map(|m| self.count_tokens(&m.content)).sum()
    }
}

/// Dummy memory extractor that returns empty results.
pub struct DummyMemoryExtractor;

impl MemoryExtractor for DummyMemoryExtractor {
    fn extract(&self, _request: ExtractionRequest) -> Result<ExtractionResult, BrainError> {
        Ok(ExtractionResult {
            nodes: vec![],
            edges: vec![],
            provenance: brain_domain::GraphProvenance::default(),
            graph_version: brain_domain::GraphVersion::V1,
        })
    }
}

/// Dummy reasoning agent for default summary generation.
pub struct DummyChatAgent;

impl brain_core::agents::ChatAgent for DummyChatAgent {
    fn name(&self) -> &str {
        "DummyChatAgent"
    }
    fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
        Ok("Conversation summary placeholder".to_string())
    }
}
