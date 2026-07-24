use brain_integrations::dto::v1::{EvolutionActionKind, EvolutionPolicyDto, EvolutionTriggerKind};

/// Manager maintaining governance evolution policies sorted strictly by evaluation priority.
#[derive(Debug, Clone)]
pub struct EvolutionPolicyManager {
    policies: Vec<EvolutionPolicyDto>,
}

impl Default for EvolutionPolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EvolutionPolicyManager {
    /// Creates a new `EvolutionPolicyManager` initialized with standard governance policies.
    pub fn new() -> Self {
        let default_policies = vec![
            EvolutionPolicyDto {
                policy_id: "policy_merge_duplicates".to_string(),
                priority: 10,
                name: "Merge Duplicate Entities Policy".to_string(),
                description: "Automatically identify and merge entity pairs sharing >= 90% property similarity.".to_string(),
                trigger_kind: EvolutionTriggerKind::HighSimilarityDuplicate,
                action_kind: EvolutionActionKind::MergeEntities,
                auto_apply: false,
                created_at_ms: 1700000000000,
            },
            EvolutionPolicyDto {
                policy_id: "policy_prune_superseded".to_string(),
                priority: 20,
                name: "Prune Superseded Facts Policy".to_string(),
                description: "Prune non-canonical facts superseded by updated observations across 2+ epochs.".to_string(),
                trigger_kind: EvolutionTriggerKind::SupersededFactAccumulation,
                action_kind: EvolutionActionKind::PruneFact,
                auto_apply: true,
                created_at_ms: 1700000002000,
            },
            EvolutionPolicyDto {
                policy_id: "policy_strengthen_edges".to_string(),
                priority: 30,
                name: "Strengthen Co-Occurrence Edges Policy".to_string(),
                description: "Strengthen edge weight between concepts demonstrating high co-occurrence frequency.".to_string(),
                trigger_kind: EvolutionTriggerKind::InactivityExceeded,
                action_kind: EvolutionActionKind::StrengthenEdgeWeight,
                auto_apply: true,
                created_at_ms: 1700000004000,
            },
        ];

        let mut manager = Self {
            policies: default_policies,
        };
        manager.sort_policies();
        manager
    }

    /// Sorts policies deterministically by priority index (ascending) then policy ID.
    fn sort_policies(&mut self) {
        self.policies.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.policy_id.cmp(&b.policy_id))
        });
    }

    /// Returns all active governance policies.
    pub fn list_policies(&self) -> Vec<EvolutionPolicyDto> {
        self.policies.clone()
    }

    /// Returns a policy by ID, if found.
    pub fn get_policy(&self, policy_id: &str) -> Option<EvolutionPolicyDto> {
        self.policies
            .iter()
            .find(|p| p.policy_id == policy_id)
            .cloned()
    }
}
