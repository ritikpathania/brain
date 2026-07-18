use crate::capability::ErasedCapability;
use std::collections::HashMap;
use std::sync::Arc;

/// Generic capability registry managing dynamic capabilities.
pub struct CapabilityRegistry<Target, Context, Error> {
    capabilities: HashMap<String, Arc<dyn ErasedCapability<Target, Context, Error>>>,
}

impl<Target, Context, Error> CapabilityRegistry<Target, Context, Error> {
    /// Create a new empty CapabilityRegistry.
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    /// Register a new capability into the registry.
    pub fn register(&mut self, capability: Arc<dyn ErasedCapability<Target, Context, Error>>) {
        let name = capability.name().to_string();
        self.capabilities.insert(name, capability);
    }

    /// Retrieve a capability by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ErasedCapability<Target, Context, Error>>> {
        self.capabilities.get(name).cloned()
    }

    /// List all registered capabilities.
    pub fn list(&self) -> Vec<Arc<dyn ErasedCapability<Target, Context, Error>>> {
        self.capabilities.values().cloned().collect()
    }
}

impl<Target, Context, Error> Default for CapabilityRegistry<Target, Context, Error> {
    fn default() -> Self {
        Self::new()
    }
}
