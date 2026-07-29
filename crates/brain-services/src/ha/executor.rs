#![allow(missing_docs)]

use crate::ha::models::*;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EffectExecutionError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

#[async_trait]
pub trait CoordinatorEffectExecutor: Send + Sync {
    async fn execute_effect(
        &self,
        effect_id: EffectId,
        effect: &CoordinatorEffect,
    ) -> Result<(), EffectExecutionError>;
}

pub struct MockEffectExecutor {
    executed_effects: Arc<Mutex<HashSet<EffectId>>>,
}

impl Default for MockEffectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEffectExecutor {
    pub fn new() -> Self {
        Self {
            executed_effects: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn executed_count(&self) -> usize {
        self.executed_effects.lock().len()
    }
}

#[async_trait]
impl CoordinatorEffectExecutor for MockEffectExecutor {
    async fn execute_effect(
        &self,
        effect_id: EffectId,
        _effect: &CoordinatorEffect,
    ) -> Result<(), EffectExecutionError> {
        let mut executed = self.executed_effects.lock();
        if executed.contains(&effect_id) {
            return Ok(()); // Idempotency check
        }
        executed.insert(effect_id);
        Ok(())
    }
}
