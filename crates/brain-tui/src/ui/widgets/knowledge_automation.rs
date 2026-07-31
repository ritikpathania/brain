use crate::ui::theme::Theme;
use crate::ui::widgets::screen_state::ScreenState;
use crate::ui::widgets::view_models::KnowledgeAutomationViewModel;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Row, Table};
use ratatui::Frame;

/// Active panel focus within Knowledge Automation screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KnowledgeAutomationPanelFocus {
    /// Automation orchestration rules listing table.
    #[default]
    RulesList,
    /// Scheduled background execution queue table.
    QueueTimeline,
    /// Execution history logs table.
    ExecutionLogs,
}

/// Strongly-typed interaction intents for Knowledge Automation screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeAutomationIntent {
    /// Select next rule down.
    SelectNextRule,
    /// Select previous rule up.
    SelectPrevRule,
    /// Manually trigger execution of selected rule.
    TriggerSelectedRule,
    /// Toggle active state of selected rule.
    ToggleSelectedRule,
    /// Cancel selected queue item.
    CancelQueuedItem,
    /// Focus specific panel.
    FocusPanel(KnowledgeAutomationPanelFocus),
}

/// Stateful container holding Knowledge Automation session state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnowledgeAutomationState {
    /// Selected rule index in rules catalog table.
    pub selected_rule_index: usize,
    /// Active panel focus.
    pub focused_panel: KnowledgeAutomationPanelFocus,
}

impl KnowledgeAutomationState {
    /// Creates a new `KnowledgeAutomationState`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScreenState for KnowledgeAutomationState {
    fn selected_index(&self) -> usize {
        self.selected_rule_index
    }

    fn reset(&mut self) {
        self.selected_rule_index = 0;
        self.focused_panel = KnowledgeAutomationPanelFocus::RulesList;
    }
}

/// Handler executing `KnowledgeAutomationIntent` state transitions.
pub struct KnowledgeAutomationNavigator;

impl KnowledgeAutomationNavigator {
    /// Processes a `KnowledgeAutomationIntent` to mutate session state cleanly.
    pub fn process_intent(state: &mut KnowledgeAutomationState, intent: KnowledgeAutomationIntent) {
        match intent {
            KnowledgeAutomationIntent::SelectNextRule => {
                state.selected_rule_index = state.selected_rule_index.saturating_add(1);
            }
            KnowledgeAutomationIntent::SelectPrevRule => {
                state.selected_rule_index = state.selected_rule_index.saturating_sub(1);
            }
            KnowledgeAutomationIntent::TriggerSelectedRule => {
                state.focused_panel = KnowledgeAutomationPanelFocus::QueueTimeline;
            }
            KnowledgeAutomationIntent::ToggleSelectedRule => {
                // Toggles rule active status
            }
            KnowledgeAutomationIntent::CancelQueuedItem => {
                state.focused_panel = KnowledgeAutomationPanelFocus::ExecutionLogs;
            }
            KnowledgeAutomationIntent::FocusPanel(panel) => {
                state.focused_panel = panel;
            }
        }
    }
}

