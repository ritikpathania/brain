#![allow(missing_docs)]

use crate::ha::executor::*;
use crate::ha::intent_log::*;
use crate::ha::models::*;
use std::sync::Arc;

pub struct IntentReplayEngine<L: IntentLog, E: CoordinatorEffectExecutor> {
    log: Arc<L>,
    executor: Arc<E>,
}

impl<L: IntentLog, E: CoordinatorEffectExecutor> IntentReplayEngine<L, E> {
    pub fn new(log: Arc<L>, executor: Arc<E>) -> Self {
        Self { log, executor }
    }

    pub async fn replay_pending(&self) -> Result<(), IntentLogError> {
        let pending = self.log.scan_pending().await?;

        for record in pending {
            match record.status {
                IntentStatus::Completed => continue,
                IntentStatus::Created
                | IntentStatus::Persisted
                | IntentStatus::Executing
                | IntentStatus::Failed => {
                    let _ = self
                        .log
                        .update_status(record.effect_id, IntentStatus::Executing)
                        .await;
                    if self
                        .executor
                        .execute_effect(record.effect_id, &record.effect)
                        .await
                        .is_ok()
                    {
                        let _ = self
                            .log
                            .update_status(record.effect_id, IntentStatus::Completed)
                            .await;
                    } else {
                        let _ = self
                            .log
                            .update_status(record.effect_id, IntentStatus::Failed)
                            .await;
                    }
                }
            }
        }
        Ok(())
    }
}
