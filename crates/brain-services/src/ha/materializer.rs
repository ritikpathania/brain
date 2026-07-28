#![allow(missing_docs)]

use crate::ha::models::*;

pub struct CoordinatorDecisionMaterializer;

impl CoordinatorDecisionMaterializer {
    pub fn materialize(decision: CoordinatorDecision) -> Vec<CoordinatorEffect> {
        match decision {
            CoordinatorDecision::MarkWorkerLost { worker_id } => {
                vec![CoordinatorEffect::EmitWorkerLost(worker_id)]
            }
            CoordinatorDecision::RescheduleTask { task_id, .. } => {
                vec![CoordinatorEffect::ScheduleRetry(task_id)]
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_materializer_preserves_generation_order() {
        let decision = CoordinatorDecision::MarkWorkerLost {
            worker_id: "worker-1".to_string(),
        };

        let effects = CoordinatorDecisionMaterializer::materialize(decision);
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            CoordinatorEffect::EmitWorkerLost("worker-1".to_string())
        );
    }
}
