# Design Specification: Unified Global Search Omnibox & Pluggable Providers

This document defines the architectural design for a unified search omnibox overlaid inside the `brain-tui` workspace. The system is designed as an asynchronous, event-driven search pipeline with decoupled provider indexing, incremental result aggregation, and pure result ranking.

---

## 1. Pipeline Architecture

The global search subsystem is structured as a unidirectional pipeline where controllers trigger sessions, providers emit facts, the aggregator accumulates state, the ranking engine orders results, and the UI remains a passive viewer.

```text
Query Input
      │
      ▼
SearchController ──────────► SearchSession (Generation, Cancellation Token)
      │
      ├──────────────────────────┬──────────────────────────┐
      ▼                          ▼                          ▼
CommandsProvider           SessionsProvider           MessagesProvider
 (Immediate)                (Immediate)                 (Hybrid)
      │                          │                          │
      ▼                          ▼                          ▼
  SearchEvent                SearchEvent                SearchEvent
      │                          │                          │
      └──────────────────────────┼──────────────────────────┘
                                 ▼
                         SearchEventSink
                                 │
                                 ▼ (Updates status & results)
                         SearchAggregator
                                 │
                                 ▼ (Flattens results)
                          RankingEngine
                                 │
                                 ▼ (Produces snapshot)
                         SearchViewState
                                 │
                                 ▼
                           AppRenderer
```

---

## 2. Component Design & Structures

### Data Models & Contracts

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use crate::ui::command::CommandId;
use crate::ui::interaction::MessageId;
use brain_domain::SessionId;

/// Stable provider identifier value object. Private construction to prevent
/// arbitrary provider IDs from being created outside this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderId(&'static str);

impl ProviderId {
    /// Internal construction helper.
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
    /// Returns the inner string slice.
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

pub const PROVIDER_COMMANDS: ProviderId = ProviderId::new("commands");
pub const PROVIDER_SESSIONS: ProviderId = ProviderId::new("sessions");
pub const PROVIDER_LOCAL_MESSAGES: ProviderId = ProviderId::new("messages.local");
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
    Cancelled,
    BackendUnavailable,
    Timeout,
    Internal,
}

/// Typed categories of search results consumed by presentation layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchResultKind {
    Command,
    Session,
    Message,
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
        message_id: MessageId,
    },
}

/// Unified search result produced by providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Result title text.
    pub title: String,
    /// Explanatory subtitle or description.
    pub subtitle: String,
    /// Display category.
    pub kind: SearchResultKind,
    /// Specific provider match score.
    pub provider_score: i32,
    /// Triggerable action payload.
    pub action: SearchResultAction,
}
```

### Event Streaming & Sink

```rust
/// Typed search events emitted by providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchEvent {
    /// Signals provider started searching for a generation.
    Started {
        generation: SearchGeneration,
        provider: ProviderId,
    },
    /// Emits a list of found search results.
    Results {
        generation: SearchGeneration,
        provider: ProviderId,
        results: Vec<SearchResult>,
    },
    /// Signals provider completed searching.
    Finished {
        generation: SearchGeneration,
        provider: ProviderId,
    },
    /// Signals provider execution failure.
    Failed {
        generation: SearchGeneration,
        provider: ProviderId,
        reason: SearchFailure,
    },
}

/// Thread-safe event target interface.
pub trait SearchEventSink: Send + Sync {
    /// Submits a search event to the pipeline.
    fn submit(&self, event: SearchEvent);
}
```

### Provider Contract

```rust
pub trait SearchProvider: Send + Sync {
    /// Unique provider identifier.
    fn provider_id(&self) -> ProviderId;

    /// Runs a search query. Immediate providers emit results synchronously.
    /// Async providers return immediately and stream results onto the sink.
    fn search(
        &self,
        query: &SearchQuery,
        cancellation_token: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    );
}
```

---

## 3. Search Aggregator, Controller & Pure Ranking Engine

### The Search Session & Controller
The `SearchController` coordinates lifetimes and increments generation sequences. It is the only component allowed to start search providers, schedule remote searches, or trigger cancellations.

```rust
pub struct SearchSession {
    pub query: SearchQuery,
    pub cancellation: CancellationToken,
}

