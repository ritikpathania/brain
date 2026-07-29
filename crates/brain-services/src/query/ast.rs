//! Declarative intent AST for the Knowledge Query Engine (Phase 5 Milestone 5.1).
//!
//! `KnowledgeQuery` represents pure user intent (search patterns, relation filters, temporal bounds),
//! completely decoupled from internal storage engines, vector indices, or graph view representations.

use crate::compiler::EntityId;
use brain_domain::RelationId;
use serde::{Deserialize, Serialize};

/// Half-open or closed temporal time range bound in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TemporalRange {
    /// Optional inclusive lower bound timestamp in milliseconds.
    pub start_ms: Option<u64>,
    /// Optional inclusive upper bound timestamp in milliseconds.
    pub end_ms: Option<u64>,
}

/// Declarative relationship hop filter specifying a strongly-typed `RelationId` and target `EntityId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationFilter {
    /// Strongly-typed relation identifier (e.g. "depends_on", "associated_with").
    pub relation_kind: RelationId,
    /// Strongly-typed target entity identifier.
    pub target_id: EntityId,
}

/// Declarative query intent AST for searching and traversing canonical knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct KnowledgeQuery {
    /// Text pattern for lexical full-text search.
    pub text: Option<String>,
    /// Semantic prompt text for embedding similarity search.
    pub semantic_prompt: Option<String>,
    /// Optional target entity concept classification kind filter.
    pub target_kind: Option<String>,
    /// Declarative relationship hop constraint filters.
    pub relation_filters: Vec<RelationFilter>,
    /// Temporal timestamp range constraint filter.
    pub temporal_range: Option<TemporalRange>,
    /// Maximum number of candidate results to return (default: 20).
    pub limit: usize,
}

impl KnowledgeQuery {
    /// Instantiates a new builder for `KnowledgeQuery`.
    pub fn new() -> Self {
        Self {
            text: None,
            semantic_prompt: None,
            target_kind: None,
            relation_filters: Vec::new(),
            temporal_range: None,
            limit: 20,
        }
    }

    /// Sets the lexical text search pattern.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Sets the semantic embedding prompt pattern.
    pub fn with_semantic_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.semantic_prompt = Some(prompt.into());
        self
    }

    /// Adds a declarative relation hop filter constraint.
    pub fn with_relation_filter(mut self, relation_kind: RelationId, target_id: EntityId) -> Self {
        self.relation_filters.push(RelationFilter {
            relation_kind,
            target_id,
        });
        self
    }

    /// Sets temporal bounds for the query.
    pub fn with_temporal_range(mut self, range: TemporalRange) -> Self {
        self.temporal_range = Some(range);
        self
    }

    /// Sets maximum result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}
