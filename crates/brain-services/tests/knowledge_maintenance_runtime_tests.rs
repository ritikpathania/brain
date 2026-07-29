//! Integration test suite for Knowledge Maintenance Runtime (Phase 6 Milestone 6.3).

use brain_services::compiler::{EntityIR, EntityId, ProvenanceIR};
use brain_services::reflection::{
    ApprovalDecision, KnowledgeMaintenanceRuntime, MaintenanceConfig, MaintenanceState,
    ReflectionInput,
};
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "maintenance_test_origin".to_string(),
        evidence_ids: vec!["ev_404".to_string()],
        confidence: 0.95,
        timestamp_ms: 6000,
    }
}

fn sample_input() -> ReflectionInput {
    let mut props = std::collections::BTreeMap::new();
    props.insert("conflict".to_string(), "true".to_string());

    let entities = vec![
        EntityIR {
            id: EntityId("entity_main".to_string()),
            canonical_name: "Main Engine".to_string(),
            kind: "component".to_string(),
            aliases: vec![],
            properties: props,
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
        EntityIR {
            id: EntityId("entity_main_dup".to_string()),
            canonical_name: "Main Engine".to_string(),
            kind: "component".to_string(),
            aliases: vec![],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.40,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    ];

    ReflectionInput::new(entities, vec![], 6000)
}

#[test]
fn test_maintenance_runtime_auto_apply_cycle() {
    let runtime = KnowledgeMaintenanceRuntime::new(MaintenanceConfig {
        require_approval: false,
        dry_run: false,
    });

    let input = sample_input();
    let res = runtime.run_cycle(&input).unwrap();

    assert_eq!(res.state, MaintenanceState::Completed);
    assert!(res.evolution_plan.is_some());
    assert!(res.execution_report.is_some());
    assert!(!res.events.is_empty());
}

#[test]
fn test_maintenance_runtime_require_approval_workflow() {
    let runtime = KnowledgeMaintenanceRuntime::new(MaintenanceConfig {
        require_approval: true,
        dry_run: false,
    });

    let input = sample_input();
    let res = runtime.run_cycle(&input).unwrap();

    assert_eq!(res.state, MaintenanceState::WaitingForApproval);
    assert!(res.evolution_plan.is_some());
    assert!(res.execution_report.is_none());

    let plan = res.evolution_plan.unwrap();
    let decision = ApprovalDecision {
        decision_id: Uuid::new_v4(),
        plan_id: plan.plan_id,
        approved_by: "operator_admin".to_string(),
        is_approved: true,
        comments: "Approved for execution".to_string(),
        timestamp_ms: 6001,
    };

    let (_mutations, exec_report) = runtime.execute_approved_plan(&plan, &decision).unwrap();
    assert_eq!(exec_report.applied_proposals.len(), 2);
}

#[test]
fn test_maintenance_runtime_dry_run_mode() {
    let runtime = KnowledgeMaintenanceRuntime::new(MaintenanceConfig {
        require_approval: false,
        dry_run: true,
    });

    let input = sample_input();
    let res = runtime.run_cycle(&input).unwrap();

    assert_eq!(res.state, MaintenanceState::Completed);
    assert!(res.evolution_plan.is_some());
    assert!(res.execution_report.is_none());
}
