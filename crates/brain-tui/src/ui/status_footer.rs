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
        let dot = match state.connection_mode {
            crate::state::ConnectionMode::Daemon | crate::state::ConnectionMode::Embedded => "●",
            crate::state::ConnectionMode::Connecting => "◐",
            crate::state::ConnectionMode::Disconnected => "○",
        };

        let footer_text = if let Some((ref msg, instant)) = state.transient_message {
            if instant.elapsed() < std::time::Duration::from_secs(5) {
                format!(" {} {}", dot, msg)
            } else {
                Self::build_default_footer(state)
            }
        } else {
            Self::build_default_footer(state)
        };

        let style = Style::default()
            .bg(theme.bg_secondary)
            .fg(theme.text_secondary);
        let widget = Paragraph::new(footer_text).style(style);
        f.render_widget(widget, area);
    }

    fn build_default_footer(state: &UiState) -> String {
        match state.screen {
            crate::ui::navigation::Screen::Home => {
                " ▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents".to_string()
            }
            crate::ui::navigation::Screen::Workspace => {
                " enter to return · space to reply · ctrl+x to delete · ? for shortcuts".to_string()
            }
            _ => {
                " ▍▍ manual mode on · ? for shortcuts".to_string()
            }
        }
    }
}


