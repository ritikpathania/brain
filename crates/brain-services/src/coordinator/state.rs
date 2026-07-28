#![allow(missing_docs)]

use parking_lot::RwLock;
use std::sync::Arc;

pub struct CoordinatorState {
    max_queue_depth: usize,
    pending_count: Arc<RwLock<usize>>,
}

impl CoordinatorState {
    pub fn new(max_queue_depth: usize) -> Self {
        Self {
            max_queue_depth,
            pending_count: Arc::new(RwLock::new(0)),
        }
    }

    pub fn max_queue_depth(&self) -> usize {
        self.max_queue_depth
    }

    pub fn pending_task_count(&self) -> usize {
        *self.pending_count.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_state_initialization() {
        let state = CoordinatorState::new(100);
        assert_eq!(state.pending_task_count(), 0);
        assert_eq!(state.max_queue_depth(), 100);
    }
}
