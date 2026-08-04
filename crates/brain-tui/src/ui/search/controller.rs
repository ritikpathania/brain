//! Search Controller orchestrating lifetimes, generation sequences, UI state transitions, and cancellation triggers.

use crate::ui::search::types::{
    InvalidStateTransitionError, SearchContext, SearchEventSink, SearchGeneration, SearchProvider,
    SearchQuery, UiSearchState,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Represents an active query search context.
pub struct SearchSession {
    /// Active search query generation and details.
    pub query: SearchQuery,
    /// Cancellation token for active providers.
    pub cancellation: CancellationToken,
}

/// The controller increments query generations, schedules immediate/async providers,
/// maintains the UI state machine (ADR-027), and cancels in-flight tasks when queries change.
pub struct SearchController {
    active_session: Option<SearchSession>,
    generation_counter: u64,
    state: UiSearchState,
    immediate_providers: Vec<Arc<dyn SearchProvider>>,
    async_providers: Vec<Arc<dyn SearchProvider>>,
    sink: Arc<dyn SearchEventSink>,
}

impl SearchController {
    /// Instantiates a new SearchController.
    /// Invariant: Registration of providers occurs once here and remains immutable.
    pub fn new(
        immediate_providers: Vec<Arc<dyn SearchProvider>>,
        async_providers: Vec<Arc<dyn SearchProvider>>,
        sink: Arc<dyn SearchEventSink>,
    ) -> Self {
        Self {
            active_session: None,
            generation_counter: 0,
            state: UiSearchState::Idle,
            immediate_providers,
            async_providers,
            sink,
        }
    }

    /// Returns the current UI search state.
    pub fn state(&self) -> UiSearchState {
        self.state
    }

    /// Evaluates whether a state transition from `current` to `next` is valid according to ADR-027.
    pub fn is_valid_transition(current: UiSearchState, next: UiSearchState) -> bool {
        match (current, next) {
            // Identity transitions are always valid
            (s1, s2) if s1 == s2 => true,
            // Idle transitions
            (UiSearchState::Idle, UiSearchState::Debouncing) => true,
            (UiSearchState::Idle, UiSearchState::Searching) => true,
            // Debouncing transitions
            (UiSearchState::Debouncing, UiSearchState::Searching) => true,
            (UiSearchState::Debouncing, UiSearchState::Idle) => true,
            (UiSearchState::Debouncing, UiSearchState::Debouncing) => true,
            // Searching transitions
            (UiSearchState::Searching, UiSearchState::Results) => true,
            (UiSearchState::Searching, UiSearchState::Empty) => true,
            (UiSearchState::Searching, UiSearchState::Error) => true,
            (UiSearchState::Searching, UiSearchState::Debouncing) => true,
            (UiSearchState::Searching, UiSearchState::Idle) => true,
            // Outcome state transitions back to new search or reset
            (
                UiSearchState::Results | UiSearchState::Empty | UiSearchState::Error,
                UiSearchState::Debouncing,
            ) => true,
            (
                UiSearchState::Results | UiSearchState::Empty | UiSearchState::Error,
                UiSearchState::Idle,
            ) => true,
            _ => false,
        }
    }

    /// Transitions to the target `next` state if valid, or returns `InvalidStateTransitionError`.
    pub fn transition_to(
        &mut self,
        next: UiSearchState,
    ) -> Result<(), InvalidStateTransitionError> {
        if Self::is_valid_transition(self.state, next) {
            self.state = next;
            Ok(())
        } else {
            Err(InvalidStateTransitionError {
                from: self.state,
                to: next,
            })
        }
    }

    /// Triggers a new search query. Cancels the active generation and starts providers.
    pub fn search(&mut self, text: String, context: &SearchContext) {
        if text.trim().is_empty() {
            self.cancel();
            let _ = self.transition_to(UiSearchState::Idle);
            return;
        }

        let _ = self.transition_to(UiSearchState::Debouncing);

        self.generation_counter += 1;
        let gen = SearchGeneration(self.generation_counter);

        // Cancel previous active session if any
        if let Some(session) = self.active_session.take() {
            session.cancellation.cancel();
        }

        let cancellation = CancellationToken::new();
        let query = SearchQuery {
            generation: gen,
            text,
        };

        // Run immediate/synchronous providers first
        for provider in &self.immediate_providers {
            provider.search(&query, context, cancellation.clone(), self.sink.clone());
        }

        let _ = self.transition_to(UiSearchState::Searching);

        // Spawn async/debounced providers
        for provider in &self.async_providers {
            let token = cancellation.clone();
            let query_clone = query.clone();
            let context_clone = context.clone();
            let provider_clone = provider.clone();
            let sink_clone = self.sink.clone();

            tokio::spawn(async move {
                // Debounce delay of 150ms before triggering async search
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                if token.is_cancelled() {
                    return;
                }
                provider_clone.search(&query_clone, &context_clone, token, sink_clone);
            });
        }

        self.active_session = Some(SearchSession {
            query,
            cancellation,
        });
    }

    /// Explicitly cancels the active search session and resets state to `Idle`.
    pub fn cancel(&mut self) {
        if let Some(session) = self.active_session.take() {
            session.cancellation.cancel();
        }
        let _ = self.transition_to(UiSearchState::Idle);
    }
}
