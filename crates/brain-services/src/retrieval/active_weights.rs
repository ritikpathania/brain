use brain_core::errors::BrainError;
use brain_domain::retrieval::models::WeightSnapshot;
use std::sync::{Arc, RwLock};

/// Provides thread-safe access to the currently active weight snapshot.
pub trait ActiveWeightProvider: Send + Sync {
    /// Retrieve the currently active weight snapshot.
    fn active_snapshot(&self) -> Result<Arc<WeightSnapshot>, BrainError>;

    /// Swap/replace the currently active weight snapshot atomically.
    fn swap_active(&self, new_snapshot: WeightSnapshot) -> Result<(), BrainError>;
}

/// Memory-backed default implementation of `ActiveWeightProvider`.
pub struct DefaultActiveWeightProvider {
    active: RwLock<Arc<WeightSnapshot>>,
}

impl DefaultActiveWeightProvider {
    /// Create a new `DefaultActiveWeightProvider` with an initial snapshot.
    pub fn new(initial: WeightSnapshot) -> Self {
        Self {
            active: RwLock::new(Arc::new(initial)),
        }
    }
}

impl ActiveWeightProvider for DefaultActiveWeightProvider {
    fn active_snapshot(&self) -> Result<Arc<WeightSnapshot>, BrainError> {
        let guard = self.active.read().map_err(|e| BrainError::Internal {
            message: format!("Failed to read active weight snapshot lock: {}", e),
        })?;
        Ok(guard.clone())
    }

    fn swap_active(&self, new_snapshot: WeightSnapshot) -> Result<(), BrainError> {
        let mut guard = self.active.write().map_err(|e| BrainError::Internal {
            message: format!("Failed to write active weight snapshot lock: {}", e),
        })?;
        *guard = Arc::new(new_snapshot);
        Ok(())
    }
}
