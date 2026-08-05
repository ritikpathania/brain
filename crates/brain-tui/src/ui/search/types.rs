//! Core data structures, identifiers, and types for the unified search pipeline.

use crate::ui::command::CommandId;
use crate::ui::interaction::MessageId;
use brain_domain::SessionId;
use std::collections::HashMap;

/// Stable provider identifier value object. Private construction to prevent
/// arbitrary provider IDs from being created outside this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderId(&'static str);

impl ProviderId {
    /// Crate-private construction helper.
    pub(crate) const fn new(id: &'static str) -> Self {
        Self(id)
    }
    /// Returns the inner string slice.
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

/// Commands search provider identifier.
pub const PROVIDER_COMMANDS: ProviderId = ProviderId::new("commands");
/// Sessions search provider identifier.
pub const PROVIDER_SESSIONS: ProviderId = ProviderId::new("sessions");
/// Local active-session messages search provider identifier.
pub const PROVIDER_LOCAL_MESSAGES: ProviderId = ProviderId::new("messages.local");
/// Remote daemon messages history search provider identifier.
pub const PROVIDER_REMOTE_MESSAGES: ProviderId = ProviderId::new("messages.remote");

/// Monotonic search query generation sequence identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SearchGeneration(pub u64);

/// Bundled query context passed to providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Monotonic query generation identifier.
    pub generation: SearchGeneration,
    /// Raw search query text.
    pub text: String,
}

/// Enumerated structural failure categories for providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchFailure {
    /// Action cancelled by a newer query generation.
    Cancelled,
    /// Connection to Daemon database unavailable.
    BackendUnavailable,
    /// Provider execution timed out.
    Timeout,
    /// Unspecified internal provider error.
    Internal,
}

/// UI search state machine lifecycle categories specified in ADR-027.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum UiSearchState {
    /// Idle state with no active search query.
    #[default]
    Idle,
    /// Query character entered; awaiting 150ms debounce trigger.
    Debouncing,
    /// Asynchronous search active across providers.
    Searching,
    /// Ranked search results available.
    Results,
    /// Search completed with zero matching candidates.
    Empty,
    /// Search failed with a provider or system error.
    Error,
}

/// Error returned when attempting an invalid UI search state transition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Invalid UI search state transition from {from:?} to {to:?}")]
pub struct InvalidStateTransitionError {
    /// Current state before attempted transition.
    pub from: UiSearchState,
    /// Attempted target state.
    pub to: UiSearchState,
}

/// Typed categories of search results consumed by presentation layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchResultKind {
    /// Result triggers a workflow command.
    Command,
    /// Result switches the active session.
    Session,
    /// Result jumps to a specific message content coordinate.
    Message,
    /// Result is a knowledge graph entity from the daemon's retrieval pipeline.
    Knowledge,
}

/// Opaque actions triggered by selecting a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultAction {
    /// Invoke a command parameter collection workflow.
    InvokeCommand(CommandId),
    /// Switch context to the targeted session.
    SwitchSession(SessionId),
    /// Jump scroll focus to a specific message coordinate.
    JumpToMessage {
        /// Message target identifier.
        message_id: MessageId,
    },
    /// Open the memory detail view for a knowledge graph entity.
    /// Used by Phase 2.5 expand-to-details flow.
    OpenMemoryDetail {
        /// Stable entity ID for detail lookup.
        entity_id: String,
    },
}

/// Unified search result produced by providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Stable canonical entity ID from the source knowledge graph.
    ///
    /// Used for deduplication across providers. Empty string for results that
    /// have no stable backing ID (commands, sessions) — these are never deduplicated.
    pub entity_id: String,
    /// Result title text, or `None` if the provider has no human-readable label.
    /// Only `MemoryResultViewModel::from_search_result` resolves `None` to a placeholder.
    pub title: Option<String>,
    /// Explanatory subtitle or description, or `None` if unavailable.
    pub subtitle: Option<String>,
    /// Display category.
    pub kind: SearchResultKind,
    /// Scaled ranking score (i32). Note: follow-up rename to `ranking_score` pending.
    pub provider_score: i32,
    /// Typed confidence carried from the retrieval pipeline.
    pub confidence: crate::client::Confidence,
    /// Triggerable action payload.
    pub action: SearchResultAction,
}

/// Typed search events emitted by providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchEvent {
    /// Signals provider started searching for a generation.
    Started {
        /// Targeted query generation.
        generation: SearchGeneration,
        /// Emitting provider ID.
        provider: ProviderId,
    },
    /// Emits a list of found search results.
    Results {
        /// Targeted query generation.
        generation: SearchGeneration,
        /// Emitting provider ID.
        provider: ProviderId,
        /// Found result batch.
        results: Vec<SearchResult>,
    },
    /// Signals provider completed searching.
    Finished {
        /// Targeted query generation.
        generation: SearchGeneration,
        /// Emitting provider ID.
        provider: ProviderId,
    },
    /// Signals provider execution failure.
    Failed {
        /// Targeted query generation.
        generation: SearchGeneration,
        /// Emitting provider ID.
        provider: ProviderId,
        /// Failure classification category.
        reason: SearchFailure,
    },
}

/// Thread-safe event target interface.
pub trait SearchEventSink: Send + Sync {
    /// Submits a search event to the pipeline.
    fn submit(&self, event: SearchEvent);
}

/// Status tracking of search providers during a query generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatus {
    /// Provider is idle and has not run for this generation.
    Idle,
    /// Provider has started searching.
    Searching,
    /// Provider completed successfully.
    Completed,
    /// Provider execution failed.
    Failed(SearchFailure),
}

/// Read-only snapshot representing active search state.
#[derive(Clone)]
pub struct SearchViewState {
    pub(crate) generation: SearchGeneration,
    pub(crate) query: String,
    pub(crate) ranked_results: Vec<SearchResult>,
    pub(crate) provider_statuses: HashMap<ProviderId, ProviderStatus>,
}

impl SearchViewState {
    /// Returns the active generation ID.
    pub fn generation(&self) -> SearchGeneration {
        self.generation
    }

    /// Returns the active query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Access read-only ranked results.
    pub fn results(&self) -> &[SearchResult] {
        &self.ranked_results
    }

    /// Returns an iterator over statuses to avoid leaking backing collection type.
    pub fn statuses(&self) -> impl Iterator<Item = (&ProviderId, &ProviderStatus)> {
        self.provider_statuses.iter()
    }
}

impl Default for SearchViewState {
    fn default() -> Self {
        Self {
            generation: SearchGeneration(0),
            query: String::new(),
            ranked_results: Vec::new(),
            provider_statuses: HashMap::new(),
        }
    }
}

/// Contextual dynamic snapshot of client-side application state passed to search providers.
#[derive(Debug, Clone)]
pub struct SearchContext {
    /// Active sessions.
    pub sessions: Vec<crate::state::SessionViewModel>,
    /// Loaded messages in the current session.
    pub active_messages: Vec<brain_domain::Message>,
}

/// Abstract search source provider interface.
pub trait SearchProvider: Send + Sync {
    /// Unique provider identifier.
    fn provider_id(&self) -> ProviderId;

    /// Runs a search query. Immediate providers emit results synchronously.
    /// Async providers return immediately and stream results onto the sink.
    fn search(
        &self,
        query: &SearchQuery,
        context: &SearchContext,
        cancellation_token: tokio_util::sync::CancellationToken,
        sink: std::sync::Arc<dyn SearchEventSink>,
    );
}