pub struct SearchController {
    active_session: Option<SearchSession>,
    providers: Vec<Arc<dyn SearchProvider>>,
    sink: Arc<dyn SearchEventSink>,
}
```

### The Search Aggregator
The `SearchAggregator` is reactive. It updates internal caches upon receiving generation-matched events and builds `SearchViewState` snapshots.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatus {
    Idle,
    Searching,
    Completed,
    Failed(SearchFailure),
}

/// Read-only snapshot representing active search state.
#[derive(Clone)]
pub struct SearchViewState {
    generation: SearchGeneration,
    query: String,
    ranked_results: Vec<SearchResult>,
    provider_statuses: HashMap<ProviderId, ProviderStatus>,
}

impl SearchViewState {
    pub fn generation(&self) -> SearchGeneration { self.generation }
    pub fn query(&self) -> &str { &self.query }
    pub fn results(&self) -> &[SearchResult] { &self.ranked_results }
    
    /// Returns an iterator over statuses to avoid leaking backing collection type.
    pub fn statuses(&self) -> impl Iterator<Item = (&ProviderId, &ProviderStatus)> {
        self.provider_statuses.iter()
    }
}

pub struct SearchAggregator {
    active_generation: SearchGeneration,
    active_query_text: String,
    collected_results: HashMap<ProviderId, Vec<SearchResult>>,
    statuses: HashMap<ProviderId, ProviderStatus>,
    ranking_engine: RankingEngine,
}

impl SearchAggregator {
    /// Processes an incoming search event and updates internal caches.
    pub fn handle_event(&mut self, event: SearchEvent) {
        // Drop events if generation is older than active_generation
        // Update collected_results and statuses accordingly
    }

    /// Generates a pure ViewState snapshot of the current aggregator state.
    pub fn view_state(&self) -> SearchViewState {
        let flattened = self.collected_results.values().flatten().cloned();
        let ranked = self.ranking_engine.rank(&self.active_query_text, flattened);
        
        SearchViewState {
            generation: self.active_generation,
            query: self.active_query_text.clone(),
            ranked_results: ranked,
            provider_statuses: self.statuses.clone(),
        }
    }

    /// Evaluates if all active providers have completed or failed search.
    /// Invariant: A provider that never emitted `Started` is treated as `Idle`, not `Completed`.
    pub fn is_complete(&self) -> bool {
        self.statuses.values().all(|&status| {
            matches!(status, ProviderStatus::Completed | ProviderStatus::Failed(_))
        })
    }
}
```

### Pure Ranking Engine & Sorting Invariants
The `RankingEngine` is pure, containing no internal caching or network calls. It computes a sorted result list using an ordered, additive scoring pipeline:

1. **ProviderScore**: Base matching score computed by the provider.
2. **PrefixBoost**: Adds $+100$ if the title starts with the query.
3. **WordBoundaryBoost**: Adds $+50$ if a word boundary in the title matches the query.
4. **KindBoost**: Adds $+10$ for `Session`, $+5$ for `Command`.

```rust
pub struct RankingEngine;

impl RankingEngine {
    /// Merges, scores, and stably sorts search results.
    pub fn rank(&self, query: &str, results: impl IntoIterator<Item = SearchResult>) -> Vec<SearchResult> {
        let trimmed_query = query.trim().to_lowercase();
        let mut scored_results = Vec::new();

        for res in results {
            let mut score = res.provider_score;

            if !trimmed_query.is_empty() {
                let title_lower = res.title.to_lowercase();

                // 1. Prefix Boost
                if title_lower.starts_with(&trimmed_query) {
                    score += 100;
                }

                // 2. Word Boundary Boost
                if title_lower.contains(&format!(" {}", trimmed_query)) {
                    score += 50;
                }
            }

            // 3. Kind Boost
            match res.kind {
                SearchResultKind::Session => score += 10,
                SearchResultKind::Command => score += 5,
                SearchResultKind::Message => {},
            }

            scored_results.push((score, res));
        }

        // Stably sort descending by score. If scores are equal, fall back to title.
        scored_results.sort_by(|a, b| {
            b.0.cmp(&a.0) // Primary: Score descending
                .then_with(|| a.1.title.cmp(&b.1.title)) // Secondary: Title alphabetical
        });

        scored_results.into_iter().map(|(_, res)| res).collect()
    }
}
```

---

## 4. Verification & Testing Strategy

1. **Aggregator Monotonic Generation Tests**: Assert that `SearchAggregator` drops events carrying generation sequences older than the current generation, maintaining state integrity under network races.
2. **Deterministic Stable Sorting Tests**: Verify `RankingEngine` produces identical, stably-sorted output sequences across duplicate scoring inputs, sorting alphabetically on title.
3. **Provider Lifecycle & Cancellation Tests**: Verify `SearchController` successfully cancels in-flight `SearchProvider` tasks when a new query generation starts.
4. **Hybrid Message Provider Tests**: Test that the `MessagesProvider` immediately emits local messages, while the controller schedules debounced daemon query commands, discarding results if aborted.
