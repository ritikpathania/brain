//! Downstream format and storage projections for compiled KPP representations.

use crate::bkf::errors::BkfError;
use crate::bkf::ir::{CompiledKnowledge, IREdge, IRNode};

/// A transactional diff operation representing an incremental mutation.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectionDelta<O> {
    /// Ingest/Insert new database entry.
    Insert(O),
    /// Mutate an existing database entry.
    Update {
        /// Target unique ID.
        id: String,
        /// Structure containing changed fields.
        changes: O,
    },
    /// Remove/Delete an entry from database.
    Delete(String),
}

/// Boundary trait for computing idempotent delta changesets for storage targets.
pub trait IncrementalProjection {
    /// The specific database operation type target.
    type TargetOp;
    /// The projection conversion error type.
    type Error;

    /// Evaluates the diff between the old compiled graph and new compiled graph, returning deltas.
    fn calculate_delta(
        &self,
        old_graph: Option<&CompiledKnowledge>,
        new_graph: &CompiledKnowledge,
    ) -> Result<Vec<ProjectionDelta<Self::TargetOp>>, Self::Error>;
}

/// Schema layout representing node details in the SQLite projection target.
#[derive(Debug, Clone, PartialEq)]
pub struct SqliteNodeOp {
    /// Unique identifier for the node.
    pub id: String,
    /// Human-readable label of the node.
    pub label: String,
    /// Entity type classification of the node.
    pub entity_type: String,
    /// Extensible semantic attributes serialized as JSON.
    pub attributes: String,
    /// Processing lifecycle state of the node.
    pub lifecycle: String,
    /// Validity status of the node.
    pub validity: String,
    /// Evolution/version status of the node.
    pub version_state: String,
}

/// Schema layout representing edge details in the SQLite projection target.
#[derive(Debug, Clone, PartialEq)]
pub struct SqliteEdgeOp {
    /// Unique identifier for the edge.
    pub id: String,
    /// Source node identifier.
    pub source: String,
    /// Target node identifier.
    pub target: String,
    /// Directional relation tag.
    pub relation: String,
    /// Strength/weight of the relation.
    pub weight: f32,
    /// Processing lifecycle state of the edge.
    pub lifecycle: String,
    /// Validity status of the edge.
    pub validity: String,
    /// Evolution/version status of the edge.
    pub version_state: String,
}

/// Combined database operational delta variants for SQLite.
#[derive(Debug, Clone, PartialEq)]
pub enum SqliteOp {
    /// Database delta targeting a node table entry.
    Node(ProjectionDelta<SqliteNodeOp>),
    /// Database delta targeting an edge table entry.
    Edge(ProjectionDelta<SqliteEdgeOp>),
}

/// SQLite Projection implementation.
pub struct SqliteProjection;

impl SqliteProjection {
    /// Evaluates the diff between two graphs and yields SQLite deltas.
    pub fn calculate_delta(
        &self,
        old_graph: Option<&CompiledKnowledge>,
        new_graph: &CompiledKnowledge,
    ) -> Result<Vec<SqliteOp>, BkfError> {
        let mut ops = Vec::new();

        let old_nodes: std::collections::HashMap<&String, &IRNode> = old_graph
            .map(|g| g.nodes.iter().map(|n| (&n.id, n)).collect())
            .unwrap_or_default();

        let old_edges: std::collections::HashMap<&String, &IREdge> = old_graph
            .map(|g| g.edges.iter().map(|e| (&e.id, e)).collect())
            .unwrap_or_default();

        // 1. Evaluate node operations
        for new_node in &new_graph.nodes {
            let attrs =
                serde_json::to_string(&new_node.attributes).unwrap_or_else(|_| "{}".to_string());
            let node_op = SqliteNodeOp {
                id: new_node.id.clone(),
                label: new_node.label.clone(),
                entity_type: new_node.entity_type.clone(),
                attributes: attrs,
                lifecycle: format!("{:?}", new_node.lifecycle),
                validity: format!("{:?}", new_node.validity),
                version_state: format!("{:?}", new_node.version_state),
            };

            if let Some(old_node) = old_nodes.get(&new_node.id) {
                if *old_node != new_node {
                    ops.push(SqliteOp::Node(ProjectionDelta::Update {
                        id: new_node.id.clone(),
                        changes: node_op,
                    }));
                }
            } else {
                ops.push(SqliteOp::Node(ProjectionDelta::Insert(node_op)));
            }
        }

        for old_id in old_nodes.keys() {
            if !new_graph.nodes.iter().any(|n| &n.id == *old_id) {
                ops.push(SqliteOp::Node(ProjectionDelta::Delete((*old_id).clone())));
            }
        }

        // 2. Evaluate edge operations
        for new_edge in &new_graph.edges {
            let edge_op = SqliteEdgeOp {
                id: new_edge.id.clone(),
                source: new_edge.source.clone(),
                target: new_edge.target.clone(),
                relation: new_edge.relation.clone(),
                weight: new_edge.weight,
                lifecycle: format!("{:?}", new_edge.lifecycle),
                validity: format!("{:?}", new_edge.validity),
                version_state: format!("{:?}", new_edge.version_state),
            };

            if let Some(old_edge) = old_edges.get(&new_edge.id) {
                if *old_edge != new_edge {
                    ops.push(SqliteOp::Edge(ProjectionDelta::Update {
                        id: new_edge.id.clone(),
                        changes: edge_op,
                    }));
                }
            } else {
                ops.push(SqliteOp::Edge(ProjectionDelta::Insert(edge_op)));
            }
        }

        for old_id in old_edges.keys() {
            if !new_graph.edges.iter().any(|e| &e.id == *old_id) {
                ops.push(SqliteOp::Edge(ProjectionDelta::Delete((*old_id).clone())));
            }
        }

        Ok(ops)
    }
}

/// JSON Projection serializer outputting raw serialized string content.
pub struct JsonProjection;

impl JsonProjection {
    /// Projects a graph into a serialized JSON string.
    pub fn project(&self, graph: &CompiledKnowledge) -> Result<String, serde_json::Error> {
        serde_json::to_string(graph)
    }
}
