use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use crate::ui::widgets::view_models::{
    ExplanationDetailPaneViewModel, ExplanationSummaryViewModel, ExplanationTimelineViewModel,
    ExplanationViewModel,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Row, Table};
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
        .border_type(BorderType::Rounded)
        .title(" CONCEPT EXPLAINABILITY SUMMARY ")
        .border_style(theme.border);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "No explanation report loaded for concept.",
                theme.style(ThemeToken::TextMuted),
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
                theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Type: "),
            Span::styled(&vm.node_type, theme.style(ThemeToken::Warning)),
            Span::raw("  |  ID: "),
            Span::styled(&vm.concept_id, theme.style(ThemeToken::TextMuted)),
        ]),
        Line::from(vec![
            Span::raw("Created At:    "),
            Span::styled(&vm.created_at_text, theme.style(ThemeToken::TextMuted)),
            Span::raw("  |  Total Causal Steps: "),
            Span::styled(
                &vm.total_steps_text,
                theme
                    .style(ThemeToken::Secondary)
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
        .border_type(BorderType::Rounded)
        .title(" CHRONOLOGICAL CAUSAL TIMELINE ")
        .border_style(border_style);

    if vm.items.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No causal execution steps available.",
            theme.style(ThemeToken::TextMuted),
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
                Span::styled(cursor_str, theme.style(ThemeToken::Warning)),
                Span::styled(
                    item.step_sequence.to_string(),
                    style.patch(theme.style(ThemeToken::TextMuted)),
                ),
                Span::styled(
                    &item.status_badge,
                    style.patch(theme.style(item.status_token)),
                ),
                Span::styled(
                    &item.stage_text,
                    style.patch(theme.style(ThemeToken::Warning)),
                ),
                Span::styled(&item.title, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    &item.time_text,
                    style.patch(theme.style(ThemeToken::TextMuted)),
                ),
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
        .border_type(BorderType::Rounded)
        .title(" STAGE EXECUTION DETAILS ")
        .border_style(border_style);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "Select a timeline step to inspect causal details.",
                theme.style(ThemeToken::TextMuted),
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
                theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Sequence: "),
            Span::styled(
                vm.step_sequence.to_string(),
                theme.style(ThemeToken::TextMuted),
            ),
            Span::raw("  |  Parent ID: "),
            Span::styled(&vm.parent_step_id_text, theme.style(ThemeToken::Warning)),
        ]),
        Line::from(vec![
            Span::raw("Stage:      "),
            Span::styled(&vm.stage_text, theme.style(ThemeToken::Warning)),
            Span::raw("  |  Status: "),
            Span::styled(&vm.status_text, theme.style(ThemeToken::Success)),
        ]),
        Line::from(vec![
            Span::raw("Title:      "),
            Span::styled(
                &vm.title,
                theme
                    .style(ThemeToken::TextPrimary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Details:    "),
            Span::styled(&vm.description, theme.style(ThemeToken::TextPrimary)),
        ]),
    ];

    if !vm.metadata_items.is_empty() {
        lines.push(Line::from("─── Step Metadata Annotations ───"));
        for (k, v) in &vm.metadata_items {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} : ", k), theme.style(ThemeToken::TextMuted)),
                Span::styled(v, theme.style(ThemeToken::Info)),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the bottom Command Hint Footer bar for Explainability screen.
pub fn draw_explainability_command_hint_footer(frame: &mut Frame, area: Rect, theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Scroll Timeline Steps   "),
        Span::styled(
            " Tab ",
            theme
                .style(ThemeToken::Warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Switch Focus Panel   "),
        Span::styled(
            " q / Esc ",
            theme
                .style(ThemeToken::TextMuted)
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
