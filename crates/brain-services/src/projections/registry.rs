use crate::projections::{ProjectionId, StateReducer};
use brain_core::errors::BrainError;
use parking_lot::Mutex;
use std::collections::HashMap;

/// Registry mapping typed ProjectionId enums to stateful event projections.
pub struct ReducerRegistry {
    reducers: Mutex<HashMap<ProjectionId, Box<dyn StateReducer>>>,
}

impl ReducerRegistry {
    /// Creates a new empty `ReducerRegistry`.
    pub fn new() -> Self {
        Self {
            reducers: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a reducer. Returns an error if a duplicate name is registered.
    pub fn register(&self, reducer: Box<dyn StateReducer>) -> Result<(), BrainError> {
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

    /// Accesses all registered reducers for execution.
    pub fn with_all_mut<F>(&self, mut f: F) -> Result<(), BrainError>
    where
        F: FnMut(ProjectionId, &mut dyn StateReducer) -> Result<(), BrainError>,
    {
        let mut map = self.reducers.lock();
        for (&id, reducer) in map.iter_mut() {
            f(id, reducer.as_mut())?;
        }
        Ok(())
    }

    /// Accesses a single registered reducer by ID.
    pub fn with_mut<F, R>(&self, id: ProjectionId, f: F) -> Result<R, BrainError>
    where
        F: FnOnce(&mut dyn StateReducer) -> Result<R, BrainError>,
    {
        let mut map = self.reducers.lock();
        if let Some(reducer) = map.get_mut(&id) {
            f(reducer.as_mut())
        } else {
            Err(BrainError::Storage {
                message: format!("Projection ID {:?} not registered in runner", id),
                source: None,
            })
        }
    }
}

impl Default for ReducerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
