//! Pure domain reducer for Graph Adjacency Projection.

use crate::bkf::events::FactEvent;
use crate::bkf::fact_version::AssertionTarget;
use crate::identifiers::EntityId;
use crate::projection::errors::*;
use crate::projection::graph_adjacency::models::*;
use crate::projection::graph_adjacency::state::*;
use crate::projection::id::*;
use crate::projection::reducer::*;

/// Domain reducer reducing FactEvents into GraphAdjacencyState.
#[derive(Debug, Clone)]
pub struct GraphAdjacencyReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: GraphAdjacencyState,
}

impl GraphAdjacencyReducer {
    /// Creates a new GraphAdjacencyReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: GraphAdjacencyState::default(),
        }
    }

    /// Returns reference to internal graph adjacency state.
    pub fn state(&self) -> &GraphAdjacencyState {
        &self.state
    }
}

impl ProjectionReducer for GraphAdjacencyReducer {
    fn id(&self) -> ProjectionId {
        self.id.clone()
    }

    fn version(&self) -> ProjectionVersion {
        self.version
    }

    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError> {
        match event {
            FactEvent::FactRecorded { fact, assertion } => {
                let edge_id = GraphEdgeId(fact.id.clone());
                if let Some(assert) = assertion {
                    let source = GraphNodeId(EntityId(assert.subject.0));
                    if let AssertionTarget::Entity(target_id) = &assert.object {
                        let target = GraphNodeId(EntityId(target_id.0));
                        let record = EdgeRecord {
                            id: edge_id,
                            source,
                            target,
                            predicate: assert.predicate.clone(),
                            confidence: fact.confidence,
                            temporal: fact.temporal.clone(),
                        };
                        self.state.insert_edge(record);
                    }
                }
            }
            FactEvent::FactSuperseded { old_fact_id, .. }
            | FactEvent::FactArchived { fact_id: old_fact_id, .. } => {
                let edge_id = GraphEdgeId(old_fact_id.clone());
                self.state.remove_edge(&edge_id);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = GraphAdjacencyState::default();
        Ok(())
    }
}
