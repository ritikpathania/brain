//! Knowledge Evolution screen component.

use crate::ui::theme::Theme;
use crate::ui::widgets::evolution_overview::EvolutionOverviewWidget;
use crate::ui::widgets::proposal_diff::ProposalDiffWidget;
use brain_domain::evolution::EvolutionPlan;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Screen view state aggregate for EvolutionScreen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EvolutionScreenState {
    /// Active EvolutionPlan domain aggregate.
    pub plan: EvolutionPlan,
    /// Currently selected index in proposal list.
    pub selected_index: usize,
}

/// Screen component rendering Knowledge Evolution plan reviewer.
pub struct EvolutionScreen<'a> {
    /// Screen view state.
    pub state: &'a EvolutionScreenState,
}

impl<'a> EvolutionScreen<'a> {
    /// Renders screen view into area buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let horiz_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let left_panel = horiz_chunks[0];
        let right_panel = horiz_chunks[1];

        let overview = EvolutionOverviewWidget {
            plan: &self.state.plan,
            selected_index: self.state.selected_index,
        };
        overview.render(left_panel, buf, theme);

        if let Some(proposal) = self.state.plan.proposals.get(self.state.selected_index) {
            let diff_widget = ProposalDiffWidget { proposal };
            diff_widget.render(right_panel, buf, theme);
        }
    }
}
