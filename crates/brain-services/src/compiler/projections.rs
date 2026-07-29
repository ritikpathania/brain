//! Read Projection Engine and deterministic ReadProjection trait abstractions for the Knowledge Compiler.

use crate::compiler::delta::GraphDelta;
use brain_domain::dtos::{EdgeDTO, NodeDTO};
use std::collections::HashMap;

/// Generic trait implemented by all downstream read-model projections.
pub trait ReadProjection: Send + Sync {
    /// Applies a calculated GraphDelta to update the projection's internal view state.
    fn apply_delta(&mut self, delta: &GraphDelta) -> Result<(), String>;
}

/// Search index projection materializing searchable NodeDTO records.
#[derive(Debug, Clone, Default)]
pub struct SearchProjector {
    /// Search index entries mapped by node ID.
    pub index: HashMap<String, NodeDTO>,
}

impl SearchProjector {
    /// Instantiates a new empty `SearchProjector`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ReadProjection for SearchProjector {
    fn apply_delta(&mut self, delta: &GraphDelta) -> Result<(), String> {
        for node in &delta.added_nodes {
            self.index.insert(node.id.clone(), node.clone());
        }
        for node in &delta.updated_nodes {
            self.index.insert(node.id.clone(), node.clone());
        }
        for node_id in &delta.removed_nodes {
            self.index.remove(&node_id.0);
        }
        Ok(())
    }
}

/// Graph view projection materializing relationship edge structures.
#[derive(Debug, Clone, Default)]
pub struct GraphProjector {
    /// Graph edges.
    pub edges: Vec<EdgeDTO>,
}

impl GraphProjector {
    /// Instantiates a new empty `GraphProjector`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ReadProjection for GraphProjector {
    fn apply_delta(&mut self, delta: &GraphDelta) -> Result<(), String> {
        for edge in &delta.added_edges {
            if !self.edges.iter().any(|e| {
                e.source == edge.source && e.target == edge.target && e.relation == edge.relation
            }) {
                self.edges.push(edge.clone());
            }
        }
        for edge in &delta.updated_edges {
            if let Some(pos) = self.edges.iter().position(|e| {
                e.source == edge.source && e.target == edge.target && e.relation == edge.relation
            }) {
                self.edges[pos] = edge.clone();
            } else {
                self.edges.push(edge.clone());
            }
        }
        for edge_id in &delta.removed_edges {
            self.edges
                .retain(|e| format!("{}_{}_{}", e.source, e.relation, e.target) != edge_id.0);
        }
        Ok(())
    }
}

/// Engine managing deterministic, ordered projection updates with isolated failure handling.
#[derive(Default)]
pub struct ProjectionEngine {
    projections: Vec<Box<dyn ReadProjection>>,
}

impl ProjectionEngine {
    /// Instantiates an empty `ProjectionEngine`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new `ReadProjection` implementation.
    pub fn register(&mut self, projection: Box<dyn ReadProjection>) {
        self.projections.push(projection);
    }

    /// Applies a GraphDelta across all registered projections in deterministic sequence order.
    /// Isolated failure handling ensures single projection errors do not abort independent update passes.
    pub fn apply_delta_all(&mut self, delta: &GraphDelta) -> Vec<Result<(), String>> {
        self.projections
            .iter_mut()
            .map(|proj| proj.apply_delta(delta))
            .collect()
    }
}
