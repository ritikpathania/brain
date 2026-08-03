use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use crate::ui::widgets::screen_state::ScreenState;
use crate::ui::widgets::view_models::{
    EvolutionPlanViewModel, EvolutionSimulationViewModel, KnowledgeEvolutionViewModel,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Row, Table};
use ratatui::Frame;

/// Active panel focus within Knowledge Evolution screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KnowledgeEvolutionPanelFocus {
    /// Governance policies listing table.
    #[default]
    PoliciesList,
    /// Generated evolution plan steps pane.
    PlanTimeline,
    /// Simulation impact analysis breakdown pane.
    SimulationReport,
    /// Audit history log table.
    AuditHistory,
}

/// Strongly-typed interaction intents for Knowledge Evolution screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeEvolutionIntent {
    /// Select next policy down.
    SelectNextPolicy,
    /// Select previous policy up.
    SelectPrevPolicy,
    /// Generate draft evolution plan for selected policy.
    GeneratePlanForSelected,
    /// Simulate active evolution plan impact without side effects.
    SimulateActivePlan,
    /// Execute active evolution plan with optimistic concurrency check.
    ExecuteActivePlan,
    /// Focus specific panel.
    FocusPanel(KnowledgeEvolutionPanelFocus),
}

/// Stateful container holding Knowledge Evolution session state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnowledgeEvolutionState {
    /// Selected policy index in catalog table.
    pub selected_policy_index: usize,
    /// Active panel focus.
    pub focused_panel: KnowledgeEvolutionPanelFocus,
    /// Selected plan ID currently loaded.
    pub active_plan_id: Option<String>,
}

impl KnowledgeEvolutionState {
    /// Creates a new `KnowledgeEvolutionState`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScreenState for KnowledgeEvolutionState {
    fn selected_index(&self) -> usize {
        self.selected_policy_index
    }

    fn reset(&mut self) {
        self.selected_policy_index = 0;
        self.focused_panel = KnowledgeEvolutionPanelFocus::PoliciesList;
        self.active_plan_id = None;
    }
}

/// Handler executing `KnowledgeEvolutionIntent` state transitions.
pub struct KnowledgeEvolutionNavigator;

impl KnowledgeEvolutionNavigator {
    /// Processes a `KnowledgeEvolutionIntent` to mutate session state cleanly.
    pub fn process_intent(state: &mut KnowledgeEvolutionState, intent: KnowledgeEvolutionIntent) {
        match intent {
            KnowledgeEvolutionIntent::SelectNextPolicy => {
                state.selected_policy_index = state.selected_policy_index.saturating_add(1);
            }
            KnowledgeEvolutionIntent::SelectPrevPolicy => {
                state.selected_policy_index = state.selected_policy_index.saturating_sub(1);
            }
            KnowledgeEvolutionIntent::GeneratePlanForSelected => {
                state.focused_panel = KnowledgeEvolutionPanelFocus::PlanTimeline;
            }
            KnowledgeEvolutionIntent::SimulateActivePlan => {
                state.focused_panel = KnowledgeEvolutionPanelFocus::SimulationReport;
            }
            KnowledgeEvolutionIntent::ExecuteActivePlan => {
                state.focused_panel = KnowledgeEvolutionPanelFocus::AuditHistory;
            }
            KnowledgeEvolutionIntent::FocusPanel(panel) => {
                state.focused_panel = panel;
            }
        }
    }
}

