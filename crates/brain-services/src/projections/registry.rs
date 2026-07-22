use crate::projections::{ProjectionId, StateReducer};
use brain_core::errors::BrainError;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry mapping typed ProjectionId enums to stateless event projections.
pub struct ReducerRegistry {
    reducers: Mutex<HashMap<ProjectionId, Arc<dyn StateReducer>>>,
}

impl ReducerRegistry {
    /// Creates a new empty `ReducerRegistry`.
    pub fn new() -> Self {
        Self {
            reducers: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a reducer. Returns an error if a duplicate name is registered.
    pub fn register(&self, reducer: Arc<dyn StateReducer>) -> Result<(), BrainError> {
        let mut map = self.reducers.lock();
        let id = reducer.id();
        if map.contains_key(&id) {
            return Err(BrainError::Storage {
                message: format!("Duplicate reducer registration for projection ID: {:?}", id),
                source: None,
            });
        }
        map.insert(id, reducer);
        Ok(())
    }

    /// Returns a list of all registered reducer IDs.
    pub fn ids(&self) -> Vec<ProjectionId> {
        let map = self.reducers.lock();
        map.keys().cloned().collect()
    }

    /// Retrieves a single registered reducer by ID.
    pub fn get(&self, id: ProjectionId) -> Option<Arc<dyn StateReducer>> {
        let map = self.reducers.lock();
        map.get(&id).cloned()
    }

    /// Accesses all registered reducers for execution.
    pub fn with_all<F>(&self, mut f: F) -> Result<(), BrainError>
    where
        F: FnMut(ProjectionId, &dyn StateReducer) -> Result<(), BrainError>,
    {
        let map = self.reducers.lock();
        for (&id, reducer) in map.iter() {
            f(id, reducer.as_ref())?;
        }
        Ok(())
    }
}

impl Default for ReducerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
