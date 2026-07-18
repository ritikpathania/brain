//! Pure, side-effect free search result ranking engine.

use crate::ui::search::types::{SearchResult, SearchResultKind};

/// The pure ranking engine calculates result match scores and performs stable sorting.
pub struct RankingEngine;

impl RankingEngine {
    /// Merges, scores, and stably sorts search results.
    ///
    /// The scoring pipeline is ordered and additive:
    /// 1. Base provider score (`SearchResult::provider_score`).
    /// 2. Prefix Boost (+100) if the title starts with the query.
    /// 3. Word Boundary Boost (+50) if a word boundary in the title matches the query.
    /// 4. Kind Boost (+10 for Session, +5 for Command, +0 for Message).
    pub fn rank(
        &self,
        query: &str,
        results: impl IntoIterator<Item = SearchResult>,
    ) -> Vec<SearchResult> {
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
                SearchResultKind::Message => {}
            }

            scored_results.push((score, res));
        }

        // Stably sort descending by score. If scores are equal, fall back to title alphabetically.
        scored_results.sort_by(|a, b| {
            b.0.cmp(&a.0) // Primary: Score descending
                .then_with(|| a.1.title.cmp(&b.1.title)) // Secondary: Title alphabetical
        });

        scored_results.into_iter().map(|(_, res)| res).collect()
    }
}
