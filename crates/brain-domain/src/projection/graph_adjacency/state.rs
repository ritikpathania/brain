//! In-memory graph adjacency state.

use crate::projection::graph_adjacency::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// In-memory graph adjacency state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphAdjacencyState {
    out_edges: HashMap<GraphNodeId, Vec<GraphEdgeId>>,
    in_edges: HashMap<GraphNodeId, Vec<GraphEdgeId>>,
    edges: HashMap<GraphEdgeId, EdgeRecord>,
    degrees: HashMap<GraphNodeId, NodeDegree>,
}

impl GraphAdjacencyState {
    /// Returns total count of indexed edges.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Returns true if no edges are indexed.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
    /// Returns outgoing edge IDs for node.
    pub fn neighbors_out(&self, node: &GraphNodeId) -> &[GraphEdgeId] {
        self.out_edges.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns incoming edge IDs for node.
    pub fn neighbors_in(&self, node: &GraphNodeId) -> &[GraphEdgeId] {
        self.in_edges.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns node degree stats.
    pub fn degree(&self, node: &GraphNodeId) -> NodeDegree {
        self.degrees.get(node).copied().unwrap_or_default()
    }

    /// Returns edge record by GraphEdgeId.
    pub fn edge(&self, id: &GraphEdgeId) -> Option<&EdgeRecord> {
        self.edges.get(id)
    }

    /// Internal helper inserting edge record atomically.
    pub fn insert_edge(&mut self, record: EdgeRecord) {
        if self.edges.contains_key(&record.id) {
            return; // Idempotent duplicate ignore
        }
        let edge_id = record.id.clone();
        let source = record.source.clone();
        let target = record.target.clone();

        self.edges.insert(edge_id.clone(), record);
        self.out_edges.entry(source.clone()).or_default().push(edge_id.clone());
        self.in_edges.entry(target.clone()).or_default().push(edge_id);

        self.degrees.entry(source).or_default().out_degree += 1;
        self.degrees.entry(target).or_default().in_degree += 1;
    }

    /// Internal helper removing edge record atomically with empty key pruning.
    pub fn remove_edge(&mut self, edge_id: &GraphEdgeId) {
        if let Some(record) = self.edges.remove(edge_id) {
            if let Some(out_list) = self.out_edges.get_mut(&record.source) {
                out_list.retain(|id| id != edge_id);
                if out_list.is_empty() {
                    self.out_edges.remove(&record.source);
                }
            }
            if let Some(in_list) = self.in_edges.get_mut(&record.target) {
                in_list.retain(|id| id != edge_id);
                if in_list.is_empty() {
                    self.in_edges.remove(&record.target);
                }
            }

            if let Some(deg) = self.degrees.get_mut(&record.source) {
                deg.out_degree = deg.out_degree.saturating_sub(1);
                if deg.out_degree == 0 && deg.in_degree == 0 {
                    self.degrees.remove(&record.source);
                }
            }
            if let Some(deg) = self.degrees.get_mut(&record.target) {
                deg.in_degree = deg.in_degree.saturating_sub(1);
                if deg.out_degree == 0 && deg.in_degree == 0 {
                    self.degrees.remove(&record.target);
                }
            }
        }
    }
}
