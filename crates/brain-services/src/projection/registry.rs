//! Registry managing registered projection instances.

use crate::projection::instance::*;
use brain_domain::projection::*;
use std::collections::HashMap;

/// Registry managing active ProjectionInstance entries.
#[derive(Default)]
pub struct ProjectionRegistry {
    instances: HashMap<ProjectionId, ProjectionInstance>,
}

impl ProjectionRegistry {
    /// Creates a new ProjectionRegistry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a projection instance.
    pub fn register(&mut self, instance: ProjectionInstance) -> Result<(), ProjectionError> {
        let id = instance.id();
        if self.instances.contains_key(&id) {
            return Err(ProjectionError::ReducerFailed {
                message: format!("Duplicate projection ID registered: {}", id.as_str()),
            });
        }
        self.instances.insert(id, instance);
        Ok(())
    }

    /// Gets reference to an instance.
    pub fn get(&self, id: &ProjectionId) -> Option<&ProjectionInstance> {
        self.instances.get(id)
    }

    /// Gets mutable reference to an instance.
    pub fn get_mut(&mut self, id: &ProjectionId) -> Option<&mut ProjectionInstance> {
        self.instances.get_mut(id)
    }

    /// Returns iterator over mutable instances.
    pub fn instances_mut(&mut self) -> impl Iterator<Item = &mut ProjectionInstance> {
        self.instances.values_mut()
    }
}
