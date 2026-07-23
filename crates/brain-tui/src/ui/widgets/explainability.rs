use crate::ui::theme::Theme;
use crate::ui::widgets::view_models::{
    ExplanationDetailPaneViewModel, ExplanationSummaryViewModel, ExplanationTimelineViewModel,
    ExplanationViewModel,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;

/// Active panel focus inside the Explainability screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplainabilityPanelFocus {
    /// Target concept summary header.
    Summary,
    /// Chronological causal timeline list.
    #[default]
    Timeline,
    /// Focused step execution details pane.
    StepDetail,
}

/// Strongly-typed interaction intents for Explainability screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainabilityIntent {
    /// Select next timeline step down.
    SelectStepNext,
    /// Select previous timeline step up.
    SelectStepPrev,
    /// Focus specific panel.
    FocusPanel(ExplainabilityPanelFocus),
    /// Return to Knowledge Explorer screen.
    BackToExplorer,
}

/// Stateful container holding Explainability session state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplainabilityState {
    /// Target concept ID being explained.
    pub concept_id: Option<String>,
    /// Selected timeline step index.
    pub selected_step_index: usize,
    /// Active panel focus.
    pub focused_panel: ExplainabilityPanelFocus,
}

impl ExplainabilityState {
    /// Creates a new `ExplainabilityState`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Handler executing `ExplainabilityIntent` transitions.
pub struct ExplanationNavigator;

impl ExplanationNavigator {
    /// Processes an `ExplainabilityIntent` to mutate session state cleanly.
    pub fn process_intent(state: &mut ExplainabilityState, intent: ExplainabilityIntent) {
        match intent {
            ExplainabilityIntent::SelectStepNext => {
                state.selected_step_index = state.selected_step_index.saturating_add(1);
            }
            ExplainabilityIntent::SelectStepPrev => {
                state.selected_step_index = state.selected_step_index.saturating_sub(1);
            }
            ExplainabilityIntent::FocusPanel(panel) => {
                state.focused_panel = panel;
            }
            ExplainabilityIntent::BackToExplorer => {}
        }
    }
}

/// Renders the target concept explanation summary widget.
pub fn draw_explanation_summary_widget(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&ExplanationSummaryViewModel>,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" CONCEPT EXPLAINABILITY SUMMARY ")
        .border_style(theme.border);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "No explanation report loaded for concept.",
                Style::default().fg(Color::DarkGray),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let lines = vec![
        Line::from(vec![
            Span::raw("Concept Label: "),
            Span::styled(
                &vm.concept_label,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Type: "),
            Span::styled(&vm.node_type, Style::default().fg(Color::Yellow)),
            Span::raw("  |  ID: "),
            Span::styled(&vm.concept_id, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::raw("Created At:    "),
            Span::styled(&vm.created_at_text, Style::default().fg(Color::Gray)),
            Span::raw("  |  Total Causal Steps: "),
            Span::styled(
                &vm.total_steps_text,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the chronological causal timeline widget table.
pub fn draw_explanation_timeline_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &ExplanationTimelineViewModel,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" CHRONOLOGICAL CAUSAL TIMELINE ")
        .border_style(border_style);

    if vm.items.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No causal execution steps available.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = [
        "Cursor",
        "Seq",
        "Status",
        "Stage",
        "Title / Event",
        "Timestamp",
    ]
    .iter()
    .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = vm.selected_index == Some(idx);
            let cursor_str = if is_selected { " ▶ " } else { "   " };
            let style = if is_selected {
                theme.border_active.add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cells = vec![
                Span::styled(cursor_str, Style::default().fg(Color::Yellow)),
                Span::styled(item.step_sequence.to_string(), style.fg(Color::Gray)),
                Span::styled(&item.status_badge, style.fg(item.status_color)),
                Span::styled(&item.stage_text, style.fg(Color::Yellow)),
                Span::styled(&item.title, style.add_modifier(Modifier::BOLD)),
                Span::styled(&item.time_text, style.fg(Color::DarkGray)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(8),
        Constraint::Percentage(8),
        Constraint::Percentage(22),
        Constraint::Percentage(42),
        Constraint::Percentage(16),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the stage execution details pane widget.
pub fn draw_explanation_detail_widget(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&ExplanationDetailPaneViewModel>,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" STAGE EXECUTION DETAILS ")
        .border_style(border_style);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "Select a timeline step to inspect causal details.",
                Style::default().fg(Color::DarkGray),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let mut lines = vec![
        Line::from(vec![
            Span::raw("Step ID:    "),
            Span::styled(
                &vm.step_id,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Sequence: "),
            Span::styled(
                vm.step_sequence.to_string(),
                Style::default().fg(Color::Gray),
            ),
            Span::raw("  |  Parent ID: "),
            Span::styled(&vm.parent_step_id_text, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("Stage:      "),
            Span::styled(&vm.stage_text, Style::default().fg(Color::Yellow)),
            Span::raw("  |  Status: "),
            Span::styled(&vm.status_text, Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::raw("Title:      "),
            Span::styled(
                &vm.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Details:    "),
            Span::styled(&vm.description, Style::default().fg(Color::White)),
        ]),
    ];

    if !vm.metadata_items.is_empty() {
        lines.push(Line::from("─── Step Metadata Annotations ───"));
        for (k, v) in &vm.metadata_items {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} : ", k), Style::default().fg(Color::Gray)),
                Span::styled(v, Style::default().fg(Color::Cyan)),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the bottom Command Hint Footer bar for Explainability screen.
pub fn draw_explainability_command_hint_footer(frame: &mut Frame, area: Rect, _theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Scroll Timeline Steps   "),
        Span::styled(
            " Tab ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Switch Focus Panel   "),
        Span::styled(
            " q / Esc ",
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Back to Knowledge Explorer "),
    ]);

    let paragraph = Paragraph::new(hints).style(Style::default());
    frame.render_widget(paragraph, area);
}

/// Top-level coordinator rendering the full Explainability screen layout.
pub fn draw_explainability_screen(
    frame: &mut Frame,
    area: Rect,
    vm: &ExplanationViewModel,
    state: &ExplainabilityState,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),      // Summary Header
            Constraint::Percentage(55), // Timeline List
            Constraint::Min(6),         // Step Detail Pane
            Constraint::Length(1),      // Command Hint Footer
        ])
        .split(area);

    // 1. Draw Summary Header
    draw_explanation_summary_widget(frame, chunks[0], vm.summary.as_ref(), theme);

    // 2. Draw Timeline List
    draw_explanation_timeline_widget(
        frame,
        chunks[1],
        &vm.timeline,
        state.focused_panel == ExplainabilityPanelFocus::Timeline,
        theme,
    );

    // 3. Draw Step Detail Pane
    draw_explanation_detail_widget(
        frame,
        chunks[2],
        vm.detail_pane.as_ref(),
        state.focused_panel == ExplainabilityPanelFocus::StepDetail,
        theme,
    );

    // 4. Draw Command Hint Footer
    draw_explainability_command_hint_footer(frame, chunks[3], theme);
}
