//! Pure, side-effect free search result ranking engine.

use crate::ui::search::types::{SearchResult, SearchResultKind};

/// Configuration controlling the ranking engine's filtering behaviour.
///
/// `minimum_score` prevents candidates from appearing when they score below
/// a meaningful threshold during a typed query.
///
/// **Derivation of the default value (6):**
/// The `SearchProjector` produces raw scores as follows:
/// - Token overlap: +50 per matching term
/// - Fuzzy match: 0–N (fuzzy_score from skim_v2)
/// - Aggregator prefix boost: +100 if title starts with query
/// - Aggregator word-boundary boost: +50
/// - Kind boost: Session +10, Command +5, Knowledge/Message +0
///
/// A result with score 1–5 has received only a negligible kind boost with no
/// textual signal. Threshold 6 passes any result that received at least a
/// Kind boost, ensuring Commands and Sessions are always surfaced.
///
/// Has **no effect on empty queries** — all results pass through unfiltered.
#[derive(Debug, Clone)]
pub struct RankingConfig {
    /// Minimum composite score required to include a result when the query is non-empty.
    pub minimum_score: i32,
}

impl Default for RankingConfig {
    fn default() -> Self {
        Self { minimum_score: 6 }
    }
}

/// The pure ranking engine calculates result match scores and performs stable sorting.
#[derive(Default)]
pub struct RankingEngine {
    /// Configuration controlling score threshold filtering.
    pub config: RankingConfig,
}

impl RankingEngine {
    /// Merges, scores, stably sorts, and filters search results.
    ///
    /// The scoring pipeline is ordered and additive:
    /// 1. Base provider score (`SearchResult::provider_score`).
    /// 2. Prefix Boost (+100) if the title starts with the query.
    /// 3. Word Boundary Boost (+50) if a word boundary in the title matches the query.
    /// 4. Kind Boost (+10 for Session, +5 for Command, +0 for Knowledge/Message).
    /// 5. Score threshold filter: results below `config.minimum_score` are excluded
    ///    when the query is non-empty. Empty queries bypass filtering entirely.
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
                // Use title if available, otherwise skip text boosts
                if let Some(title) = &res.title {
                    let title_lower = title.to_lowercase();

                    // 1. Prefix Boost
                    if title_lower.starts_with(&trimmed_query) {
                        score += 100;
                    }

                    // 2. Word Boundary Boost
                    if title_lower.contains(&format!(" {}", trimmed_query)) {
                        score += 50;
                    }
                }
            }

            // 3. Kind Boost
            match res.kind {
                SearchResultKind::Session => score += 10,
                SearchResultKind::Command => score += 5,
                SearchResultKind::Message | SearchResultKind::Knowledge => {}
            }

            scored_results.push((score, res));
        }

        // 4. Threshold filter (only when query is non-empty)
        if !trimmed_query.is_empty() {
            scored_results.retain(|(score, _)| *score >= self.config.minimum_score);
        }

        // Stably sort descending by score. If scores are equal, fall back to title alphabetically.
        scored_results.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| match (&a.1.title, &b.1.title) {
                (Some(ta), Some(tb)) => ta.cmp(tb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
        });

        scored_results.into_iter().map(|(_, res)| res).collect()
    }
}
