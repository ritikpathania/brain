use brain_integrations::dto::v1::AutomationQueueStatus;
use brain_services::automation::{AutomationScheduler, RuntimeSnapshot};
use brain_services::evolution::KnowledgeEvolutionPlanner;

#[test]
fn test_automation_rule_triggering() {
    let planner = KnowledgeEvolutionPlanner::new();
    let scheduler = AutomationScheduler::new(planner);

    let rules = scheduler.list_rules();
    assert_eq!(rules.len(), 3);

    let queued_item = scheduler.trigger_rule("rule_nightly_merge").unwrap();
    assert_eq!(queued_item.rule_id, "rule_nightly_merge");
    assert_eq!(queued_item.status, AutomationQueueStatus::Queued);
    assert!(!queued_item.automation_execution_id.is_empty());
}

#[test]
fn test_automation_scheduler_queue_dispatch_pipeline() {
    let planner = KnowledgeEvolutionPlanner::new();
    let scheduler = AutomationScheduler::new(planner);

    // 1. Trigger rule
    let queued = scheduler.trigger_rule("rule_nightly_merge").unwrap();
    let exec_id = queued.automation_execution_id.clone();

    // 2. Process queue against current graph version 10
    let logs = scheduler.process_queue(10);

    assert_eq!(logs.len(), 1);
    let log = &logs[0];
    assert_eq!(log.automation_execution_id, exec_id);
    assert_eq!(log.rule_id, "rule_nightly_merge");
    assert!(log.plan_id.is_some());
    assert_eq!(log.graph_version, 11);

    // 3. Queue item status updated to Completed
    let queue_items = scheduler.list_queue();
    assert_eq!(queue_items[0].status, AutomationQueueStatus::Completed);
}

#[test]
fn test_queue_state_machine_legal_transitions() {
    let planner = KnowledgeEvolutionPlanner::new();
    let scheduler = AutomationScheduler::new(planner);

    let item = scheduler.trigger_rule("rule_epoch_fact_prune").unwrap();

    // Legal: Queued -> Running
    let running = scheduler
        .transition_status(&item.queue_id, AutomationQueueStatus::Running)
        .unwrap();
    assert_eq!(running.status, AutomationQueueStatus::Running);

    // Illegal: Running -> Queued
    let err1 = scheduler.transition_status(&item.queue_id, AutomationQueueStatus::Queued);
    assert!(err1.is_err());

    // Legal: Running -> Completed
    let completed = scheduler
        .transition_status(&item.queue_id, AutomationQueueStatus::Completed)
        .unwrap();
    assert_eq!(completed.status, AutomationQueueStatus::Completed);

    // Illegal: Completed -> Running
    let err2 = scheduler.transition_status(&item.queue_id, AutomationQueueStatus::Running);
    assert!(err2.is_err());
}

#[test]
fn test_duplicate_scheduler_trigger_deduplication() {
    let planner = KnowledgeEvolutionPlanner::new();
    let scheduler = AutomationScheduler::new(planner);

    let item1 = scheduler.trigger_rule("rule_nightly_merge").unwrap();
    // 2nd duplicate trigger while item1 is Queued
    let item2 = scheduler.trigger_rule("rule_nightly_merge").unwrap();

    // Deduplicated: returns exact same item & queue_id
    assert_eq!(item1.queue_id, item2.queue_id);
    assert_eq!(item1.automation_execution_id, item2.automation_execution_id);
    assert_eq!(scheduler.list_queue().len(), 1);
}

#[test]
fn test_scheduler_restart_deterministic_recovery() {
    let planner = KnowledgeEvolutionPlanner::new();
    let scheduler = AutomationScheduler::new(planner);

    let snapshot = RuntimeSnapshot {
        current_epoch: 10,
        graph_version: 5,
        pending_proposals_count: 4,
    };

    let queued = scheduler.evaluate_snapshot(snapshot);
    assert!(!queued.is_empty());

    let logs = scheduler.process_queue(5);
    assert!(!logs.is_empty());
}
