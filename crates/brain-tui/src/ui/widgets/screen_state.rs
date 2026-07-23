use crate::ui::widgets::explainability::ExplainabilityState;
use crate::ui::widgets::knowledge_explorer::KnowledgeExplorerState;
use crate::ui::widgets::runtime_dashboard::RuntimeDashboardState;

/// Shared trait abstraction for focusable, stateful TUI screen states.
pub trait ScreenState {
    /// Returns current primary item selection index.
    fn selected_index(&self) -> usize;
    /// Resets selection and viewport state to defaults.
    fn reset(&mut self);
}

impl ScreenState for RuntimeDashboardState {
    fn selected_index(&self) -> usize {
        self.selected_history_index
    }

    fn reset(&mut self) {
        self.selected_history_index = 0;
    }
}

impl ScreenState for KnowledgeExplorerState {
    fn selected_index(&self) -> usize {
        self.selected_concept_index
    }

    fn reset(&mut self) {
        self.selected_concept_index = 0;
        self.selected_relation_index = 0;
        self.history_stack.clear();
        self.forward_stack.clear();
    }
}

impl ScreenState for ExplainabilityState {
    fn selected_index(&self) -> usize {
        self.selected_step_index
    }

    fn reset(&mut self) {
        self.selected_step_index = 0;
        self.concept_id = None;
    }
}
