//! Diagnostic TUI widget for visualizing reasoning plan execution DAGs.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use crate::ui::view_models::ReasoningPlanViewModel;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem};
use ratatui::Frame;

/// Renders the diagnostic reasoning plan execution DAG widget.
pub fn render_reasoning_plan(
    f: &mut Frame,
    area: Rect,
    vm: &ReasoningPlanViewModel,
    theme: &Theme,
) {
    let block = Block::default()
        .title(Span::styled(
            format!(" Reasoning Plan Debugger — Query: \"{}\" ", vm.user_query),
            theme
                .style(ThemeToken::HeaderPrimary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.style(ThemeToken::BorderActive));

    let mut lines = Vec::new();
    let max_width = area.width.saturating_sub(4) as usize;

    lines.push(Line::from(vec![
        Span::styled("Plan ID: ", theme.style(ThemeToken::TextMuted)),
        Span::styled(&vm.plan_id, theme.style(ThemeToken::Info)),
        Span::styled(
            format!("  ({} steps total)", vm.total_steps),
            theme.style(ThemeToken::TextMuted),
        ),
    ]));
    lines.push(Line::from("─".repeat(max_width)));

    for (idx, step) in vm.steps.iter().enumerate() {
        let is_last = idx + 1 == vm.steps.len();
        let connector = if is_last { " └─ " } else { " ├─ " };

        lines.push(Line::from(vec![
            Span::styled(connector, theme.style(ThemeToken::Info)),
            Span::styled(
                format!("[{}] ", step.id_badge),
                theme
                    .style(ThemeToken::HeaderSecondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("<{}> ", step.kind_label),
                theme.style(ThemeToken::Warning),
            ),
            Span::styled(&step.description, theme.style(ThemeToken::TextPrimary)),
        ]));

        let mut meta_spans = vec![
            Span::styled("     Complexity: ", theme.style(ThemeToken::TextMuted)),
            Span::styled(step.complexity_badge, theme.style(ThemeToken::Success)),
        ];

        if !step.dependency_badges.is_empty() {
            meta_spans.push(Span::styled(
                "  Depends on: ",
                theme.style(ThemeToken::TextMuted),
            ));
            meta_spans.push(Span::styled(
                step.dependency_badges.join(", "),
                theme.style(ThemeToken::Info),
            ));
        } else {
            meta_spans.push(Span::styled(
                "  (Root Step)",
                theme
                    .style(ThemeToken::TextMuted)
                    .add_modifier(Modifier::ITALIC),
            ));
        }

        lines.push(Line::from(meta_spans));
        if !is_last {
            lines.push(Line::from(vec![Span::styled(
                " │",
                theme.style(ThemeToken::Info),
            )]));
        }
    }

    let items: Vec<ListItem> = lines.into_iter().map(ListItem::new).collect();
    let list = List::new(items).block(block).style(theme.text);

    f.render_widget(list, area);
}
