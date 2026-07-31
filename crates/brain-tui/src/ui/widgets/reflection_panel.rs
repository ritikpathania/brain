use crate::ui::theme::Theme;
use brain_integrations::dto::v1::{ReflectionFindingDto, ReflectionReport, ReflectionStatusReport};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

/// State wrapper for the TUI Reflection Panel widget.
#[derive(Debug, Clone, Default)]
pub struct ReflectionPanelState {
    /// Background scheduler status report.
    pub status: Option<ReflectionStatusReport>,
    /// Most recent reflection report.
    pub last_report: Option<ReflectionReport>,
    /// Currently active findings.
    pub active_findings: Vec<ReflectionFindingDto>,
    /// Currently selected finding index for detail browsing.
    pub selected_finding_index: usize,
}

/// Renders the TUI Reflection Inspection Panel.
pub fn draw(
    f: &mut Frame<'_>,
    area: Rect,
    state: &ReflectionPanelState,
    theme: &Theme,
    has_focus: bool,
) {
    let border_style = if has_focus {
        theme.accent
    } else {
        theme.border
    };

    let title_suffix = if has_focus { " (Active Focus) " } else { "" };
    let outer_block = Block::default()
        .title(format!(" Reflection Subsystem Inspector{} ", title_suffix))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner_area = outer_block.inner(area);
    f.render_widget(outer_block, area);

    if inner_area.height < 6 {
        let compact_text = vec![Line::from(Span::styled(
            "Terminal height too compact for reflection inspection.",
            Style::default().fg(Color::Yellow),
        ))];
        f.render_widget(Paragraph::new(compact_text), inner_area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Scheduler Status Header
            Constraint::Min(4),    // Findings & Decisions
        ])
        .split(inner_area);

    // 1. Scheduler Status Header
    let (bg_status_text, bg_status_style) = if let Some(ref st) = state.status {
        if st.background_enabled {
            (
                format!("Running (Interval: {}s)", st.interval_secs),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                "Disabled (Manual Only)".to_string(),
                Style::default().fg(Color::Yellow),
            )
        }
    } else {
        ("Unknown".to_string(), Style::default().fg(Color::Gray))
    };

    let mut status_lines = vec![Line::from(vec![
        Span::styled("Scheduler: ", Style::default().fg(Color::Gray)),
        Span::styled(bg_status_text, bg_status_style),
        Span::styled("  │  Total Cycles: ", Style::default().fg(Color::Gray)),
        Span::styled(
            state
                .status
                .as_ref()
                .map(|s| s.reflections_executed.to_string())
                .unwrap_or_else(|| "0".to_string()),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  │  Last Run: ", Style::default().fg(Color::Gray)),
        Span::styled(
            state
                .status
                .as_ref()
                .and_then(|s| s.last_reflection_duration_ms)
                .map(|d| format!("{} ms", d))
                .unwrap_or_else(|| "N/A".to_string()),
            Style::default().fg(Color::Cyan),
        ),
    ])];

    if let Some(ref r) = state.last_report {
        status_lines.push(Line::from(vec![
            Span::styled("Last Report ID: ", Style::default().fg(Color::Gray)),
            Span::styled(&r.execution_id[..8], Style::default().fg(Color::White)),
            Span::styled(
                "  │  Findings Evaluated: ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                r.findings_processed.to_string(),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  │  Executed: ", Style::default().fg(Color::Gray)),
            Span::styled(
                r.commands_executed.to_string(),
                Style::default().fg(Color::Green),
            ),
            Span::styled("  │  Skipped: ", Style::default().fg(Color::Gray)),
            Span::styled(
                r.skipped_findings.len().to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    } else {
        status_lines.push(Line::from(vec![Span::styled(
            "No reflection cycles recorded yet.",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]));
    }

    let status_block = Block::default()
        .title(" Scheduler Telemetry ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border);
    f.render_widget(Paragraph::new(status_lines).block(status_block), chunks[0]);

    // 2. Findings & Decisions Browser
    let sub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Left Panel: Active Findings List
    let mut finding_lines = Vec::new();
    if state.active_findings.is_empty() {
        finding_lines.push(Line::from(""));
        finding_lines.push(Line::from(vec![Span::styled(
            "  ✔  No active unresolved findings detected.",
            Style::default().fg(Color::Green),
        )]));
    } else {
        for (idx, f_dto) in state.active_findings.iter().enumerate() {
            let is_selected = idx == state.selected_finding_index;
            let item_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "▸ " } else { "  " };

            finding_lines.push(Line::from(vec![
                Span::styled(prefix, item_style),
                Span::styled(
                    format!("[{}] ", f_dto.kind),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("Conf: {:.2} ", f_dto.confidence),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(format!("Targets: {:?}", f_dto.target_ids), item_style),
            ]));
        }
    }

    let findings_block = Block::default()
        .title(format!(
            " Active Findings ({}) ",
            state.active_findings.len()
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border);
    f.render_widget(
        Paragraph::new(finding_lines).block(findings_block),
        sub_chunks[0],
    );

    // Right Panel: Executed Commands & Decisions
    let mut decision_lines = Vec::new();
    if let Some(ref report) = state.last_report {
        if !report.executed_commands.is_empty() {
            decision_lines.push(Line::from(vec![Span::styled(
                "Executed Commands:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
            for cmd in &report.executed_commands {
                decision_lines.push(Line::from(vec![
                    Span::styled("  ✔ ", Style::default().fg(Color::Green)),
                    Span::styled(cmd, Style::default().fg(Color::White)),
                ]));
            }
        }
        if !report.skipped_findings.is_empty() {
            decision_lines.push(Line::from(""));
            decision_lines.push(Line::from(vec![Span::styled(
                "Skipped Findings:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
            for sk in &report.skipped_findings {
                decision_lines.push(Line::from(vec![
                    Span::styled("  ⊘ ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!(
                            "{} (Conf: {:.2}): {}",
                            sk.finding_kind, sk.confidence, sk.reasoning
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    } else {
        decision_lines.push(Line::from(""));
        decision_lines.push(Line::from(vec![Span::styled(
            "  No executed commands or decisions recorded.",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]));
    }

    let decisions_block = Block::default()
        .title(" Executed Commands & Decision Log ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border);
    f.render_widget(
        Paragraph::new(decision_lines).block(decisions_block),
        sub_chunks[1],
    );
}
