use brain_integrations::dto::v1::{AutomationActionKind, AutomationRuleDto, AutomationTriggerKind};
use std::collections::HashMap;

/// Read-only snapshot of runtime state evaluated deterministically by automation rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    /// Current graph epoch sequence.
    pub current_epoch: u64,
    /// Current graph version epoch sequence.
    pub graph_version: u64,
    /// Number of pending reflection proposals.
    pub pending_proposals_count: usize,
}

/// Manager maintaining automation orchestration rules.
#[derive(Debug, Clone)]
pub struct AutomationRuleManager {
    rules: HashMap<String, AutomationRuleDto>,
}

impl Default for AutomationRuleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomationRuleManager {
    /// Creates a new `AutomationRuleManager` populated with standard rules.
    pub fn new() -> Self {
        let mut rules = HashMap::new();

        let r1 = AutomationRuleDto {
            rule_id: "rule_nightly_merge".to_string(),
            name: "Nightly Duplicate Entity Merge".to_string(),
            trigger_kind: AutomationTriggerKind::CronSchedule,
            action_kind: AutomationActionKind::AutoSimulateAndExecute,
            target_policy_id: "policy_merge_duplicates".to_string(),
            cron_expr: Some("0 2 * * *".to_string()),
            is_active: true,
            last_run_ms: None,
        };

        let r2 = AutomationRuleDto {
            rule_id: "rule_epoch_fact_prune".to_string(),
            name: "Epoch Fact Pruning Rule".to_string(),
            trigger_kind: AutomationTriggerKind::EpochInterval,
            action_kind: AutomationActionKind::AutoGeneratePlan,
            target_policy_id: "policy_prune_superseded".to_string(),
            cron_expr: None,
            is_active: true,
            last_run_ms: None,
        };

        let r3 = AutomationRuleDto {
            rule_id: "rule_proposal_threshold_strengthen".to_string(),
            name: "Proposal Threshold Strengthening Rule".to_string(),
            trigger_kind: AutomationTriggerKind::PendingProposalsThreshold,
            action_kind: AutomationActionKind::AutoSimulateAndExecute,
            target_policy_id: "policy_strengthen_edges".to_string(),
            cron_expr: None,
            is_active: true,
            last_run_ms: None,
        };

        rules.insert(r1.rule_id.clone(), r1);
        rules.insert(r2.rule_id.clone(), r2);
        rules.insert(r3.rule_id.clone(), r3);

        Self { rules }
    }

    /// Returns catalog of automation rules sorted deterministically by rule ID.
    pub fn list_rules(&self) -> Vec<AutomationRuleDto> {
        let mut list: Vec<AutomationRuleDto> = self.rules.values().cloned().collect();
        list.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
        list
    }

    /// Returns rule by ID, if found.
    pub fn get_rule(&self, rule_id: &str) -> Option<AutomationRuleDto> {
        self.rules.get(rule_id).cloned()
    }

    /// Toggles active status of an automation rule.
    pub fn toggle_rule(&mut self, rule_id: &str) -> Option<AutomationRuleDto> {
        let rule = self.rules.get_mut(rule_id)?;
        rule.is_active = !rule.is_active;
        Some(rule.clone())
    }

    /// Evaluates rules against a frozen `RuntimeSnapshot` returning rules that should trigger.
    pub fn evaluate_rules(&self, snapshot: RuntimeSnapshot) -> Vec<AutomationRuleDto> {
        let mut triggered = Vec::new();
        for rule in self.rules.values() {
            if !rule.is_active {
                continue;
            }
            match rule.trigger_kind {
                AutomationTriggerKind::PendingProposalsThreshold => {
                    if snapshot.pending_proposals_count >= 3 {
                        triggered.push(rule.clone());
                    }
                }
                AutomationTriggerKind::EpochInterval => {
                    if snapshot.current_epoch > 0 && snapshot.current_epoch.is_multiple_of(5) {
                        triggered.push(rule.clone());
                    }
                }
                AutomationTriggerKind::CronSchedule => {
                    // Manual or scheduled tick
                }
            }
        }
        triggered.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
        triggered
    }
}
