//! Integration tests for Phase C Modular Reflection Subsystem & Schedulable Tasks.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_services::mapper::to_reflection_report_dto;
use brain_services::reflection::{
    CentralityTask, EmbeddingRefreshTask, ModularReflectionExecutor, ModularReflectionPlanner,
    ModularReflectionScheduler, OptimizeTask, ReflectionExecutionMode, RepairTask, StrengthenTask,
    SummarizeTask,
};

#[test]
fn test_phase_c_dream_mode_orchestration_plan() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "  STALE FACT  ".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence {
                retention: Default::default(),
                source_reliability: 0.1,
            },
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id2,
            preferred_name: "VERIFIED CONCEPT".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence {
                retention: Default::default(),
                source_reliability: 0.9,
            },
            state: KnowledgeState::Observed,
        },
    ];

    let mut planner = ModularReflectionPlanner::new();
    planner.register_task(RepairTask::new());
    planner.register_task(StrengthenTask::new());
    planner.register_task(EmbeddingRefreshTask::new());
    planner.register_task(CentralityTask::new());
    planner.register_task(SummarizeTask::new());
    planner.register_task(OptimizeTask::new());

    let scheduler = ModularReflectionScheduler::new(planner);
    let plan = scheduler.trigger(ReflectionExecutionMode::Dream);

    assert_eq!(plan.mode, ReflectionExecutionMode::Dream);
    assert_eq!(plan.tasks.len(), 6);

    let executor = ModularReflectionExecutor::new();
    let report = executor.execute_plan(&plan, &mut entities);

    assert_eq!(report.execution_mode, ReflectionExecutionMode::Dream);
    assert_eq!(report.task_reports.len(), 6);

    // Entity normalization in RepairTask normalized "  STALE FACT  " -> "stale fact"
    assert_eq!(entities[0].preferred_name, "stale fact");
    assert_eq!(entities[1].preferred_name, "verified concept");

    // DTO mapping and serialization test
    let dto = to_reflection_report_dto(&report);
    assert_eq!(dto.execution_mode, "dream");
    assert_eq!(dto.task_reports.len(), 6);

    let json = serde_json::to_string(&dto).expect("JSON serialization failed");
    assert!(json.contains("\"execution_mode\":\"dream\""));
}

#[test]
fn test_phase_c_reflection_invariants_and_idempotence() {
    let id1 = EntityId::new();

    let mut entities = vec![CanonicalEntity {
        id: id1,
        preferred_name: "CANONICAL NODE".to_string(),
        aliases: vec!["node".to_string()],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    let mut planner = ModularReflectionPlanner::new();
    planner.register_task(RepairTask::new());
    planner.register_task(StrengthenTask::new());

    let scheduler = ModularReflectionScheduler::new(planner);
    let executor = ModularReflectionExecutor::new();

    // First execution run
    let plan1 = scheduler.trigger(ReflectionExecutionMode::Manual);
    let report1 = executor.execute_plan(&plan1, &mut entities);

    assert_eq!(entities[0].id, id1); // Identity immutability invariant
    assert_eq!(entities[0].preferred_name, "canonical node");
    assert!(report1.total_changes > 0);

    let snapshot_after_run1 = entities.clone();

    // Second execution run (Idempotence & Monotonicity check)
    let plan2 = scheduler.trigger(ReflectionExecutionMode::Manual);
    let report2 = executor.execute_plan(&plan2, &mut entities);

    assert_eq!(entities[0].id, id1);
    assert_eq!(entities, snapshot_after_run1);
    assert_eq!(
        report2.total_changes, 0,
        "Second reflection run applied unexpected changes"
    );
}
