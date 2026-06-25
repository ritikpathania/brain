use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::{HashMap, HashSet};

use crate::plugins::RetrievalAlgorithm;
use crate::stm::{STMIndex, TempNode};

/// Tokenize and normalize text by removing punctuation, lowercasing, and skipping stop-words.
pub fn tokenize(text: &str) -> HashSet<String> {
    let stop_words: HashSet<&str> = [
        "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in", "on",
        "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it", "its",
        "you", "your", "my", "up", "down", "out", "off",
    ]
    .iter()
    .cloned()
    .collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && s.len() > 1 && !stop_words.contains(s))
        .map(|s| s.to_string())
        .collect()
}

#[derive(Clone)]
pub struct FuzzyRetrieval;

impl RetrievalAlgorithm for FuzzyRetrieval {
    fn name(&self) -> &str {
        "fuzzy"
    }

    fn retrieve(
        &self,
        query: &str,
        index: &STMIndex,
        window: &[TempNode],
    ) -> Result<Vec<(TempNode, i64)>, String> {
        if window.is_empty() {
            return Ok(Vec::new());
        }

        let query_tokens = tokenize(query);
        let matcher = SkimMatcherV2::default();

        // 1. Inverted index lookup for exact token overlaps (highly weighted)
        let mut candidate_scores: HashMap<String, i64> = HashMap::new();
        for token in &query_tokens {
            if let Some(node_ids) = index.inverted_index.get(token) {
                for id in node_ids {
                    *candidate_scores.entry(id.clone()).or_insert(0) += 50;
                }
            }
        }

        // 2. Fuzzy text matching scoring
        let mut scored_nodes = Vec::new();
        for node in window {
            let base_score = *candidate_scores.get(&node.id).unwrap_or(&0);

            // Fuzzy match the raw content against query
            let fuzzy_score = matcher.fuzzy_match(&node.content, query).unwrap_or(0);

            let total_score = base_score + fuzzy_score;

            // Only return nodes with some degree of match
            if total_score > 0 {
                scored_nodes.push((node.clone(), total_score));
            }
        }

        Ok(scored_nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenization() {
        let tokens = tokenize("Setting up a Database Configuration in SQLite!");
        assert!(tokens.contains("setting"));
        assert!(tokens.contains("database"));
        assert!(tokens.contains("configuration"));
        assert!(tokens.contains("sqlite"));
        // Check stop-words are ignored
        assert!(!tokens.contains("up"));
        assert!(!tokens.contains("a"));
        assert!(!tokens.contains("in"));
    }
}
