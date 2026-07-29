//! Integration test suite for Execution Planning Engine (Phase 7 Milestone 7.3).

use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::planning::{
    BarrierKind, ExecutionPlanner, ExecutionPlanningPolicy, GoalId, GoalIntent, PlanningRuntime,
    Priority, TaskDependencyEdge, TaskGraph, TaskPlan, TaskStep,
};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "execution_planner_origin".to_string(),
        evidence_ids: vec!["ev_1010".to_string()],
        confidence: 0.95,
        timestamp_ms: 9000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_api_gateway".to_string()),
        EntityIR {
            id: EntityId("entity_api_gateway".to_string()),
            canonical_name: "API Gateway Service".to_string(),
            kind: "service".to_string(),
            aliases: vec!["api-gateway".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

#[test]
fn test_execution_planner_stage_partitioning_and_invariants() {
    let task_1 = brain_services::planning::TaskId(Uuid::new_v4());
    let task_2 = brain_services::planning::TaskId(Uuid::new_v4());
    let task_3 = brain_services::planning::TaskId(Uuid::new_v4());

    let mut graph = TaskGraph::new();

    graph.nodes.push(TaskStep {
        task_id: task_1,
        description: "Fetch configuration".to_string(),
        required_capabilities: vec![],
        confidence: 0.9,
    });
    graph.nodes.push(TaskStep {
        task_id: task_2,
        description: "Parse schema A".to_string(),
        required_capabilities: vec![],
        confidence: 0.9,
    });
    graph.nodes.push(TaskStep {
        task_id: task_3,
        description: "Consolidate results".to_string(),
        required_capabilities: vec![],
        confidence: 0.9,
    });

    // task_1 -> task_3 and task_2 -> task_3 (task_1 and task_2 can execute in parallel in Stage 0)
    graph.edges.push(TaskDependencyEdge {
        source: task_1,
        target: task_3,
    });
    graph.edges.push(TaskDependencyEdge {
        source: task_2,
        target: task_3,
    });

    let task_plan = TaskPlan {
        plan_id: brain_services::planning::PlanId(Uuid::new_v4()),
        goal_id: GoalId(Uuid::new_v4()),
        task_graph: graph,
        priority: Priority::High,
        timestamp_ms: 9000,
    };

    let planner = ExecutionPlanner::new();
    let policy = ExecutionPlanningPolicy::default();

    let exec_plan = planner.plan_execution(&task_plan, &policy).unwrap();

    // 1. Stage count invariant
    assert_eq!(exec_plan.stages.len(), 2);

    // 2. Contiguous stage index invariant (0, 1)
    assert_eq!(exec_plan.stages[0].stage_index, 0);
    assert_eq!(exec_plan.stages[1].stage_index, 1);

    // 3. Parallel task grouping invariant (Stage 0 contains task_1 and task_2 in parallel)
    assert_eq!(exec_plan.stages[0].parallel_tasks.len(), 2);
    assert_eq!(exec_plan.stages[1].parallel_tasks.len(), 1);
    assert_eq!(exec_plan.stages[1].parallel_tasks[0], task_3);

    // 4. Total task coverage invariant
    assert_eq!(exec_plan.total_tasks(), 3);
    assert_eq!(exec_plan.stages[0].barrier_kind, BarrierKind::Strict);
}

#[test]
fn test_execution_planner_determinism_property() {
    let task_1 = brain_services::planning::TaskId(Uuid::new_v4());
    let task_2 = brain_services::planning::TaskId(Uuid::new_v4());

    let mut graph = TaskGraph::new();
    graph.nodes.push(TaskStep {
        task_id: task_1,
        description: "Task 1".to_string(),
        required_capabilities: vec![],
        confidence: 0.9,
    });
    graph.nodes.push(TaskStep {
        task_id: task_2,
        description: "Task 2".to_string(),
        required_capabilities: vec![],
        confidence: 0.9,
    });

    let task_plan = TaskPlan {
        plan_id: brain_services::planning::PlanId(Uuid::new_v4()),
        goal_id: GoalId(Uuid::new_v4()),
        task_graph: graph,
        priority: Priority::Normal,
        timestamp_ms: 9000,
    };

    let planner = ExecutionPlanner::new();
    let policy = ExecutionPlanningPolicy::default();

    let run1 = planner.plan_execution(&task_plan, &policy).unwrap();
    let run2 = planner.plan_execution(&task_plan, &policy).unwrap();

    // Deterministic ordering invariant: PlanExecution(plan) == PlanExecution(plan)
    assert_eq!(run1.stages, run2.stages);
}

#[test]
fn test_planning_runtime_create_execution_plan() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Deploy API Gateway".to_string(),
        context_query: KnowledgeQuery::new().with_text("Gateway"),
        constraints: vec![],
        priority: Priority::Critical,
    };

    let runtime = PlanningRuntime::new();
    let (task_plan, exec_plan) = runtime.create_execution_plan(&goal, &ctx).unwrap();

    assert_eq!(task_plan.goal_id, goal.goal_id);
    assert_eq!(exec_plan.task_plan_id, task_plan.plan_id);
    assert!(exec_plan.stage_count() >= 1);
}
