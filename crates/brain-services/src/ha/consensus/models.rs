#![allow(missing_docs)]

use crate::ha::models::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicatedIntent {
    pub sequence: SequenceNumber,
    pub event_id: EventId,
    pub effect_id: EffectId,
    pub created_at: u64,
    pub effect: CoordinatorEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalExecutionState {
    Committed,
    Executing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadershipEvent {
    BecameLeader { term: u64 },
    BecameFollower { term: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_replicated_intent_and_leadership_events() {
        let seq = SequenceNumber(1);
        let event_id = EventId(Uuid::new_v4());
        let effect_id = EffectId(Uuid::new_v4());

        let intent = ReplicatedIntent {
            sequence: seq,
            event_id,
            effect_id,
            created_at: 1000,
            effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        };

        assert_eq!(intent.sequence, SequenceNumber(1));

        let ev = LeadershipEvent::BecameLeader { term: 2 };
        assert!(matches!(ev, LeadershipEvent::BecameLeader { term: 2 }));
    }
}
