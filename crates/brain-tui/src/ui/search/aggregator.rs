//! Search event aggregator and reactive view state compiler.

use crate::ui::search::ranking::RankingEngine;
use crate::ui::search::types::{
    ProviderId, ProviderStatus, SearchEvent, SearchGeneration, SearchResult, SearchViewState,
};
use std::collections::HashMap;

/// The SearchAggregator consumes provider search events, filters them by generation,
/// accumulates active results, and compiles read-only ViewState snapshots.
pub struct SearchAggregator {
    active_generation: SearchGeneration,
    active_query_text: String,
    collected_results: HashMap<ProviderId, Vec<SearchResult>>,
    statuses: HashMap<ProviderId, ProviderStatus>,
    expected_providers: Vec<ProviderId>,
    ranking_engine: RankingEngine,
}

impl SearchAggregator {
    /// Instantiates a new SearchAggregator with a set of expected providers.
    pub fn new(expected_providers: Vec<ProviderId>) -> Self {
        let mut statuses = HashMap::new();
        for &provider in &expected_providers {
            statuses.insert(provider, ProviderStatus::Idle);
        }
        Self {
            active_generation: SearchGeneration(0),
            active_query_text: String::new(),
            collected_results: HashMap::new(),
            statuses,
            expected_providers,
            ranking_engine: RankingEngine::default(),
        }
    }

    /// Set the active query text.
    pub fn set_query(&mut self, text: String) {
        self.active_query_text = text;
    }

    /// Processes an incoming search event and updates internal caches.
    /// Drop events if their generation is older than active_generation.
    /// If an event has a newer generation, increment active_generation, clear old results,
    /// reset statuses for all expected providers, and then process the event.
    pub fn handle_event(&mut self, event: SearchEvent) {
        let event_gen = match &event {
            SearchEvent::Started { generation, .. } => *generation,
            SearchEvent::Results { generation, .. } => *generation,
            SearchEvent::Finished { generation, .. } => *generation,
            SearchEvent::Failed { generation, .. } => *generation,
        };

        if event_gen < self.active_generation {
            // Stale generation, drop
            return;
        }

        if event_gen > self.active_generation {
            // Reset for the new generation
            self.active_generation = event_gen;
            self.collected_results.clear();
            for provider in &self.expected_providers {
                self.statuses.insert(*provider, ProviderStatus::Idle);
            }
        }

        match event {
            SearchEvent::Started { provider, .. } => {
                self.statuses.insert(provider, ProviderStatus::Searching);
            }
            SearchEvent::Results {
                provider, results, ..
            } => {
                // Duplicate Results events replace the previous ones for this provider
                self.collected_results.insert(provider, results);
            }
            SearchEvent::Finished { provider, .. } => {
                self.statuses.insert(provider, ProviderStatus::Completed);
            }
            SearchEvent::Failed {
                provider, reason, ..
            } => {
                self.statuses
                    .insert(provider, ProviderStatus::Failed(reason));
            }
        }
    }

    /// Generates a pure ViewState snapshot of the current aggregator state.
    ///
    /// Results with a stable `entity_id` are deduplicated across providers using
    /// `HashSet<&str>` (borrow-based — no clone until the final `.cloned()`).
    /// Results without a stable ID (commands, sessions) are always kept.
    pub fn view_state(&self) -> SearchViewState {
        let mut seen_entity_ids = std::collections::HashSet::new();
        let flattened: Vec<SearchResult> = self
            .collected_results
            .values()
            .flatten()
            .filter(|r| {
                if r.entity_id.is_empty() {
                    true // No stable ID — always keep (commands, sessions)
                } else {
                    // Borrow &str rather than cloning — clone happens once at .cloned() below
                    seen_entity_ids.insert(r.entity_id.as_str())
                }
            })
            .cloned()
            .collect();

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
            matches!(
                status,
                ProviderStatus::Completed | ProviderStatus::Failed(_)
            )
        })
    }

    /// Resets the aggregator back to initial empty state.
    pub fn reset(&mut self) {
        self.active_generation = SearchGeneration(0);
        self.active_query_text = String::new();
        self.collected_results.clear();
        for provider in &self.expected_providers {
            self.statuses.insert(*provider, ProviderStatus::Idle);
        }
    }
}
