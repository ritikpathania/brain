//! Graph domain model.
//!
//! This module defines presentation-agnostic domain models for knowledge graph
//! vertices, edges, subgraphs, and selection states.
//!
//! **Domain Invariants**:
//! - The domain is presentation-agnostic. No ratatui or UI dependencies allowed.
//! - Derived properties like `degree` are computed dynamically from `Subgraph`.

/// Directed edge aggregate representation.
pub mod edge;
/// Node aggregate representation.
pub mod node;
/// Semantic relationship classification.
pub mod relation;
/// Graph focus and selection state tracking.
pub mod selection;
/// In-memory bounded Subgraph aggregate.
pub mod subgraph;

pub use crate::identifiers::{EdgeId, NodeId, RelationId};
pub use edge::EdgeAggregate;
pub use node::{NodeAggregate, NodeKind};
pub use relation::RelationKind;
pub use selection::GraphSelection;
pub use subgraph::{Subgraph, MAX_NEIGHBORHOOD_NODES};