/// Renders the governance policies listing table.
pub fn draw_governance_policies_list(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeEvolutionViewModel,
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
        .title(" GOVERNANCE EVOLUTION POLICIES ")
        .border_style(border_style);

    if vm.policies.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No active governance evolution policies loaded.",
            theme.style(ThemeToken::TextMuted),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = [
        "Cursor",
        "Priority",
        "Policy Name",
        "Trigger Kind",
        "Action Kind",
        "Mode",
    ]
    .iter()
    .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .policies
        .iter()
        .enumerate()
        .map(|(idx, policy)| {
            let is_selected = vm.selected_policy_index == Some(idx);
            let cursor_str = if is_selected { " ▶ " } else { "   " };
            let style = if is_selected {
                theme.border_active.add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cells = vec![
                Span::styled(cursor_str, theme.style(ThemeToken::Warning)),
                Span::styled(
                    &policy.priority_badge,
                    style.patch(theme.style(ThemeToken::Secondary)),
                ),
                Span::styled(&policy.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    &policy.trigger_badge,
                    style.patch(theme.style(ThemeToken::Info)),
                ),
                Span::styled(
                    &policy.action_badge,
                    style.patch(theme.style(ThemeToken::Success)),
                ),
                Span::styled(
                    &policy.auto_apply_text,
                    style.patch(theme.style(ThemeToken::TextMuted)),
                ),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(10),
        Constraint::Percentage(42),
        Constraint::Percentage(20),
        Constraint::Percentage(14),
        Constraint::Percentage(10),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the generated evolution plan timeline pane.
pub fn draw_evolution_plan_timeline(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&EvolutionPlanViewModel>,
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
        .title(" GENERATED EVOLUTION PLAN STEPS ")
        .border_style(border_style);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "Press [g] to generate an Evolution Plan for the selected policy.",
                theme.style(ThemeToken::TextMuted),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let mut lines = vec![Line::from(vec![
        Span::raw("Plan ID: "),
        Span::styled(
            &vm.plan_id,
            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled(&vm.target_version_text, theme.style(ThemeToken::Warning)),
        Span::raw("  |  Status: "),
        Span::styled(
            &vm.status_badge,
            theme.style(vm.status_token).add_modifier(Modifier::BOLD),
        ),
    ])];

    lines.push(Line::from(""));
    for desc in &vm.step_descriptions {
        lines.push(Line::from(vec![
            Span::styled("   • ", theme.style(ThemeToken::Success)),
            Span::styled(desc, theme.style(ThemeToken::TextPrimary)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the separate side-effect-free simulation impact report pane.
pub fn draw_simulation_impact_report(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&EvolutionSimulationViewModel>,
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
        .title(" SIMULATED IMPACT ANALYSIS ")
        .border_style(border_style);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "Press [s] to simulate plan impact without side effects.",
                theme.style(ThemeToken::TextMuted),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let lines = vec![
        Line::from(vec![
            Span::raw("Plan ID Analyzed: "),
            Span::styled(&vm.plan_id, theme.style(ThemeToken::Info)),
            Span::raw("  |  Risk Level: "),
            Span::styled(
                &vm.risk_badge,
                theme.style(vm.risk_token).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                &vm.entities_affected_text,
                theme.style(ThemeToken::TextPrimary),
            ),
            Span::raw("  |  "),
            Span::styled(&vm.facts_retired_text, theme.style(ThemeToken::TextPrimary)),
            Span::raw("  |  "),
            Span::styled(
                &vm.edges_strengthened_text,
                theme.style(ThemeToken::TextPrimary),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                &vm.confidence_delta_text,
                theme
                    .style(ThemeToken::Success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" (Estimated graph quality enhancement)"),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the Command Hint Footer bar for Knowledge Evolution screen.
pub fn draw_evolution_command_hint_footer(frame: &mut Frame, area: Rect, theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Select Policy   "),
        Span::styled(
            " g ",
            theme
                .style(ThemeToken::Success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Generate Plan   "),
        Span::styled(
            " s ",
            theme
                .style(ThemeToken::Warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Simulate Impact   "),
        Span::styled(
            " e ",
            theme.style(ThemeToken::Danger).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Execute Plan   "),
        Span::styled(
            " q / Esc ",
            theme
                .style(ThemeToken::TextMuted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Exit "),
    ]);

    let paragraph = Paragraph::new(hints).style(Style::default());
    frame.render_widget(paragraph, area);
}

/// Top-level layout coordinator for Knowledge Evolution screen.
pub fn draw_knowledge_evolution_screen(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeEvolutionViewModel,
    state: &KnowledgeEvolutionState,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45), // Governance Policies List
            Constraint::Percentage(30), // Plan Timeline Steps
            Constraint::Min(6),         // Simulation Report
            Constraint::Length(1),      // Command Footer
        ])
        .split(area);

    // 1. Draw Governance Policies List
    draw_governance_policies_list(
        frame,
        chunks[0],
        vm,
        state.focused_panel == KnowledgeEvolutionPanelFocus::PoliciesList,
        theme,
    );

    // 2. Draw Plan Timeline Steps
    draw_evolution_plan_timeline(
        frame,
        chunks[1],
        vm.active_plan.as_ref(),
        state.focused_panel == KnowledgeEvolutionPanelFocus::PlanTimeline,
        theme,
    );

    // 3. Draw Simulation Impact Report
    draw_simulation_impact_report(
        frame,
        chunks[2],
        vm.simulation_report.as_ref(),
        state.focused_panel == KnowledgeEvolutionPanelFocus::SimulationReport,
        theme,
    );

    // 4. Draw Command Footer
    draw_evolution_command_hint_footer(frame, chunks[3], theme);
}
