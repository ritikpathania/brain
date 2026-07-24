use crate::automation::rule::{AutomationRuleManager, RuntimeSnapshot};
use crate::evolution::KnowledgeEvolutionPlanner;
use brain_integrations::dto::v1::{
    AutomationExecutionLogDto, AutomationQueueItemDto, AutomationQueueStatus, AutomationRuleDto,
    EvolutionExecutionOutcome,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Background Automation Scheduler orchestrating rule triggers, queue state machine, and evolution execution.
#[derive(Debug, Clone)]
pub struct AutomationScheduler {
    rule_manager: Arc<parking_lot::Mutex<AutomationRuleManager>>,
    planner: KnowledgeEvolutionPlanner,
    queue: Arc<parking_lot::Mutex<HashMap<String, AutomationQueueItemDto>>>,
    execution_logs: Arc<parking_lot::Mutex<Vec<AutomationExecutionLogDto>>>,
    sequence_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl Default for AutomationScheduler {
    fn default() -> Self {
        Self::new(KnowledgeEvolutionPlanner::new())
    }
}

impl AutomationScheduler {
    /// Creates a new `AutomationScheduler`.
    pub fn new(planner: KnowledgeEvolutionPlanner) -> Self {
        Self {
            rule_manager: Arc::new(parking_lot::Mutex::new(AutomationRuleManager::new())),
            planner,
            queue: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            execution_logs: Arc::new(parking_lot::Mutex::new(Vec::new())),
            sequence_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Accesses the underlying `AutomationRuleManager`.
    pub fn rule_manager(&self) -> Arc<parking_lot::Mutex<AutomationRuleManager>> {
        Arc::clone(&self.rule_manager)
    }

    /// Returns list of active automation rules.
    pub fn list_rules(&self) -> Vec<AutomationRuleDto> {
        self.rule_manager.lock().list_rules()
    }

    /// Toggles active state of an automation rule.
    pub fn toggle_rule(&self, rule_id: &str) -> Option<AutomationRuleDto> {
        self.rule_manager.lock().toggle_rule(rule_id)
    }

    /// Triggers an automation rule, enqueuing a queue item with `automation_execution_id`.
    /// Performs deduplication: returns existing queue item if an uncompleted item for `rule_id` is pending.
    pub fn trigger_rule(&self, rule_id: &str) -> Option<AutomationQueueItemDto> {
        let rule = self.rule_manager.lock().get_rule(rule_id)?;

        let mut queue = self.queue.lock();
        // Deduplication check: if there is already a Queued or Running item for rule_id
        for item in queue.values() {
            if item.rule_id == rule_id
                && (item.status == AutomationQueueStatus::Queued
                    || item.status == AutomationQueueStatus::Running)
            {
                return Some(item.clone());
            }
        }

        let seq = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let queue_id = format!("queue_{:03}", seq);
        let exec_id = format!("exec_{:03}", seq);

        let item = AutomationQueueItemDto {
            queue_id: queue_id.clone(),
            automation_execution_id: exec_id,
            rule_id: rule.rule_id.clone(),
            target_policy_id: rule.target_policy_id.clone(),
            status: AutomationQueueStatus::Queued,
            retry_count: 0,
            scheduled_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            started_at_ms: None,
            completed_at_ms: None,
        };

        queue.insert(queue_id, item.clone());
        Some(item)
    }

    /// Evaluates rules against a frozen `RuntimeSnapshot` and queues candidate items.
    pub fn evaluate_snapshot(&self, snapshot: RuntimeSnapshot) -> Vec<AutomationQueueItemDto> {
        let rules = self.rule_manager.lock().evaluate_rules(snapshot);
        let mut queued = Vec::new();
        for rule in rules {
            if let Some(item) = self.trigger_rule(&rule.rule_id) {
                queued.push(item);
            }
        }
        queued
    }

    /// Transition queue item status following strict state machine rules.
    /// Legal transitions: Queued -> Running -> (Completed | Failed | Cancelled).
    pub fn transition_status(
        &self,
        queue_id: &str,
        new_status: AutomationQueueStatus,
    ) -> Result<AutomationQueueItemDto, String> {
        let mut queue = self.queue.lock();
        let item = queue
            .get_mut(queue_id)
            .ok_or_else(|| format!("Queue item '{}' not found", queue_id))?;

        let current = item.status;
        let is_valid = matches!(
            (current, new_status),
            (
                AutomationQueueStatus::Queued,
                AutomationQueueStatus::Running
            ) | (
                AutomationQueueStatus::Queued,
                AutomationQueueStatus::Cancelled
            ) | (
                AutomationQueueStatus::Running,
                AutomationQueueStatus::Completed
            ) | (
                AutomationQueueStatus::Running,
                AutomationQueueStatus::Failed
            ) | (
                AutomationQueueStatus::Running,
                AutomationQueueStatus::Cancelled
            )
        );

        if !is_valid {
            return Err(format!(
                "Illegal state machine transition from {:?} to {:?}",
                current, new_status
            ));
        }

        item.status = new_status;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if new_status == AutomationQueueStatus::Running {
            item.started_at_ms = Some(now);
        } else if matches!(
            new_status,
            AutomationQueueStatus::Completed
                | AutomationQueueStatus::Failed
                | AutomationQueueStatus::Cancelled
        ) {
            item.completed_at_ms = Some(now);
        }

        Ok(item.clone())
    }

    /// Processes queued execution items by dispatching plan generation, simulation, and execution.
    pub fn process_queue(&self, current_graph_version: u64) -> Vec<AutomationExecutionLogDto> {
        let pending_ids: Vec<String> = {
            let queue = self.queue.lock();
            queue
                .values()
                .filter(|i| i.status == AutomationQueueStatus::Queued)
                .map(|i| i.queue_id.clone())
                .collect()
        };

        let mut produced_logs = Vec::new();

        for queue_id in pending_ids {
            // 1. Transition Queued -> Running
            if self
                .transition_status(&queue_id, AutomationQueueStatus::Running)
                .is_err()
            {
                continue;
            }

            let (rule_id, target_policy_id, exec_id) = {
                let queue = self.queue.lock();
                let item = match queue.get(&queue_id) {
                    Some(i) => i,
                    None => continue,
                };
                (
                    item.rule_id.clone(),
                    item.target_policy_id.clone(),
                    item.automation_execution_id.clone(),
                )
            };

            // 2. Dispatch to KnowledgeEvolutionPlanner
            let plan = self
                .planner
                .generate_plan(&target_policy_id, current_graph_version);

            let (plan_id, audit_record) = match plan {
                Some(p) => {
                    let pid = p.plan_id.clone();
                    let _sim = self.planner.simulate_plan(&pid);
                    let audit = self.planner.execute_plan(&pid, current_graph_version);
                    (Some(pid), audit)
                }
                None => (
                    None,
                    brain_integrations::dto::v1::EvolutionAuditRecordDto {
                        audit_id: format!("audit_fail_{}", queue_id),
                        graph_version: current_graph_version,
                        plan_id: "none".to_string(),
                        policy_name: target_policy_id.clone(),
                        executed_at_ms: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        outcome: EvolutionExecutionOutcome::NotFound,
                        steps_applied_count: 0,
                        summary: format!("Policy '{}' not found", target_policy_id),
                    },
                ),
            };

            let log_id = format!("log_{}", queue_id);
            let is_success = audit_record.outcome == EvolutionExecutionOutcome::Applied;

            let log = AutomationExecutionLogDto {
                log_id,
                automation_execution_id: exec_id,
                rule_id,
                plan_id,
                graph_version: audit_record.graph_version,
                outcome_summary: audit_record.summary,
                executed_at_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            };

            // 3. Transition Running -> Completed or Failed
            let final_status = if is_success {
                AutomationQueueStatus::Completed
            } else {
                AutomationQueueStatus::Failed
            };
            let _ = self.transition_status(&queue_id, final_status);

            self.execution_logs.lock().push(log.clone());
            produced_logs.push(log);
        }

        produced_logs
    }

    /// Cancels a queued item if not yet completed.
    pub fn cancel_queue_item(&self, queue_id: &str) -> Option<AutomationQueueItemDto> {
        self.transition_status(queue_id, AutomationQueueStatus::Cancelled)
            .ok()
    }

    /// Returns list of all queue items.
    pub fn list_queue(&self) -> Vec<AutomationQueueItemDto> {
        let mut list: Vec<AutomationQueueItemDto> = self.queue.lock().values().cloned().collect();
        list.sort_by_key(|a| a.scheduled_at_ms);
        list
    }

    /// Returns execution history logs.
    pub fn list_execution_logs(&self) -> Vec<AutomationExecutionLogDto> {
        self.execution_logs.lock().clone()
    }
}
