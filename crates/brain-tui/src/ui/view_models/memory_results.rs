//! Encapsulated presentation view model for interactive memory stewardship collections.

use brain_domain::{MemoryCategory, MemoryState, MemorySummary};

/// Presentation view model for an individual memory record item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryItemViewModel {
    /// Unique memory identifier.
    pub id: String,
    /// Display label / memory title.
    pub display_name: String,
    /// Category classification badge string.
    pub category_badge: String,
    /// Memory lifecycle state badge string.
    pub state_badge: String,
    /// Short content preview snippet.
    pub snippet: String,
    /// Originating source system / engine label.
    pub source_kind: String,
}

impl MemoryItemViewModel {
    /// Constructs a `MemoryItemViewModel` from a domain `MemorySummary`.
    pub fn from_summary(summary: &MemorySummary) -> Self {
        Self {
            id: summary.id.clone(),
            display_name: summary.display_name.clone(),
            category_badge: format!("[{}]", summary.category.badge_text()),
            state_badge: format!("[{}]", summary.state.badge_text()),
            snippet: summary.snippet.clone(),
            source_kind: summary.source_kind.clone(),
        }
    }
}

/// Encapsulated presentation view model for an interactive memory collection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryResultsViewModel {
    /// Ordered list of memory item view models.
    items: Vec<MemoryItemViewModel>,
    /// Private active selection index.
    selected: Option<usize>,
    /// Whether memory selection focus is currently active.
    active: bool,
}

impl MemoryResultsViewModel {
    /// Instantiates a new `MemoryResultsViewModel`.
    pub fn new(items: Vec<MemoryItemViewModel>) -> Self {
        let selected = if items.is_empty() { None } else { Some(0) };
        Self {
            items,
            selected,
            active: false,
        }
    }

    /// Returns a slice of current memory item view models.
    pub fn items(&self) -> &[MemoryItemViewModel] {
        &self.items
    }

    /// Returns true if memory selection focus is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Sets whether memory selection focus is active.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if active && self.selected.is_none() && !self.items.is_empty() {
            self.selected = Some(0);
        }
    }

    /// Returns the currently selected memory item view model, if any.
    pub fn selected_item(&self) -> Option<&MemoryItemViewModel> {
        self.selected.and_then(|idx| self.items.get(idx))
    }

    /// Returns the active selection index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// Navigates selection to the next item.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
            return;
        }

        self.selected = match self.selected {
            Some(curr) => {
                if curr + 1 < self.items.len() {
                    Some(curr + 1)
                } else {
                    Some(curr)
                }
            }
            None => Some(0),
        };
    }

    /// Navigates selection to the previous item.
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
            return;
        }

        self.selected = match self.selected {
            Some(curr) => Some(curr.saturating_sub(1)),
            None => Some(0),
        };
    }

    /// Updates memory list preserving selection stability if selected ID exists.
    pub fn update_items(&mut self, new_items: Vec<MemoryItemViewModel>) {
        let currently_selected_id = self.selected_item().map(|item| item.id.clone());
        self.items = new_items;

        if self.items.is_empty() {
            self.selected = None;
            return;
        }

        if let Some(target_id) = currently_selected_id {
            if let Some(new_idx) = self.items.iter().position(|item| item.id == target_id) {
                self.selected = Some(new_idx);
                return;
            }
        }

        self.selected = Some(0);
    }

    /// Optimistically updates an item's state to Pinned, returning the previous item view model for reconciliation rollback if needed.
    pub fn optimistic_pin(&mut self, id: &str) -> Option<MemoryItemViewModel> {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            let previous = item.clone();
            item.state_badge = format!("[{}]", MemoryState::Pinned.badge_text());
            item.category_badge = format!("[{}]", MemoryCategory::PinnedContext.badge_text());
            Some(previous)
        } else {
            None
        }
    }

    /// Optimistically updates an item's state to Active (unpinned), returning the previous item view model for reconciliation rollback if needed.
    pub fn optimistic_unpin(&mut self, id: &str) -> Option<MemoryItemViewModel> {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            let previous = item.clone();
            item.state_badge = format!("[{}]", MemoryState::Active.badge_text());
            item.category_badge = format!("[{}]", MemoryCategory::ConsolidatedMemory.badge_text());
            Some(previous)
        } else {
            None
        }
    }

    /// Optimistically updates an item's state to Archived, returning the previous item view model for reconciliation rollback if needed.
    pub fn optimistic_archive(&mut self, id: &str) -> Option<MemoryItemViewModel> {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            let previous = item.clone();
            item.state_badge = format!("[{}]", MemoryState::Archived.badge_text());
            Some(previous)
        } else {
            None
        }
    }

    /// Optimistically updates an item's state from Archived back to Active, returning the previous item view model for reconciliation rollback if needed.
    pub fn optimistic_restore(&mut self, id: &str) -> Option<MemoryItemViewModel> {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            let previous = item.clone();
            item.state_badge = format!("[{}]", MemoryState::Active.badge_text());
            Some(previous)
        } else {
            None
        }
    }

    /// Restores a snapshot item view model during reconciliation rollback.
    pub fn rollback_item(&mut self, previous: MemoryItemViewModel) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == previous.id) {
            *item = previous;
        }
    }
}

