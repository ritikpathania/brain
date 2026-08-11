//! Read-only TUI ReasoningTraceWidget visualizing execution report telemetry and phase latencies.

use crate::ui::theme::Theme;
use brain_domain::RuntimeExecutionReport;
use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

/// Read-only diagnostic trace visualization widget.
/// Invariant: Diagnostics observe runtime state but never influence runtime behavior.
#[derive(Debug, Clone)]
pub struct ReasoningTraceWidget<'a> {
    report: Option<&'a RuntimeExecutionReport>,
}

impl<'a> ReasoningTraceWidget<'a> {
    /// Instantiates a new `ReasoningTraceWidget`.
    pub fn new(report: Option<&'a RuntimeExecutionReport>) -> Self {
        Self { report }
    }

    /// Renders the diagnostic trace panel using active TUI Theme tokens.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.border)
            .title(" Reasoning Trace Diagnostics ");

        let content = match self.report {
            Some(rep) => {
                vec![
                    Line::from(vec![
                        Span::styled("Execution ID: ", theme.primary.add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{}", rep.execution_id), theme.text),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "Session Stage: ",
                            theme.primary.add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("{}", rep.session.stage), theme.text),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "Policy Provenance: ",
                            theme.primary.add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(rep.policy_set.policy_version.to_string(), theme.text),
                    ]),
                ]
            }
            None => vec![Line::from(Span::styled(
                "No active reasoning trace available.",
                theme.muted,
            ))],
        };

        let paragraph = Paragraph::new(content).block(block);
        frame.render_widget(paragraph, area);
    }
}
