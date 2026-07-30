//! Pure domain reducer for Temporal State Projection.

use crate::bkf::events::FactEvent;
use crate::bkf::fact_version::*;
use crate::projection::errors::*;
use crate::projection::id::*;
use crate::projection::reducer::*;
use crate::projection::temporal_state::models::*;
use crate::projection::temporal_state::state::*;

/// Domain reducer reducing FactEvents into TemporalState.
#[derive(Debug, Clone)]
pub struct TemporalStateReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: TemporalState,
}

impl TemporalStateReducer {
    /// Creates a new TemporalStateReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: TemporalState::default(),
        }
    }

    /// Returns reference to internal temporal state.
    pub fn state(&self) -> &TemporalState {
        &self.state
    }
}

impl ProjectionReducer for TemporalStateReducer {
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
                    let fact_id = TemporalFactId(fact.id.clone());
                    let record = TemporalRecord {
                        id: fact_id,
                        entity_id: assert.subject,
                        predicate_id: assert.predicate,
                        valid_from: fact.temporal.valid_from,
                        valid_until: fact.temporal.valid_to,
                        lifecycle: fact.lifecycle,
                        confidence: fact.confidence,
                        previous_version: fact.supersedes,
                    };
                    self.state.insert_record(record);
                }
            }
            FactEvent::FactSuperseded { old_fact_id, superseded_at, .. } => {
                let old_id = TemporalFactId(old_fact_id.clone());
                self.state.close_interval(&old_id, *superseded_at, FactLifecycle::Superseded);
            }
            FactEvent::FactArchived { fact_id, archived_at } => {
                let archived_id = TemporalFactId(fact_id.clone());
                self.state.close_interval(&archived_id, *archived_at, FactLifecycle::Archived);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = TemporalState::default();
        Ok(())
    }
}
