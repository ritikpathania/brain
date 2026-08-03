use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_integrations::dto::v1::{
    CompilerIrSummaryDto, CompilerStatusReport, DiagnosticDto, KnowledgeCompilationReport,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

/// State model for the TUI Knowledge Compiler Inspection Panel widget.
#[derive(Debug, Clone, Default)]
pub struct CompilerPanelState {
    /// Background compiler status and operational telemetry.
    pub status: Option<CompilerStatusReport>,
    /// Most recent compilation report.
    pub last_report: Option<KnowledgeCompilationReport>,
    /// Compiled Knowledge IR summary.
    pub ir_summary: Option<CompilerIrSummaryDto>,
    /// Active compiler diagnostics list.
    pub diagnostics: Vec<DiagnosticDto>,
    /// Currently selected diagnostic index for detail browsing.
    pub selected_diagnostic_index: usize,
}

/// Renders the TUI Knowledge Compiler Inspection Panel widget.
pub fn draw(
    f: &mut Frame<'_>,
    area: Rect,
    state: &CompilerPanelState,
    theme: &Theme,
    has_focus: bool,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };

    let title_suffix = if has_focus { " (Active Focus) " } else { "" };
    let outer_block = Block::default()
        .title(format!(" Knowledge Compiler Inspector{} ", title_suffix))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner_area = outer_block.inner(area);
    f.render_widget(outer_block, area);

    if inner_area.height < 6 {
        let compact_text = vec![Line::from(Span::styled(
            "Terminal height too compact for compiler inspection panel.",
            theme.style(ThemeToken::Warning),
        ))];
        f.render_widget(Paragraph::new(compact_text), inner_area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Status & Telemetry Header
            Constraint::Min(4),    // Pass Timings & Diagnostics
        ])
        .split(inner_area);

    // 1. Status & Telemetry Header
    let version_str = state
        .status
        .as_ref()
        .map(|s| s.graph_version.to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let total_compilations = state
        .status
        .as_ref()
        .map(|s| s.total_compilations)
        .unwrap_or(0);
    let full_compilations = state
        .status
        .as_ref()
        .map(|s| s.full_compilations)
        .unwrap_or(0);
    let inc_compilations = state
        .status
        .as_ref()
        .map(|s| s.incremental_compilations)
        .unwrap_or(0);

    let last_mode = state
        .status
        .as_ref()
        .and_then(|s| s.last_compilation_mode.clone())
        .unwrap_or_else(|| "none".to_string());

    let last_dur = state
        .status
        .as_ref()
        .and_then(|s| s.last_compilation_duration_ms)
        .unwrap_or(0);

    let (entities_count, facts_count) = if let Some(ref ir) = state.ir_summary {
        (ir.canonical_entities_count, ir.canonical_facts_count)
    } else if let Some(ref r) = state.last_report {
        (r.entities_compiled, r.facts_compiled)
    } else {
        (0, 0)
    };

    let sched_state = state
        .status
        .as_ref()
        .map(|s| s.scheduler_state.clone())
        .unwrap_or_else(|| "idle".to_string());

    let pending_dirty = state
        .status
        .as_ref()
        .map(|s| s.pending_dirty_count)
        .unwrap_or(0);

    let proj_synced = state
        .status
        .as_ref()
        .map(|s| if s.projection_synced { "yes" } else { "no" })
        .unwrap_or("yes");

    let status_lines = vec![
        Line::from(vec![
            Span::styled(
                "Scheduler State: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(sched_state, theme.style(ThemeToken::Success)),
            Span::raw(" | "),
            Span::styled(
                "Pending Dirty Events: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                pending_dirty.to_string(),
                theme.style(ThemeToken::Secondary),
            ),
            Span::raw(" | "),
            Span::styled(
                "Projection Synced: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(proj_synced, theme.style(ThemeToken::Success)),
            Span::raw(" | "),
            Span::styled(
                "Graph Epoch: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(version_str, theme.style(ThemeToken::Info)),
        ]),
        Line::from(vec![
            Span::styled(
                "Last Duration: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{} ms", last_dur), theme.style(ThemeToken::Warning)),
            Span::raw(" | "),
            Span::styled("Last Mode: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(last_mode, theme.style(ThemeToken::Success)),
        ]),
        Line::from(vec![
            Span::styled(
                "Total Compilations: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} (Full: {}, Inc: {})",
                total_compilations, full_compilations, inc_compilations
            )),
        ]),
        Line::from(vec![
            Span::styled(
                "Canonical Knowledge: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Entities: {} | Facts: {}", entities_count, facts_count),
                theme.style(ThemeToken::Success),
            ),
        ]),
    ];

    let header_block = Block::default()
        .title(" Telemetry Overview ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border);
    f.render_widget(Paragraph::new(status_lines).block(header_block), chunks[0]);

    // 2. Diagnostics & Pass Timing Breakdown
    let lower_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Pass Execution Table
    let mut pass_lines = Vec::new();
    if let Some(ref st) = state.status {
        if st.pass_metrics.is_empty() {
            pass_lines.push(Line::from(Span::styled(
                "No pass metrics recorded.",
                theme.style(ThemeToken::TextMuted),
            )));
        } else {
            for pm in &st.pass_metrics {
                pass_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<20}", pm.pass_name),
                        theme.style(ThemeToken::Info),
                    ),
                    Span::raw(format!(
                        " {:>3}x | avg {:>5.2} ms",
                        pm.executions, pm.avg_duration_ms
                    )),
                ]));
            }
        }
    } else {
        pass_lines.push(Line::from(Span::styled(
            "No telemetry available.",
            theme.style(ThemeToken::TextMuted),
        )));
    }

    let pass_block = Block::default()
        .title(" Pass Execution Timings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border);
    f.render_widget(
        Paragraph::new(pass_lines).block(pass_block),
        lower_chunks[0],
    );

    // Diagnostics Browser List
    let mut diag_lines = Vec::new();
    if state.diagnostics.is_empty() {
        diag_lines.push(Line::from(Span::styled(
            "No active compiler diagnostics.",
            theme.style(ThemeToken::Success),
        )));
    } else {
        for (i, diag) in state.diagnostics.iter().enumerate() {
            let selected = i == state.selected_diagnostic_index;
            let level_token = match diag.level.to_lowercase().as_str() {
                "error" => ThemeToken::Danger,
                "warning" => ThemeToken::Warning,
                _ => ThemeToken::Info,
            };

            let prefix = if selected { "> " } else { "  " };
            let style = if selected {
                theme.primary
            } else {
                Style::default()
            };

            diag_lines.push(Line::from(vec![
                Span::styled(prefix, theme.accent),
                Span::styled(
                    format!("[{}] ", diag.level.to_uppercase()),
                    theme.style(level_token),
                ),
                Span::styled(format!("{} - {}", diag.kind, diag.message), style),
            ]));
        }
    }

    let diag_block = Block::default()
        .title(format!(
            " Compiler Diagnostics ({}) ",
            state.diagnostics.len()
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border);
    f.render_widget(
        Paragraph::new(diag_lines).block(diag_block),
        lower_chunks[1],
    );
}