#[cfg(test)]
#[allow(clippy::useless_vec)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_results_navigation_and_selection_stability() {
        let summaries = vec![
            MemorySummary {
                id: "mem_a".to_string(),
                display_name: "Memory A".to_string(),
                category: MemoryCategory::PinnedContext,
                state: MemoryState::Pinned,
                snippet: "Snippet A".to_string(),
                source_kind: "Graph".to_string(),
            },
            MemorySummary {
                id: "mem_b".to_string(),
                display_name: "Memory B".to_string(),
                category: MemoryCategory::ConsolidatedMemory,
                state: MemoryState::Active,
                snippet: "Snippet B".to_string(),
                source_kind: "Engine".to_string(),
            },
        ];

        let vms: Vec<MemoryItemViewModel> = summaries
            .iter()
            .map(MemoryItemViewModel::from_summary)
            .collect();
        let mut vm = MemoryResultsViewModel::new(vms);

        assert_eq!(vm.selected_index(), Some(0));
        assert_eq!(vm.selected_item().unwrap().id, "mem_a");

        vm.select_next();
        assert_eq!(vm.selected_index(), Some(1));
        assert_eq!(vm.selected_item().unwrap().id, "mem_b");

        // Selection stability on update
        let updated_summaries = vec![MemorySummary {
            id: "mem_b".to_string(),
            display_name: "Memory B".to_string(),
            category: MemoryCategory::ConsolidatedMemory,
            state: MemoryState::Active,
            snippet: "Updated Snippet B".to_string(),
            source_kind: "Engine".to_string(),
        }];
        let updated_vms: Vec<MemoryItemViewModel> = updated_summaries
            .iter()
            .map(MemoryItemViewModel::from_summary)
            .collect();
        vm.update_items(updated_vms);

        assert_eq!(vm.selected_index(), Some(0));
        assert_eq!(vm.selected_item().unwrap().id, "mem_b");
    }

    #[test]
    fn test_optimistic_mutation_and_reconciliation_rollback() {
        let summaries = vec![MemorySummary {
            id: "mem_1".to_string(),
            display_name: "Memory 1".to_string(),
            category: MemoryCategory::ConsolidatedMemory,
            state: MemoryState::Active,
            snippet: "Snippet 1".to_string(),
            source_kind: "Engine".to_string(),
        }];

        let vms: Vec<MemoryItemViewModel> = summaries
            .iter()
            .map(MemoryItemViewModel::from_summary)
            .collect();
        let mut vm = MemoryResultsViewModel::new(vms);

        // Optimistic pin
        let previous = vm.optimistic_pin("mem_1").unwrap();
        assert_eq!(vm.items()[0].state_badge, "[Pinned]");
        assert_eq!(vm.items()[0].category_badge, "[Pinned Context]");

        // Rollback on reconciliation failure
        vm.rollback_item(previous);
        assert_eq!(vm.items()[0].state_badge, "[Active]");
        assert_eq!(vm.items()[0].category_badge, "[Consolidated Memory]");
    }

    #[test]
    fn test_concurrent_mutations_and_partial_rollback() {
        let summaries = vec![
            MemorySummary {
                id: "mem_a".to_string(),
                display_name: "Memory A".to_string(),
                category: MemoryCategory::ConsolidatedMemory,
                state: MemoryState::Active,
                snippet: "Snippet A".to_string(),
                source_kind: "Engine".to_string(),
            },
            MemorySummary {
                id: "mem_b".to_string(),
                display_name: "Memory B".to_string(),
                category: MemoryCategory::ConsolidatedMemory,
                state: MemoryState::Active,
                snippet: "Snippet B".to_string(),
                source_kind: "Engine".to_string(),
            },
        ];

        let vms: Vec<MemoryItemViewModel> = summaries
            .iter()
            .map(MemoryItemViewModel::from_summary)
            .collect();
        let mut vm = MemoryResultsViewModel::new(vms);

        // Optimistically mutate both A and B
        let prev_a = vm.optimistic_pin("mem_a").unwrap();
        let _prev_b = vm.optimistic_pin("mem_b").unwrap();

        assert_eq!(vm.items()[0].state_badge, "[Pinned]");
        assert_eq!(vm.items()[1].state_badge, "[Pinned]");

        // Failure on A -> Roll back A only; B remains pinned
        vm.rollback_item(prev_a);

        assert_eq!(vm.items()[0].state_badge, "[Active]");
        assert_eq!(vm.items()[1].state_badge, "[Pinned]");
        assert_eq!(vm.selected_index(), Some(0));
    }

    #[test]
    fn test_archive_and_restore_optimistic_lifecycle() {
        let summaries = vec![MemorySummary {
            id: "mem_arc".to_string(),
            display_name: "Archived Candidate".to_string(),
            category: MemoryCategory::ConsolidatedMemory,
            state: MemoryState::Active,
            snippet: "Snippet".to_string(),
            source_kind: "Engine".to_string(),
        }];

        let vms: Vec<MemoryItemViewModel> = summaries
            .iter()
            .map(MemoryItemViewModel::from_summary)
            .collect();
        let mut vm = MemoryResultsViewModel::new(vms);

        // Optimistic archive
        let prev_active = vm.optimistic_archive("mem_arc").unwrap();
        assert_eq!(vm.items()[0].state_badge, "[Archived]");

        // Optimistic restore
        let prev_archived = vm.optimistic_restore("mem_arc").unwrap();
        assert_eq!(vm.items()[0].state_badge, "[Active]");

        // Rollback to archived state
        vm.rollback_item(prev_archived);
        assert_eq!(vm.items()[0].state_badge, "[Archived]");

        // Rollback to original active state
        vm.rollback_item(prev_active);
        assert_eq!(vm.items()[0].state_badge, "[Active]");
    }
}
