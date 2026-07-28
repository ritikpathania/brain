#![allow(missing_docs)]

use crate::distributed::transport::*;
use crate::runtime::events::*;
use crate::runtime::models::*;
use crate::worker::models::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EffectId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentStatus {
    Created,
    Persisted,
    Executing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorDecision {
    AssignTask { task_id: TaskId, worker_id: String },
    ExpireLease { task_id: TaskId, lease_id: u64 },
    RescheduleTask { task_id: TaskId, attempt: u32 },
    MarkWorkerLost { worker_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorEffect {
    Dispatch(TaskAssignment),
    Persist(JournalEvent),
    PublishTelemetry(TaskExecutionEvent),
    EmitWorkerLost(String),
    ScheduleRetry(TaskId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentRecord {
    pub sequence: SequenceNumber,
    pub event_id: EventId,
    pub effect_id: EffectId,
    pub created_at: u64,
    pub effect: CoordinatorEffect,
    pub status: IntentStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_record_structure_and_newtypes() {
        let seq = SequenceNumber(1);
        let event_id = EventId(Uuid::new_v4());
        let effect_id = EffectId(Uuid::new_v4());

        let record = IntentRecord {
            sequence: seq,
            event_id,
            effect_id,
            created_at: 1000,
            effect: CoordinatorEffect::EmitWorkerLost("worker-1".to_string()),
            status: IntentStatus::Created,
        };

        assert_eq!(record.sequence, SequenceNumber(1));
        assert_eq!(record.status, IntentStatus::Created);
    }
}
