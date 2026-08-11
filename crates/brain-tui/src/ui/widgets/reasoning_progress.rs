//! Reasoning progress visualization widget and observer state machine for observing runtime stage progress.

use brain_domain::ExecutionId;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// Status of an individual reasoning progress step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    /// Step is waiting to be executed.
    Pending,
    /// Step is currently executing.
    Active,
    /// Step completed successfully with optional summary detail.
    Completed(Option<String>),
    /// Step failed with diagnostic error message.
    Failed(String),
}

/// An individual reasoning stage step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressStep {
    /// Unique stage identifier (e.g. "retrieval", "synthesis", "reflection").
    pub id: &'static str,
    /// Display label text.
    pub label: String,
    /// Current step status.
    pub status: StepStatus,
}

/// Observer state machine tracking reasoning stage progress per execution ID.
#[derive(Debug, Clone, Default)]
pub struct ReasoningProgressState {
    /// Strongly typed active execution identifier.
    pub execution_id: Option<ExecutionId>,
    /// Sequential stage steps.
    pub steps: Vec<ProgressStep>,
    /// Whether the progress widget should be collapsed (e.g. on response token stream start).
    pub is_collapsed: bool,
    /// Optional terminal error message if execution failed without tokens.
    pub terminal_error: Option<String>,
}

impl ReasoningProgressState {
    /// Creates a new empty `ReasoningProgressState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets state for a new execution lifecycle.
    pub fn reset(&mut self, execution_id: ExecutionId) {
        self.execution_id = Some(execution_id);
        self.steps = vec![
            ProgressStep {
                id: "retrieval",
                label: "Retrieving memories".to_string(),
                status: StepStatus::Pending,
            },
            ProgressStep {
                id: "synthesis",
                label: "Synthesizing response".to_string(),
                status: StepStatus::Pending,
            },
            ProgressStep {
                id: "reflection",
                label: "Reflecting on outcome".to_string(),
                status: StepStatus::Pending,
            },
        ];
        self.is_collapsed = false;
        self.terminal_error = None;
    }

    /// Updates the status of a specific stage by ID.
    pub fn update_stage(&mut self, stage_id: &'static str, status: StepStatus) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == stage_id) {
            step.status = status;
        }
    }

    /// Handles first response token arrival — collapses progress display if no error occurred.
    pub fn on_token(&mut self) {
        if self.terminal_error.is_none() {
            self.is_collapsed = true;
        }
    }

    /// Handles execution error or cancellation — retains terminal progress state for diagnosis.
    pub fn on_error(&mut self, err: String) {
        self.is_collapsed = false;
        self.terminal_error = Some(err);
    }
}

/// Transient widget rendering active reasoning progress.
pub struct ReasoningProgressWidget<'a, T: crate::ui::theme::ActiveTheme> {
    /// Progress state snapshot.
    pub state: &'a ReasoningProgressState,
    /// Visual rendering context.
    pub ctx: &'a crate::ui::render::RenderContext<'a, T>,
}

impl<'a, T: crate::ui::theme::ActiveTheme> Widget for ReasoningProgressWidget<'a, T> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        // If collapsed and no terminal error, render nothing (zero height overhead)
        if self.state.is_collapsed && self.state.terminal_error.is_none() {
            return;
        }

        if area.height == 0 || area.width == 0 {
            return;
        }

        let theme = self.ctx.theme;
        let mut y = area.y;

        // Render steps
        for step in &self.state.steps {
            if y >= area.y + area.height {
                break;
            }

            let (icon, style_token, detail_str) = match &step.status {
                StepStatus::Pending => ("○ ", crate::ui::theme::ThemeToken::TextMuted, None),
                StepStatus::Active => {
                    let spinner_frames = ["● ", "◐ ", "◓ ", "◑ ", "◒ "];
                    let frame_idx = (self.ctx.tick / 3) % spinner_frames.len();
                    (
                        spinner_frames[frame_idx],
                        crate::ui::theme::ThemeToken::Primary,
                        None,
                    )
                }
                StepStatus::Completed(summary) => (
                    "✓ ",
                    crate::ui::theme::ThemeToken::Success,
                    summary.as_deref(),
                ),
                StepStatus::Failed(err) => (
                    "✗ ",
                    crate::ui::theme::ThemeToken::Danger,
                    Some(err.as_str()),
                ),
            };

            let icon_style = theme.style(style_token);
            let mut spans = vec![
                Span::styled(icon, icon_style),
                Span::styled(
                    &step.label,
                    theme.style(crate::ui::theme::ThemeToken::TextPrimary),
                ),
            ];

            if let Some(detail) = detail_str {
                spans.push(Span::styled(
                    format!(" ({})", detail),
                    theme.style(crate::ui::theme::ThemeToken::TextMuted),
                ));
            }

            let line = Line::from(spans);
            buf.set_line(area.x, y, &line, area.width);
            y += 1;
        }

        // If there is a terminal execution error, display it prominently
        if let Some(ref err) = self.state.terminal_error {
            if y < area.y + area.height {
                let err_line = Line::from(vec![
                    Span::styled(
                        "✗ Execution error: ",
                        theme.style(crate::ui::theme::ThemeToken::Danger),
                    ),
                    Span::styled(err, theme.style(crate::ui::theme::ThemeToken::TextMuted)),
                ]);
                buf.set_line(area.x, y, &err_line, area.width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_execution_resets_progress_state() {
        let exec_a = ExecutionId::new();
        let exec_b = ExecutionId::new();
        let mut state = ReasoningProgressState::new();

        // Run A
        state.reset(exec_a);
        state.update_stage(
            "retrieval",
            StepStatus::Completed(Some("Retrieved 18 memories".to_string())),
        );
        state.on_token();
        assert!(state.is_collapsed);
        assert_eq!(state.execution_id, Some(exec_a));

        // Start new Execution B -> reset clears steps & un-collapses
        state.reset(exec_b);
        assert_eq!(state.execution_id, Some(exec_b));
        assert!(!state.is_collapsed);
        assert_eq!(state.steps[0].status, StepStatus::Pending);
        assert_eq!(state.terminal_error, None);
    }

    #[test]
    fn test_failed_execution_preserves_terminal_progress() {
        let exec = ExecutionId::new();
        let mut state = ReasoningProgressState::new();
        state.reset(exec);

        state.update_stage(
            "retrieval",
            StepStatus::Completed(Some("18 memories".to_string())),
        );
        state.update_stage("synthesis", StepStatus::Failed("Model timeout".to_string()));

        // Emit error before token
        state.on_error("Model timeout".to_string());

        // Token arrival does NOT collapse when terminal_error is set
        state.on_token();

        assert!(!state.is_collapsed);
        assert_eq!(state.terminal_error, Some("Model timeout".to_string()));
    }
}
