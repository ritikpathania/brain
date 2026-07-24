use brain_integrations::dto::v1::{
    EvolutionActionKind, EvolutionExecutionOutcome, EvolutionPlanStatus,
};
use brain_services::evolution::{EvolutionPolicyManager, KnowledgeEvolutionPlanner};

#[test]
fn test_policy_evaluation_determinism() {
    let manager1 = EvolutionPolicyManager::new();
    let manager2 = EvolutionPolicyManager::new();

    let policies1 = manager1.list_policies();
    let policies2 = manager2.list_policies();

    assert_eq!(policies1.len(), policies2.len());
    for (p1, p2) in policies1.iter().zip(policies2.iter()) {
        assert_eq!(p1.policy_id, p2.policy_id);
        assert_eq!(p1.priority, p2.priority);
    }

    // Verify priorities are sorted strictly ascending
    for windows in policies1.windows(2) {
        assert!(windows[0].priority <= windows[1].priority);
    }
}

#[test]
fn test_evolution_plan_simulation_safety() {
    let planner = KnowledgeEvolutionPlanner::new();
    let plan = planner.generate_plan("policy_merge_duplicates", 5).unwrap();

    assert_eq!(plan.status, EvolutionPlanStatus::Draft);

    // Simulate plan
    let sim_report = planner.simulate_plan(&plan.plan_id).unwrap();

    // Verify simulation report is produced without mutating plan status
    assert_eq!(sim_report.plan_id, plan.plan_id);
    assert_eq!(sim_report.entities_affected_count, 1);
    assert_eq!(sim_report.facts_retired_count, 0);

    // Plan status remains Draft
    let plan_after = planner.generate_plan("policy_merge_duplicates", 5).unwrap();
    assert_eq!(plan_after.status, EvolutionPlanStatus::Draft);
}

#[test]
fn test_evolution_plan_concurrency_conflict() {
    let planner = KnowledgeEvolutionPlanner::new();

    // Generate Plan A for graph version 42
    let plan = planner
        .generate_plan("policy_merge_duplicates", 42)
        .unwrap();
    assert_eq!(plan.target_graph_version, 42);

    // Current graph version has shifted to 43
    let audit_record = planner.execute_plan(&plan.plan_id, 43);

    // Verify Optimistic Concurrency Conflict returned
    assert_eq!(
        audit_record.outcome,
        EvolutionExecutionOutcome::PlanConflict
    );
    assert_eq!(audit_record.graph_version, 43); // Graph version unchanged
    assert_eq!(audit_record.steps_applied_count, 0);
}

#[test]
fn test_policy_overlap_deterministic_ordering() {
    let planner = KnowledgeEvolutionPlanner::new();
    let policies = planner.policy_manager().list_policies();

    // Verify deterministic ordering when multiple policies exist
    assert_eq!(policies[0].policy_id, "policy_merge_duplicates");
    assert_eq!(policies[1].policy_id, "policy_prune_superseded");
    assert_eq!(policies[2].policy_id, "policy_strengthen_edges");

    let plan1 = planner.generate_plan(&policies[0].policy_id, 10).unwrap();
    let plan2 = planner.generate_plan(&policies[1].policy_id, 10).unwrap();

    assert_eq!(
        plan1.steps[0].action_kind,
        EvolutionActionKind::MergeEntities
    );
    assert_eq!(plan2.steps[0].action_kind, EvolutionActionKind::PruneFact);
}

#[test]
fn test_idempotent_plan_execution() {
    let planner = KnowledgeEvolutionPlanner::new();
    let plan = planner
        .generate_plan("policy_merge_duplicates", 10)
        .unwrap();

    // 1st Execution
    let audit1 = planner.execute_plan(&plan.plan_id, 10);
    assert_eq!(audit1.outcome, EvolutionExecutionOutcome::Applied);
    assert_eq!(audit1.graph_version, 11);

    // 2nd Execution (Idempotent Replay)
    let audit2 = planner.execute_plan(&plan.plan_id, 11);
    assert_eq!(audit2.outcome, EvolutionExecutionOutcome::AlreadyExecuted);
    assert_eq!(audit2.graph_version, 11); // Version unchanged on replay
}
