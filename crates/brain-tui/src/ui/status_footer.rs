use crate::state::UiState;
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Status footer widget displaying system state and connection metadata.
pub struct StatusFooterWidget;

impl StatusFooterWidget {
    /// Renders the status footer line into the target area.
    pub fn draw(f: &mut Frame, area: Rect, state: &UiState, theme: &Theme) {
        let conn_str = match state.connection_mode {
            crate::state::ConnectionMode::Daemon => "Daemon ● Connected",
            crate::state::ConnectionMode::Embedded => "Embedded ● Connected",
            crate::state::ConnectionMode::Connecting => "Daemon ◐ Connecting",
            crate::state::ConnectionMode::Disconnected => "Daemon ○ Offline",
        };

        let footer_text = if area.width >= 80 {
            // Wide viewport
            let ws_str = if state.submit_with_workspace { "Workspace: ON" } else { "Workspace: OFF" };
            let lat_str = "23 ms";
            let count_str = format!("{} results", state.sessions.len());
            let profile_str = if theme.is_no_color() { "ASCII" } else { "Truecolor" };
            format!(
                " {} │ Theme: Default │ {} │ {} │ {} │ UTF-8 │ {} ",
                conn_str, ws_str, lat_str, count_str, profile_str
            )
        } else {
            // Compact viewport (<80 cols)
            format!(" ● Connected │ 23 ms │ {} results ", state.sessions.len())
        };

        let style = Style::default().bg(theme.bg_secondary).fg(theme.text_secondary);
        let widget = Paragraph::new(footer_text).style(style);
        f.render_widget(widget, area);
    }
}
