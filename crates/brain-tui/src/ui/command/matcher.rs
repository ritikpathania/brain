//! Candidate discovery and RankingFactors computation for command queries.

use crate::ui::command::index::CommandIndex;
use crate::ui::command::registry::CommandMetadata;

/// Structured ranking breakdown factors for transparent evaluation and testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingFactors {
    /// Whether query exact-prefixes command slash name or ID (e.g. "/se" matches "/search").
    pub exact_prefix: bool,
    /// Whether query exact-matches command title or slash name.
    pub exact_name: bool,
    /// Whether query matches a declared alias (e.g. "find" for "/search").
    pub alias_match: bool,
    /// Whether query matches a keyword tag.
    pub keyword_match: bool,
    /// Author-controlled static priority weight.
    pub priority: u16,
    /// Session-local recency score.
    pub recency: u16,
    /// Session-local invocation frequency score.
    pub frequency: u16,
}

impl RankingFactors {
    /// Computes weighted numeric score according to the formula:
    /// score = 1000 * exact_prefix + 500 * exact_name + 250 * alias_match + 100 * keyword_match + priority + 25 * recency + 10 * frequency
    pub fn score(&self) -> u64 {
        let mut score = 0u64;
        if self.exact_prefix {
            score += 1000;
        }
        if self.exact_name {
            score += 500;
        }
        if self.alias_match {
            score += 250;
        }
        if self.keyword_match {
            score += 100;
        }
        score += self.priority as u64;
        score += (self.recency as u64) * 25;
        score += (self.frequency as u64) * 10;
        score
    }
}

/// Match candidate carrying the command metadata and its structured ranking factors.
#[derive(Debug, Clone)]
pub struct CandidateMatch<'a> {
    /// Reference to command metadata entry.
    pub metadata: &'a CommandMetadata,
    /// Computed ranking factors for the query.
    pub factors: RankingFactors,
}

/// Discovers candidate commands matching query string.
pub struct FuzzyMatcher;

impl FuzzyMatcher {
    /// Matches query string against CommandIndex, yielding matching candidates with computed RankingFactors.
    pub fn match_query<'a>(index: &'a CommandIndex, query: &str) -> Vec<CandidateMatch<'a>> {
        let q = query.trim().to_lowercase();
        let is_empty = q.is_empty();
        let q_clean = q.strip_prefix('/').unwrap_or(&q);

        let mut results = Vec::new();

        for cmd in index.entries() {
            let cmd_name_clean = if cmd.name.starts_with('/') {
                &cmd.name[1..]
            } else {
                cmd.name
            };

            let exact_prefix = !is_empty
                && (cmd.name.to_lowercase().starts_with(&q)
                    || cmd_name_clean.to_lowercase().starts_with(q_clean)
                    || cmd.id.to_lowercase().starts_with(q_clean)
                    || cmd
                        .title
                        .to_lowercase()
                        .split_whitespace()
                        .any(|word| word.starts_with(q_clean)));

            let exact_name = !is_empty
                && (cmd.title.eq_ignore_ascii_case(&q)
                    || cmd_name_clean.eq_ignore_ascii_case(q_clean)
                    || cmd.id.eq_ignore_ascii_case(q_clean));

            let alias_match = !is_empty
                && cmd.aliases.iter().any(|alias| {
                    alias.eq_ignore_ascii_case(q_clean) || alias.to_lowercase().starts_with(q_clean)
                });

            let keyword_match = !is_empty
                && cmd.keywords.iter().any(|kw| {
                    kw.eq_ignore_ascii_case(q_clean)
                        || kw.to_lowercase().starts_with(q_clean)
                        || cmd.description.to_lowercase().contains(q_clean)
                });

            let is_match = is_empty || exact_prefix || exact_name || alias_match || keyword_match;

            if is_match {
                let factors = RankingFactors {
                    exact_prefix,
                    exact_name,
                    alias_match,
                    keyword_match,
                    priority: cmd.priority,
                    recency: 0,
                    frequency: 0,
                };
                results.push(CandidateMatch {
                    metadata: cmd,
                    factors,
                });
            }
        }

        results
    }
}
