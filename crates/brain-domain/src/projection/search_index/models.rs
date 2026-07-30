//! Data models for Search Index Projection.

use serde::{Deserialize, Serialize};

/// Strongly-typed normalized lexical search token.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SearchToken(pub String);

impl SearchToken {
    /// Tokenizes input string by lowercasing and splitting on whitespace and ASCII punctuation.
    pub fn tokenize(input: &str) -> Vec<Self> {
        input
            .to_lowercase()
            .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .map(|s| SearchToken(s.to_string()))
            .collect()
    }
}
