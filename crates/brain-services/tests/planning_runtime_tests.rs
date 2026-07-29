//! Integration test suite for Goal Decomposition & Task Planning Engine (Phase 7 Milestone 7.1).

use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::planning::{
    CapabilityId, Constraint, GoalId, GoalIntent, GoalValidator, PlanningRuntime, Priority,
};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "planning_test_origin".to_string(),
        evidence_ids: vec!["ev_707".to_string()],
        confidence: 0.95,
        timestamp_ms: 7500,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_db_migration".to_string()),
        EntityIR {
            id: EntityId("entity_db_migration".to_string()),
            canonical_name: "PostgreSQL Migration Service".to_string(),
            kind: "service".to_string(),
            aliases: vec!["db-migrator".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

#[test]
fn test_planning_runtime_end_to_end_compilation() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Migrate database schema".to_string(),
        context_query: KnowledgeQuery::new().with_text("Migration"),
        constraints: vec![Constraint::MandatoryCapability(CapabilityId(
            "capability_execution".to_string(),
        ))],
        priority: Priority::High,
    };

    let runtime = PlanningRuntime::new();
    let (plan, val_report) = runtime.create_plan(&goal, &ctx).unwrap();

    assert!(val_report.is_valid);
    assert_eq!(plan.goal_id, goal.goal_id);
    assert_eq!(plan.priority, Priority::High);
    assert!(!plan.task_graph.nodes.is_empty());

    let topological_order = plan.task_graph.topological_sort().unwrap();
    assert_eq!(topological_order.len(), plan.task_graph.nodes.len());
}

#[test]
fn test_goal_validator_detects_duplicate_tasks() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Test duplicate task validation".to_string(),
        context_query: KnowledgeQuery::new().with_text("Migration"),
        constraints: vec![],
        priority: Priority::Normal,
    };

    let runtime = PlanningRuntime::new();
    let (mut plan, _) = runtime.create_plan(&goal, &ctx).unwrap();

    // Inject duplicate task ID in graph nodes
    if !plan.task_graph.nodes.is_empty() {
        let dup_node = plan.task_graph.nodes[0].clone();
        plan.task_graph.nodes.push(dup_node);
    }

    let validator = GoalValidator::new();
    let report = validator.validate(&plan);

    assert!(!report.is_valid);
    assert!(report.errors.iter().any(|e| matches!(
        e.kind,
        brain_services::planning::PlanningValidationKind::DuplicateTask
    )));
}
