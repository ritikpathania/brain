//! Integration test suite for Knowledge Evolution Planning & Execution Engine (Phase 6 Milestone 6.2).

use brain_services::compiler::{EntityId, FactId};
use brain_services::evolution::{
    EvolutionActionKind, EvolutionExecutorV2, EvolutionPlannerV2, PlanValidatorV2,
    ProposalExecutionState,
};
use brain_services::reflection::{
    ContradictionDetails, DuplicateEntityDetails, FindingId, ReflectionFindingKind,
    ReflectionFindingV2, ReflectionReportV2, SnapshotId,
};
use uuid::Uuid;

fn sample_report() -> ReflectionReportV2 {
    let report_id = Uuid::new_v4();
    let snapshot_id = SnapshotId(Uuid::new_v4());

    let findings = vec![
        ReflectionFindingV2 {
            id: FindingId(Uuid::new_v4()),
            kind: ReflectionFindingKind::DuplicateEntity(DuplicateEntityDetails {
                entity_ids: vec![
                    EntityId("entity_tokio".to_string()),
                    EntityId("entity_tokio_alias".to_string()),
                ],
                similarity_score: 0.98,
            }),
            confidence: 0.98,
        },
        ReflectionFindingV2 {
            id: FindingId(Uuid::new_v4()),
            kind: ReflectionFindingKind::AttributeContradiction(ContradictionDetails {
                entity_id: EntityId("entity_tokio".to_string()),
                conflicting_fact_ids: vec![FactId("fact_101".to_string())],
                description: "Attribute conflict on Tokio runtime version".to_string(),
            }),
            confidence: 0.90,
        },
    ];

    ReflectionReportV2 {
        report_id,
        snapshot_id,
        findings,
        evaluated_entities_count: 2,
        timestamp_ms: 5000,
    }
}

#[test]
fn test_evolution_planner_validator_executor_end_to_end() {
    let report = sample_report();
    let planner = EvolutionPlannerV2::new();
    let validator = PlanValidatorV2::new();
    let executor = EvolutionExecutorV2::new();

    // 1. Plan composition & ProposalGraph dependency ordering
    let plan = planner.plan_from_reflection(&report);
    assert_eq!(plan.proposals.len(), 2);
    assert!(!plan.dependency_graph.nodes.is_empty());

    // 2. Pure validation
    let val_report = validator.validate(&plan);
    assert!(val_report.is_valid);
    assert!(val_report.errors.is_empty());

    // 3. Transactional execution & Intent mutation set generation
    let (mutation_set, exec_report) = executor.execute(&plan).unwrap();

    assert_eq!(exec_report.final_state, ProposalExecutionState::Committed);
    assert_eq!(exec_report.applied_proposals.len(), 2);
    assert!(!exec_report.rollback_occurred);

    assert_eq!(mutation_set.entity_merges.len(), 1);
    assert_eq!(
        mutation_set.entity_merges[0],
        (
            EntityId("entity_tokio".to_string()),
            EntityId("entity_tokio_alias".to_string())
        )
    );
    assert_eq!(mutation_set.fact_supercessions.len(), 1);
}

#[test]
fn test_plan_validator_detects_self_merge() {
    let mut plan = EvolutionPlannerV2::new().plan_from_reflection(&sample_report());

    // Inject self-merge violation
    plan.proposals[0].action = EvolutionActionKind::MergeEntities {
        target_id: EntityId("entity_same".to_string()),
        source_id: EntityId("entity_same".to_string()),
    };

    let validator = PlanValidatorV2::new();
    let val_report = validator.validate(&plan);

    assert!(!val_report.is_valid);
    assert_eq!(val_report.errors[0].code, "SELF_MERGE_FORBIDDEN");

    let executor = EvolutionExecutorV2::new();
    let result = executor.execute(&plan);
    assert!(result.is_err());
}
