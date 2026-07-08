//! Search Controller orchestrating lifetimes, generation sequences, and cancellation triggers.

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use crate::ui::search::types::{
    SearchQuery, SearchGeneration, SearchEventSink, SearchContext, SearchProvider
};

/// Represents an active query search context.
pub struct SearchSession {
    /// Active search query generation and details.
    pub query: SearchQuery,
    /// Cancellation token for active providers.
    pub cancellation: CancellationToken,
}

/// The controller increments query generations, schedules immediate/async providers,
/// and cancels in-flight tasks when queries change.
pub struct SearchController {
    active_session: Option<SearchSession>,
    generation_counter: u64,
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
            immediate_providers,
            async_providers,
            sink,
        }
    }

    /// Triggers a new search query. Cancels the active generation and starts providers.
    pub fn search(&mut self, text: String, context: &SearchContext) {
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

    /// Explicitly cancels the active search session.
    pub fn cancel(&mut self) {
        if let Some(session) = self.active_session.take() {
            session.cancellation.cancel();
        }
    }
}
