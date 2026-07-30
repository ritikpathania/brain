//! Pure domain reducer for Entity Statistics Projection.

use crate::bkf::events::FactEvent;
use crate::projection::conformance::*;
use crate::projection::entity_statistics::state::*;
use crate::projection::errors::*;
use crate::projection::id::*;
use crate::projection::reducer::*;

/// Domain reducer reducing FactEvents into EntityStatisticsState.
#[derive(Debug, Clone)]
pub struct EntityStatisticsReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: EntityStatisticsState,
}

impl EntityStatisticsReducer {
    /// Creates a new EntityStatisticsReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: EntityStatisticsState::default(),
        }
    }
}

impl ProjectionStateView for EntityStatisticsReducer {
    type State = EntityStatisticsState;
    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl ProjectionReducer for EntityStatisticsReducer {
    fn id(&self) -> ProjectionId {
        self.id.clone()
    }

    fn version(&self) -> ProjectionVersion {
        self.version
    }

    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError> {
        match event {
            FactEvent::FactRecorded { fact, assertion } => {
                if let Some(assert) = assertion {
                    self.state.record_fact(fact, assert);
                }
            }
            FactEvent::FactSuperseded { old_fact_id, superseded_at, .. } => {
                self.state.supersede_fact(old_fact_id, *superseded_at);
            }
            FactEvent::FactArchived { fact_id, archived_at } => {
                self.state.archive_fact(fact_id, *archived_at);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = EntityStatisticsState::default();
        Ok(())
    }
}
