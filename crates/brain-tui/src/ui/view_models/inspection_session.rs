//! Inspection session tracking active target entity location, relationship selection, and navigation stack.

/// Value object representing a visited inspection location in the navigation stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionLocation {
    /// Unique entity identifier.
    pub entity_id: String,
    /// Currently highlighted relationship index within this location.
    pub selected_relation_idx: Option<usize>,
    /// Viewport scroll offset.
    pub scroll_offset: usize,
}

impl InspectionLocation {
    /// Instantiates a new `InspectionLocation`.
    pub fn new(entity_id: String) -> Self {
        Self {
            entity_id,
            selected_relation_idx: None,
            scroll_offset: 0,
        }
    }
}

/// Decoupled inspection interaction session managing active navigation state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InspectionSession {
    /// Active entity location.
    current: Option<InspectionLocation>,
    /// Historical navigation stack for back-navigation.
    history: Vec<InspectionLocation>,
}

impl InspectionSession {
    /// Instantiates a fresh `InspectionSession`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets the session and navigates to a root target entity ID (e.g. from a fresh search execution).
    pub fn reset_and_inspect(&mut self, entity_id: String) {
        self.history.clear();
        self.current = Some(InspectionLocation::new(entity_id));
    }

    /// Intention-revealing domain method: Navigates to a new target entity ID, pushing current location to history stack.
    pub fn inspect(&mut self, entity_id: String) {
        if let Some(curr) = self.current.take() {
            if curr.entity_id != entity_id {
                self.history.push(curr);
            }
        }
        self.current = Some(InspectionLocation::new(entity_id));
    }

    /// Intention-revealing domain method: Returns true if back navigation is available.
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    /// Intention-revealing domain method: Pops previous location from history and restores it. Returns true if navigated.
    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.current = Some(prev);
            true
        } else {
            false
        }
    }

    /// Intention-revealing domain method: Peeks at the previous location in the navigation stack without modifying state.
    pub fn peek_previous(&self) -> Option<&InspectionLocation> {
        self.history.last()
    }

    /// Returns a reference to the active `InspectionLocation`, if any.
    pub fn current(&self) -> Option<&InspectionLocation> {
        self.current.as_ref()
    }

    /// Returns a mutable reference to the active `InspectionLocation`, if any.
    pub fn current_mut(&mut self) -> Option<&mut InspectionLocation> {
        self.current.as_mut()
    }

    /// Navigates relationship selection down/next given total relations count.
    pub fn select_next_relation(&mut self, total_relations: usize) {
        if total_relations == 0 {
            if let Some(curr) = self.current.as_mut() {
                curr.selected_relation_idx = None;
            }
            return;
        }

        if let Some(curr) = self.current.as_mut() {
            curr.selected_relation_idx = match curr.selected_relation_idx {
                Some(idx) => {
                    if idx + 1 < total_relations {
                        Some(idx + 1)
                    } else {
                        Some(idx)
                    }
                }
                None => Some(0),
            };
        }
    }

    /// Navigates relationship selection up/previous given total relations count.
    pub fn select_prev_relation(&mut self, total_relations: usize) {
        if total_relations == 0 {
            if let Some(curr) = self.current.as_mut() {
                curr.selected_relation_idx = None;
            }
            return;
        }

        if let Some(curr) = self.current.as_mut() {
            curr.selected_relation_idx = match curr.selected_relation_idx {
                Some(idx) => Some(idx.saturating_sub(1)),
                None => Some(0),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_stack_lifecycle() {
        let mut session = InspectionSession::new();
        assert!(!session.can_go_back());

        session.inspect("Entity_A".to_string());
        assert_eq!(session.current().unwrap().entity_id, "Entity_A");
        assert!(!session.can_go_back());

        session.inspect("Entity_B".to_string());
        assert_eq!(session.current().unwrap().entity_id, "Entity_B");
        assert_eq!(session.peek_previous().unwrap().entity_id, "Entity_A");
        assert!(session.can_go_back());

        session.inspect("Entity_C".to_string());
        assert_eq!(session.current().unwrap().entity_id, "Entity_C");

        assert!(session.go_back());
        assert_eq!(session.current().unwrap().entity_id, "Entity_B");

        assert!(session.go_back());
        assert_eq!(session.current().unwrap().entity_id, "Entity_A");

        assert!(!session.go_back());
        assert_eq!(session.current().unwrap().entity_id, "Entity_A");
    }

    #[test]
    fn test_search_reset_clears_history() {
        let mut session = InspectionSession::new();
        session.inspect("A".to_string());
        session.inspect("B".to_string());
        assert!(session.can_go_back());

        // Executing a new search resets history stack to new root
        session.reset_and_inspect("X".to_string());
        assert_eq!(session.current().unwrap().entity_id, "X");
        assert!(!session.can_go_back());
    }

    #[test]
    fn test_traversal_cycle_and_consecutive_deduplication() {
        let mut session = InspectionSession::new();
        session.inspect("Entity_A".to_string());
        session.inspect("Entity_B".to_string());

        // Traversal cycle: A -> B -> go_back -> A -> inspect B again
        assert!(session.go_back());
        assert_eq!(session.current().unwrap().entity_id, "Entity_A");

        session.inspect("Entity_B".to_string());
        assert_eq!(session.current().unwrap().entity_id, "Entity_B");
        assert!(session.can_go_back());

        assert!(session.go_back());
        assert_eq!(session.current().unwrap().entity_id, "Entity_A");

        // Inspecting same entity repeatedly does not accumulate duplicate consecutive locations
        session.inspect("Entity_A".to_string());
        assert!(!session.can_go_back());
    }
}
