//! Unified Query Models for Phase 5 Query Facade.

use brain_domain::bkf::*;
use serde::{Deserialize, Serialize};

/// Mutually exclusive temporal validity filtering mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalMode {
    /// Returns only currently active facts (valid_until is None).
    CurrentActive,
    /// Returns facts that were valid at the specified historical timestamp.
    ValidAt(Timestamp),
    /// Includes all historical and active facts without temporal filtering.
    AllHistorical,
}

/// Confidence score threshold filter using BKF Confidence value object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceFilter {
    /// Minimum confidence threshold enforced by BKF Confidence invariant [0.0..1.0].
    pub min_confidence: Confidence,
}

/// Sort field options for query ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    /// Sort by average entity confidence score.
    Confidence,
    /// Sort by total graph connections (degree).
    Degree,
    /// Sort by recency of facts.
    Recency,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    /// Ascending order.
    Ascending,
    /// Descending order.
    Descending,
}

/// Query ordering configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryOrdering {
    /// Field to sort results by.
    pub field: SortField,
    /// Direction of sorting.
    pub direction: SortDirection,
}

/// Reusable pagination parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaginationParams {
    /// Maximum number of items to return.
    pub limit: usize,
    /// Number of items to skip.
    pub offset: usize,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

/// Parameters for node neighborhood graph traversal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborhoodQuery {
    /// Root entity ID for neighborhood expansion.
    pub root_entity: KnowledgeEntityId,
    /// Maximum graph traversal depth (hops).
    pub max_hops: usize,
    /// Temporal filtering mode.
    pub temporal_mode: TemporalMode,
    /// Optional confidence score filter.
    pub confidence_filter: Option<ConfidenceFilter>,
    /// Pagination specification.
    pub pagination: PaginationParams,
}

/// Parameters for point-in-time entity state lookups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointInTimeQuery {
    /// Entity ID to inspect.
    pub entity: KnowledgeEntityId,
    /// Historical timestamp to evaluate validity at.
    pub timestamp: Timestamp,
}

/// Parameters for lexical inverted search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LexicalSearchQuery {
    /// Raw query text string.
    pub query_string: String,
    /// Temporal filtering mode.
    pub temporal_mode: TemporalMode,
    /// Optional confidence score filter.
    pub confidence_filter: Option<ConfidenceFilter>,
    /// Pagination specification.
    pub pagination: PaginationParams,
}

/// Compound hybrid query combining search, time, metrics, and graph topology.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridSearchQuery {
    /// Raw query text string.
    pub query_string: String,
    /// Optional root entity for neighborhood-constrained search.
    pub root_entity: Option<KnowledgeEntityId>,
    /// Temporal filtering mode.
    pub temporal_mode: TemporalMode,
    /// Optional confidence score filter.
    pub confidence_filter: Option<ConfidenceFilter>,
    /// Optional sorting specification.
    pub ordering: Option<QueryOrdering>,
    /// Pagination specification.
    pub pagination: PaginationParams,
}

/// Graph topology metadata for matched entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphMetadata {
    /// Number of incoming connections.
    pub in_degree: usize,
    /// Number of outgoing connections.
    pub out_degree: usize,
}

/// Search lexical match metadata (uses String terms, keeping SearchToken internal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMetadata {
    /// Matched token string terms.
    pub matched_terms: Vec<String>,
}

/// Core matched entity result item with optional specialized metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMatch {
    /// Knowledge entity ID.
    pub entity_id: KnowledgeEntityId,
    /// Count of active facts associated with entity.
    pub active_facts_count: usize,
    /// Average confidence score across entity facts.
    pub average_confidence: Confidence,
    /// Optional graph topology metadata.
    pub graph_metadata: Option<GraphMetadata>,
    /// Optional lexical search metadata.
    pub search_metadata: Option<SearchMetadata>,
}

/// Execution metrics container separate from semantic result items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResponseMetadata {
    /// Query execution duration in microseconds.
    pub execution_duration_us: u64,
    /// Projection snapshot watermark evaluated against.
    pub snapshot_watermark: u64,
}

/// Unified query result wrapper for Phase 5 Facade queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryFacadeResult {
    /// Matched entity items.
    pub matches: Vec<EntityMatch>,
    /// Total count of matching items before pagination limit.
    pub total_matched: usize,
    /// Execution and snapshot metadata.
    pub metadata: QueryResponseMetadata,
}
