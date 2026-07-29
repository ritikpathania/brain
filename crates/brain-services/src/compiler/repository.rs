//! Abstract In-Memory Knowledge Repository for compiler testing and isolation.

use crate::compiler::delta::GraphDelta;
use brain_domain::dtos::{EdgeDTO, NodeDTO};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Abstract repository trait for applying graph deltas and querying knowledge.
pub trait KnowledgeRepository: Send + Sync {
    /// Applies a calculated GraphDelta to the repository state.
    fn apply_delta(&self, delta: &GraphDelta) -> Result<(), String>;
    /// Retrieves all current node DTOs.
    fn get_nodes(&self) -> Result<Vec<NodeDTO>, String>;
    /// Retrieves all current edge DTOs.
    fn get_edges(&self) -> Result<Vec<EdgeDTO>, String>;
}

/// Thread-safe In-Memory implementation of `KnowledgeRepository`.
#[derive(Debug, Clone, Default)]
pub struct InMemoryKnowledgeRepository {
    nodes: Arc<RwLock<HashMap<String, NodeDTO>>>,
    edges: Arc<RwLock<Vec<EdgeDTO>>>,
}

impl InMemoryKnowledgeRepository {
    /// Instantiates a new empty `InMemoryKnowledgeRepository`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl KnowledgeRepository for InMemoryKnowledgeRepository {
    fn apply_delta(&self, delta: &GraphDelta) -> Result<(), String> {
        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
        let mut edges = self.edges.write().map_err(|e| e.to_string())?;

        // Process added nodes
        for node in &delta.added_nodes {
            nodes.insert(node.id.clone(), node.clone());
        }

        // Process updated nodes
        for node in &delta.updated_nodes {
            nodes.insert(node.id.clone(), node.clone());
        }

        // Process removed nodes
        for node_id in &delta.removed_nodes {
            nodes.remove(&node_id.0);
        }

        // Process added edges
        for edge in &delta.added_edges {
            if !edges.iter().any(|e| {
                e.source == edge.source && e.target == edge.target && e.relation == edge.relation
            }) {
                edges.push(edge.clone());
            }
        }

        // Process updated edges
        for edge in &delta.updated_edges {
            if let Some(pos) = edges.iter().position(|e| {
                e.source == edge.source && e.target == edge.target && e.relation == edge.relation
            }) {
                edges[pos] = edge.clone();
            } else {
                edges.push(edge.clone());
            }
        }

        // Process removed edges
        for edge_id in &delta.removed_edges {
            edges.retain(|e| format!("{}_{}_{}", e.source, e.relation, e.target) != edge_id.0);
        }

        Ok(())
    }

    fn get_nodes(&self) -> Result<Vec<NodeDTO>, String> {
        let nodes = self.nodes.read().map_err(|e| e.to_string())?;
        Ok(nodes.values().cloned().collect())
    }

    fn get_edges(&self) -> Result<Vec<EdgeDTO>, String> {
        let edges = self.edges.read().map_err(|e| e.to_string())?;
        Ok(edges.clone())
    }
}
