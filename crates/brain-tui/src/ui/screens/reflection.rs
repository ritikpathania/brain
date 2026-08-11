//! Reflection and Memory Stewardship screen component.

use crate::ui::theme::Theme;
use crate::ui::widgets::contradiction_card::ContradictionCardWidget;
use crate::ui::widgets::reflection_dashboard::ReflectionDashboardWidget;
use crate::ui::widgets::stewardship_list::StewardshipListWidget;
use brain_domain::reflection::StewardshipReport;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Screen view state aggregate for ReflectionScreen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReflectionScreenState {
    /// Active StewardshipReport domain aggregate.
    pub report: StewardshipReport,
    /// Currently selected index in the finding list.
    pub selected_index: usize,
}

/// Screen component rendering Reflection & Memory Stewardship dashboard.
pub struct ReflectionScreen<'a> {
    /// Screen view state.
    pub state: &'a ReflectionScreenState,
}

impl<'a> ReflectionScreen<'a> {
    /// Renders screen view into area buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let horiz_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let left_panel = horiz_chunks[0];
        let right_panel = horiz_chunks[1];

        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(10)])
            .split(left_panel);

        let dashboard_area = vert_chunks[0];
        let list_area = vert_chunks[1];

        let dashboard = ReflectionDashboardWidget {
            report: &self.state.report,
        };
        dashboard.render(dashboard_area, buf, theme);

        let list = StewardshipListWidget {
            report: &self.state.report,
            selected_index: self.state.selected_index,
        };
        list.render(list_area, buf, theme);

        if let Some(finding) = self.state.report.findings.get(self.state.selected_index) {
            let card = ContradictionCardWidget { finding };
            card.render(right_panel, buf, theme);
        }
    }
}
