//! Pure domain reducer for Search Index Projection.

use crate::bkf::events::FactEvent;
use crate::projection::conformance::*;
use crate::projection::errors::*;
use crate::projection::id::*;
use crate::projection::reducer::*;
use crate::projection::search_index::state::*;

/// Domain reducer reducing FactEvents into SearchIndexState.
#[derive(Debug, Clone)]
pub struct SearchIndexReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: SearchIndexState,
}

impl SearchIndexReducer {
    /// Creates a new SearchIndexReducer.
    pub fn new(id: ProjectionId, version: ProjectionVersion) -> Self {
        Self {
            id,
            version,
            state: SearchIndexState::default(),
        }
    }
}

impl ProjectionStateView for SearchIndexReducer {
    type State = SearchIndexState;
    fn state(&self) -> &Self::State {
        &self.state
    }
}

impl ProjectionReducer for SearchIndexReducer {
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
            FactEvent::FactSuperseded { old_fact_id, .. } => {
                self.state.remove_active_fact(old_fact_id);
            }
            FactEvent::FactArchived { fact_id, .. } => {
                self.state.remove_active_fact(fact_id);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state = SearchIndexState::default();
        Ok(())
    }
}
