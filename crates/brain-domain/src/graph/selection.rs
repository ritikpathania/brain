//! Graph selection and focus state tracking.

use crate::identifiers::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Interactive focus, selection, and expansion state of the Graph Explorer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GraphSelection {
    /// Currently focused node identifier.
    pub focused: Option<NodeId>,
    /// Range selection anchor node identifier.
    pub anchor: Option<NodeId>,
    /// Explicitly expanded node identifiers.
    pub expanded: HashSet<NodeId>,
    /// Explicitly collapsed node identifiers.
    pub collapsed: HashSet<NodeId>,
}

impl GraphSelection {
    /// Creates a new empty GraphSelection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Focuses a specific node.
    pub fn focus(&mut self, node_id: NodeId) {
        self.focused = Some(node_id);
    }

    /// Toggles node expansion state.
    pub fn toggle_expanded(&mut self, node_id: NodeId) {
        if self.expanded.contains(&node_id) {
            self.expanded.remove(&node_id);
        } else {
            self.expanded.insert(node_id);
            self.collapsed.remove(&node_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_selection_lifecycle() {
        let mut sel = GraphSelection::new();
        let node_id = NodeId::new();

        assert_eq!(sel.focused, None);
        sel.focus(node_id);
        assert_eq!(sel.focused, Some(node_id));

        sel.toggle_expanded(node_id);
        assert!(sel.expanded.contains(&node_id));
    }
}