/// Renders the automation orchestration rules table.
pub fn draw_automation_rules_list(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeAutomationViewModel,
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
        .title(" AUTOMATION ORCHESTRATION RULES ")
        .border_style(border_style);

    if vm.rules.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No active automation rules configured.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = [
        "Cursor",
        "Status",
        "Rule Name",
        "Trigger Kind",
        "Action Kind",
        "Target Policy",
    ]
    .iter()
    .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .rules
        .iter()
        .enumerate()
        .map(|(idx, rule)| {
            let is_selected = vm.selected_rule_index == Some(idx);
            let cursor_str = if is_selected { " ▶ " } else { "   " };
            let style = if is_selected {
                theme.border_active.add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cells = vec![
                Span::styled(cursor_str, Style::default().fg(Color::Yellow)),
                Span::styled(&rule.status_badge, style.fg(rule.status_color)),
                Span::styled(&rule.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(&rule.trigger_badge, style.fg(Color::Cyan)),
                Span::styled(&rule.action_badge, style.fg(Color::Magenta)),
                Span::styled(&rule.target_policy_id, style.fg(Color::Gray)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(10),
        Constraint::Percentage(36),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the scheduled background execution queue table.
pub fn draw_automation_queue_timeline(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeAutomationViewModel,
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
        .title(" SCHEDULED EXECUTION QUEUE ")
        .border_style(border_style);

    if vm.queue.is_empty() {
        let p = Paragraph::new(Span::styled(
            "Execution queue is empty.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = [
        "Queue ID",
        "Execution Trace ID",
        "Status",
        "Rule ID",
        "Retries",
    ]
    .iter()
    .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .queue
        .iter()
        .map(|q| {
            let cells = vec![
                Span::styled(&q.queue_id, Style::default().fg(Color::Cyan)),
                Span::styled(
                    &q.automation_execution_id,
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(&q.status_badge, Style::default().fg(q.status_color)),
                Span::styled(&q.rule_id, Style::default().fg(Color::White)),
                Span::styled(&q.retry_count_text, Style::default().fg(Color::DarkGray)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(35),
        Constraint::Percentage(15),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the execution history logs table.
pub fn draw_automation_execution_logs(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeAutomationViewModel,
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
        .title(" AUTOMATION EXECUTION HISTORY LOG ")
        .border_style(border_style);

    if vm.logs.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No execution history logs recorded yet.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = ["Trace ID", "Rule ID", "Plan ID", "Graph Version", "Summary"]
        .iter()
        .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .logs
        .iter()
        .map(|l| {
            let cells = vec![
                Span::styled(
                    &l.automation_execution_id,
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(&l.rule_id, Style::default().fg(Color::Cyan)),
                Span::styled(&l.plan_id_text, Style::default().fg(Color::Magenta)),
                Span::styled(&l.graph_version_text, Style::default().fg(Color::Green)),
                Span::styled(&l.summary, Style::default().fg(Color::White)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(18),
        Constraint::Percentage(12),
        Constraint::Percentage(35),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders Command Hint Footer bar for Knowledge Automation screen.
pub fn draw_automation_command_hint_footer(frame: &mut Frame, area: Rect, _theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Select Rule   "),
        Span::styled(
            " r ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Trigger Rule   "),
        Span::styled(
            " t ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Toggle Active   "),
        Span::styled(
            " c ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Cancel Item   "),
        Span::styled(
            " q / Esc ",
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Exit "),
    ]);

    let paragraph = Paragraph::new(hints).style(Style::default());
    frame.render_widget(paragraph, area);
}

/// Top-level layout coordinator for Knowledge Automation screen.
pub fn draw_knowledge_automation_screen(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeAutomationViewModel,
    state: &KnowledgeAutomationState,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Automation Rules List
            Constraint::Percentage(30), // Scheduled Execution Queue
            Constraint::Min(6),         // Execution History Logs
            Constraint::Length(1),      // Command Footer
        ])
        .split(area);

    // 1. Draw Rules List
    draw_automation_rules_list(
        frame,
        chunks[0],
        vm,
        state.focused_panel == KnowledgeAutomationPanelFocus::RulesList,
        theme,
    );

    // 2. Draw Queue Timeline
    draw_automation_queue_timeline(
        frame,
        chunks[1],
        vm,
        state.focused_panel == KnowledgeAutomationPanelFocus::QueueTimeline,
        theme,
    );

    // 3. Draw Execution Logs
    draw_automation_execution_logs(
        frame,
        chunks[2],
        vm,
        state.focused_panel == KnowledgeAutomationPanelFocus::ExecutionLogs,
        theme,
    );

    // 4. Draw Command Footer
    draw_automation_command_hint_footer(frame, chunks[3], theme);
}
