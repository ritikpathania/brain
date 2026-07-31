use crate::ui::theme::Theme;
use crate::ui::widgets::view_models::{
    HealthViewModel, OrchestratorViewModel, ProjectionLagViewModel, ReflectionViewModel,
    RuntimeDashboardViewModel, TaskHistoryViewModel,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Row, Table};
use ratatui::Frame;

/// Stateful container struct for RuntimeDashboard navigation state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDashboardState {
    /// Selected history item index for keyboard scrolling.
    pub selected_history_index: usize,
}

impl RuntimeDashboardState {
    /// Moves task history selection cursor up.
    pub fn select_previous(&mut self, total_items: usize) {
        if total_items > 0 {
            if self.selected_history_index > 0 {
                self.selected_history_index -= 1;
            } else {
                self.selected_history_index = total_items - 1;
            }
        }
    }

    /// Moves task history selection cursor down.
    pub fn select_next(&mut self, total_items: usize) {
        if total_items > 0 {
            if self.selected_history_index + 1 < total_items {
                self.selected_history_index += 1;
            } else {
                self.selected_history_index = 0;
            }
        }
    }
}

/// Renders the Health & System Overview panel widget.
pub fn draw_health_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &HealthViewModel,
    sequence_text: &str,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" ENGINE HEALTH & SYSTEM OVERVIEW ")
        .border_style(theme.border);

    let mut lines = vec![Line::from(vec![
        Span::raw("Status: "),
        Span::styled(
            format!(" [{}] ", vm.status_text),
            Style::default().fg(vm.color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  Seq: "),
        Span::styled(sequence_text, theme.accent),
        Span::raw("  |  Backend: "),
        Span::styled(&vm.storage_backend, Style::default().fg(Color::Cyan)),
    ])];

    if let Some(reason) = &vm.reason {
        lines.push(Line::from(vec![
            Span::raw("Reason: "),
            Span::styled(reason, Style::default().fg(Color::Red)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the Orchestrator Queue & Task Dispatcher widget.
pub fn draw_orchestrator_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &OrchestratorViewModel,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" ORCHESTRATOR DISPATCHER ")
        .border_style(theme.border);

    let lines = vec![
        Line::from(vec![
            Span::raw("Pending Tasks: "),
            Span::styled(
                &vm.pending_count_text,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | Completed: "),
            Span::styled(&vm.completed_count_text, Style::default().fg(Color::Green)),
            Span::raw(" | Failed: "),
            Span::styled(&vm.failed_count_text, Style::default().fg(Color::Red)),
            Span::raw(" | Dropped: "),
            Span::styled(&vm.dropped_count_text, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::raw("Latencies  —  Last Wait: "),
            Span::styled(&vm.last_wait_text, Style::default().fg(Color::Cyan)),
            Span::raw(" | Last Exec: "),
            Span::styled(&vm.last_exec_text, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Current Task: "),
            Span::styled(&vm.current_running_task_text, theme.primary),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the Projection Engine Lag metrics widget table.
pub fn draw_projection_lag_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &ProjectionLagViewModel,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" PROJECTION ENGINE LAG ")
        .border_style(theme.border);

    if vm.items.is_empty() {
        let empty_msg = Paragraph::new("No registered projection engines.").block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    let header_cells = ["Projection", "Processed", "Max Event", "Lag", "Status"]
        .iter()
        .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .items
        .iter()
        .map(|item| {
            let cells = vec![
                Span::raw(&item.name),
                Span::raw(&item.last_processed),
                Span::raw(&item.max_sequence),
                Span::raw(&item.lag_count),
                Span::styled(
                    &item.status,
                    Style::default().fg(item.color).add_modifier(Modifier::BOLD),
                ),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the Reflection Engine Telemetry widget.
pub fn draw_reflection_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &ReflectionViewModel,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" REFLECTION ENGINE METRICS ")
        .border_style(theme.border);

    let lines = vec![
        Line::from(vec![
            Span::raw("Cycles Run: "),
            Span::styled(&vm.cycles_text, Style::default().fg(Color::Cyan)),
            Span::raw(" | Findings: "),
            Span::styled(&vm.findings_text, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("Commands Executed: "),
            Span::styled(
                &vm.commands_executed_text,
                Style::default().fg(Color::Green),
            ),
            Span::raw(" | Skipped: "),
            Span::styled(&vm.commands_skipped_text, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::raw("Last Duration: "),
            Span::styled(&vm.last_duration_text, Style::default().fg(Color::Magenta)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the Task Execution Trace Log widget.
pub fn draw_task_history_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &TaskHistoryViewModel,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" TASK EXECUTION TRACE HISTORY (Scrollable: ↑/↓, j/k) ")
        .border_style(theme.border);

    if vm.items.is_empty() {
        let empty_p = Paragraph::new(Line::from(vec![Span::styled(
            "No background tasks executed yet.",
            Style::default().fg(Color::DarkGray),
        )]))
        .block(block);
        frame.render_widget(empty_p, area);
        return;
    }

    let header_cells = [
        "Cursor", "Task ID", "Kind", "Priority", "Status", "Wait", "Exec",
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
                Span::styled(&item.id, style),
                Span::styled(&item.kind, style),
                Span::styled(&item.priority, style.fg(item.priority_color)),
                Span::styled(&item.status, style.fg(item.status_color)),
                Span::styled(&item.wait_duration_text, style),
                Span::styled(&item.exec_duration_text, style),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(16),
        Constraint::Percentage(20),
        Constraint::Percentage(16),
        Constraint::Percentage(20),
        Constraint::Percentage(12),
        Constraint::Percentage(12),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the bottom Command Hint Footer bar.
pub fn draw_command_hint_footer(frame: &mut Frame, area: Rect, _theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Scroll History   "),
        Span::styled(
            " r ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Refresh Snapshot   "),
        Span::styled(
            " Tab ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Switch View   "),
        Span::styled(
            " q ",
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Back "),
    ]);

    let paragraph = Paragraph::new(hints).style(Style::default());
    frame.render_widget(paragraph, area);
}

/// Top-level coordinator rendering the full RuntimeDashboard screen layout.
pub fn draw_runtime_dashboard(
    frame: &mut Frame,
    area: Rect,
    vm: &RuntimeDashboardViewModel,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Health Header
            Constraint::Length(5), // Middle Row (Orchestrator + Projection Lag)
            Constraint::Length(5), // Reflection Metrics
            Constraint::Min(6),    // Task Trace History Table
            Constraint::Length(1), // Footer Hint Bar
        ])
        .split(area);

    // 1. Draw Health Header
    draw_health_widget(frame, chunks[0], &vm.health, &vm.sequence_text, theme);

    // 2. Partition Middle Row (Orchestrator + Projection Lag)
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    draw_orchestrator_widget(frame, middle_chunks[0], &vm.orchestrator, theme);
    draw_projection_lag_widget(frame, middle_chunks[1], &vm.projections, theme);

    // 3. Draw Reflection Widget
    draw_reflection_widget(frame, chunks[2], &vm.reflection, theme);

    // 4. Draw Task Trace History Widget
    draw_task_history_widget(frame, chunks[3], &vm.task_history, theme);

    // 5. Draw Command Hint Footer
    draw_command_hint_footer(frame, chunks[4], theme);
}
