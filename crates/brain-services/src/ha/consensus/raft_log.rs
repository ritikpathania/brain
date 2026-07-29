#![allow(missing_docs)]

use crate::ha::consensus::models::*;
use crate::ha::intent_log::*;
use crate::ha::models::*;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

#[async_trait]
pub trait CommitNotifier: Send + Sync {
    async fn wait_for_commit(
        &self,
        sequence: SequenceNumber,
    ) -> Result<ReplicatedIntent, IntentLogError>;
}

pub struct MockRaftIntentLog {
    records: Arc<Mutex<Vec<ReplicatedIntent>>>,
    local_completed: Arc<Mutex<HashSet<EffectId>>>,
}

impl Default for MockRaftIntentLog {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRaftIntentLog {
    pub fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            local_completed: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn is_locally_completed(&self, effect_id: EffectId) -> bool {
        self.local_completed.lock().contains(&effect_id)
    }
}

#[async_trait]
impl CommitNotifier for MockRaftIntentLog {
    async fn wait_for_commit(
        &self,
        sequence: SequenceNumber,
    ) -> Result<ReplicatedIntent, IntentLogError> {
        let recs = self.records.lock();
        recs.iter()
            .find(|r| r.sequence == sequence)
            .cloned()
            .ok_or_else(|| {
                IntentLogError::Storage(format!("Sequence {:?} not committed", sequence))
            })
    }
}

#[async_trait]
impl IntentLog for MockRaftIntentLog {
    async fn append_record(&self, record: &IntentRecord) -> Result<(), IntentLogError> {
        let mut recs = self.records.lock();
        let intent = ReplicatedIntent {
            sequence: record.sequence,
            event_id: record.event_id,
            effect_id: record.effect_id,
            created_at: record.created_at,
            effect: record.effect.clone(),
        };
        recs.push(intent);
        Ok(())
    }

    async fn update_status(
        &self,
        effect_id: EffectId,
        status: IntentStatus,
    ) -> Result<(), IntentLogError> {
        if status == IntentStatus::Completed {
            self.local_completed.lock().insert(effect_id);
        }
        Ok(())
    }

    async fn load_from(
        &self,
        sequence: SequenceNumber,
    ) -> Result<Vec<IntentRecord>, IntentLogError> {
        let recs = self.records.lock();
        let completed = self.local_completed.lock();

        Ok(recs
            .iter()
            .filter(|r| r.sequence.0 >= sequence.0)
            .map(|r| {
                let st = if completed.contains(&r.effect_id) {
                    IntentStatus::Completed
                } else {
                    IntentStatus::Persisted
                };
                IntentRecord {
                    sequence: r.sequence,
                    event_id: r.event_id,
                    effect_id: r.effect_id,
                    created_at: r.created_at,
                    effect: r.effect.clone(),
                    status: st,
                }
            })
            .collect())
    }

    async fn scan_pending(&self) -> Result<Vec<IntentRecord>, IntentLogError> {
        self.load_from(SequenceNumber(0)).await.map(|recs| {
            recs.into_iter()
                .filter(|r| r.status != IntentStatus::Completed)
                .collect()
        })
    }
}
