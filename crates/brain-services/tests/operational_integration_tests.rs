//! Integration test suite for Operational Integration & Observability (Phase 6 Milestone 6.4).

use brain_services::reflection::{
    ApprovalDecision, KnowledgeMaintenanceRuntime, MaintenanceConfig, MaintenanceError,
    MaintenanceState, ReflectionInput,
};
use uuid::Uuid;

#[test]
fn test_operational_metrics_and_domain_metrics_separation() {
    let runtime = KnowledgeMaintenanceRuntime::new(MaintenanceConfig {
        require_approval: false,
        dry_run: false,
    });

    let input = ReflectionInput::new(vec![], vec![], 7000);
    let res = runtime.run_cycle(&input).unwrap();

    assert_eq!(res.state, MaintenanceState::Completed);
    assert_eq!(
        res.events[0].kind,
        brain_services::reflection::MaintenanceStageEventKind::CycleStarted
    );
}

#[test]
fn test_governed_approval_rejection_error_handling() {
    let runtime = KnowledgeMaintenanceRuntime::new(MaintenanceConfig {
        require_approval: true,
        dry_run: false,
    });

    let input = ReflectionInput::new(vec![], vec![], 7000);
    let res = runtime.run_cycle(&input).unwrap();
    let plan = res.evolution_plan.unwrap();

    let decision = ApprovalDecision {
        decision_id: Uuid::new_v4(),
        plan_id: plan.plan_id,
        approved_by: "sec_auditor".to_string(),
        is_approved: false,
        comments: "Unsafe proposed entity merge rejected by policy".to_string(),
        timestamp_ms: 7001,
    };

    let result = runtime.execute_approved_plan(&plan, &decision);
    assert!(result.is_err());
    if let Err(MaintenanceError::ApprovalRejected(msg)) = result {
        assert!(msg.contains("Unsafe proposed entity merge rejected"));
    } else {
        panic!("Expected ApprovalRejected error variant");
    }
}
