//! Retention Policy and Storage Bounds Test Suite (Phase 6)

use brain_core::events::{
    CorrelationId, EventSource, OperationId, SemanticStage, TaskProgress, TaskState,
};
use brain_observability::CorrelationIndex;
use std::time::SystemTime;

#[test]
fn test_correlation_index_prunes_completed_timelines() {
    let mut index = CorrelationIndex::new();

    // Ingest 50 active tasks
    for i in 0..50 {
        let corr_id = CorrelationId::from_u128(1000 + i as u128);
        let op_id = OperationId::from_u128(2000 + i as u128);
        let progress = TaskProgress {
            operation_id: op_id,
            correlation_id: corr_id,
            source: EventSource::Domain,
            state: TaskState::Progressing {
                stage: SemanticStage::Extraction,
                completed_items: Some(i),
                total_items: Some(50),
            },
            sequence: 1,
            timestamp: SystemTime::now(),
        };
        index.ingest(&progress);
    }

    // Ingest 50 completed tasks
    for i in 0..50 {
        let corr_id = CorrelationId::from_u128(5000 + i as u128);
        let op_id = OperationId::from_u128(6000 + i as u128);
        let p1 = TaskProgress {
            operation_id: op_id,
            correlation_id: corr_id,
            source: EventSource::Domain,
            state: TaskState::Progressing {
                stage: SemanticStage::Synthesis,
                completed_items: Some(i),
                total_items: Some(50),
            },
            sequence: 1,
            timestamp: SystemTime::now(),
        };
        let p2 = TaskProgress {
            operation_id: op_id,
            correlation_id: corr_id,
            source: EventSource::Domain,
            state: TaskState::Completed,
            sequence: 2,
            timestamp: SystemTime::now(),
        };
        index.ingest(&p1);
        index.ingest(&p2);
    }

    assert_eq!(index.len(), 100);
    assert_eq!(index.total_spans(), 150); // 50*1 + 50*2

    // Prune completed
    let pruned = index.prune_completed();
    assert_eq!(pruned, 50);
    assert_eq!(index.len(), 50);
    assert_eq!(index.total_spans(), 50);
}

#[test]
fn test_correlation_index_enforces_capacity_bounds() {
    let mut index = CorrelationIndex::new();

    // Ingest 500 distinct tasks (250 completed, 250 in-flight)
    for i in 0..500 {
        let corr_id = CorrelationId::from_u128(10000 + i as u128);
        let op_id = OperationId::from_u128(20000 + i as u128);
        let p = TaskProgress {
            operation_id: op_id,
            correlation_id: corr_id,
            source: EventSource::Domain,
            state: if i % 2 == 0 {
                TaskState::Completed
            } else {
                TaskState::Progressing {
                    stage: SemanticStage::Extraction,
                    completed_items: Some(i),
                    total_items: Some(500),
                }
            },
            sequence: 1,
            timestamp: SystemTime::now(),
        };
        index.ingest(&p);
    }

    assert_eq!(index.len(), 500);

    // Enforce max retention capacity of 100
    let pruned = index.enforce_retention(100);
    assert!(pruned >= 400);
    assert_eq!(index.len(), 100);
    assert!(index.total_spans() <= 100);
}
