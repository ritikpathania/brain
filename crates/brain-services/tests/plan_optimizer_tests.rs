//! Integration test suite for Plan Optimization Engine (Phase 7 Milestone 7.2).

use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::planning::{
    CapabilityId, GoalId, GoalIntent, OptimizationPolicy, OptimizationTransformation,
    PlanOptimizer, PlanningIR, PlanningRuntime, Priority, TaskCandidate, TaskId,
};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use brain_services::reasoning::models::EvidenceRef;
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "plan_optimizer_origin".to_string(),
        evidence_ids: vec!["ev_909".to_string()],
        confidence: 0.95,
        timestamp_ms: 8000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_cache_service".to_string()),
        EntityIR {
            id: EntityId("entity_cache_service".to_string()),
            canonical_name: "Redis Cache Cluster".to_string(),
            kind: "service".to_string(),
            aliases: vec!["redis-cache".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

#[test]
fn test_optimizer_redundant_task_elimination_preserves_evidence() {
    let task_a = TaskId(Uuid::new_v4());
    let task_b = TaskId(Uuid::new_v4());

    let planning_ir = PlanningIR {
        goal_id: GoalId(Uuid::new_v4()),
        candidates: vec![
            TaskCandidate {
                task_id: task_a,
                description: "Flush cache cluster".to_string(),
                required_capabilities: vec![CapabilityId("flush".to_string())],
                evidence: vec![EvidenceRef {
                    entity_id: EntityId("entity_cache_cluster".to_string()),
                    relation_id: None,
                    fact_id: Some(brain_services::compiler::FactId("ev_fact_1".to_string())),
                    weight: 0.9,
                }],
                confidence: 0.85,
            },
            TaskCandidate {
                task_id: task_b,
                description: "Flush cache cluster".to_string(),
                required_capabilities: vec![CapabilityId("audit".to_string())],
                evidence: vec![EvidenceRef {
                    entity_id: EntityId("entity_cache_cluster".to_string()),
                    relation_id: None,
                    fact_id: Some(brain_services::compiler::FactId("ev_fact_2".to_string())),
                    weight: 0.8,
                }],
                confidence: 0.90,
            },
        ],
        alternative_decompositions: vec![],
        constraints: vec![],
        priority: Priority::Normal,
    };

    let optimizer = PlanOptimizer::new();
    let report = optimizer.optimize(planning_ir.clone(), &OptimizationPolicy::default());

    assert_eq!(report.transformed_ir.candidates.len(), 1);
    let master = &report.transformed_ir.candidates[0];

    // Verify capability and evidence merging without losing provenance
    assert_eq!(master.required_capabilities.len(), 2);
    assert_eq!(master.evidence.len(), 2);
    assert_eq!(master.confidence, 0.90);

    assert!(report
        .transformations
        .iter()
        .any(|t| matches!(t, OptimizationTransformation::RedundantTaskMerged { .. })));
}

#[test]
fn test_optimizer_idempotency_property() {
    let task_id = TaskId(Uuid::new_v4());
    let planning_ir = PlanningIR {
        goal_id: GoalId(Uuid::new_v4()),
        candidates: vec![
            TaskCandidate {
                task_id,
                description: "Low confidence step".to_string(),
                required_capabilities: vec![],
                evidence: vec![],
                confidence: 0.10,
            },
            TaskCandidate {
                task_id: TaskId(Uuid::new_v4()),
                description: "Valid step".to_string(),
                required_capabilities: vec![],
                evidence: vec![],
                confidence: 0.90,
            },
        ],
        alternative_decompositions: vec![vec![task_id], vec![task_id]],
        constraints: vec![],
        priority: Priority::Normal,
    };

    let optimizer = PlanOptimizer::new();
    let policy = OptimizationPolicy::default();

    let run1 = optimizer.optimize(planning_ir, &policy);
    let run2 = optimizer.optimize(run1.transformed_ir.clone(), &policy);

    // Verify idempotency: Optimize(Policy, Optimize(Policy, IR)) == Optimize(Policy, IR)
    assert_eq!(run1.transformed_ir, run2.transformed_ir);
    assert!(run2.transformations.is_empty());
}

#[test]
fn test_planning_runtime_create_plan_with_optimization() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Optimize redis cluster setup".to_string(),
        context_query: KnowledgeQuery::new().with_text("Redis"),
        constraints: vec![],
        priority: Priority::High,
    };

    let runtime = PlanningRuntime::new();
    let policy = OptimizationPolicy {
        minimum_confidence: 0.50,
        enable_redundant_task_elimination: true,
        enable_branch_consolidation: true,
    };

    let (plan, val_report, _opt_report) = runtime
        .create_plan_with_policy(&goal, &ctx, &policy)
        .unwrap();

    assert!(val_report.is_valid);
    assert_eq!(plan.goal_id, goal.goal_id);
    assert_eq!(plan.priority, Priority::High);
}
