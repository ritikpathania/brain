//! Widget rendering memory stewardship results collections.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use crate::ui::view_models::MemoryResultsViewModel;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

/// Renders the memory stewardship results list widget.
pub fn render_memory_list(
    frame: &mut Frame,
    area: Rect,
    vm: &MemoryResultsViewModel,
    theme: &Theme,
) {
    if !vm.is_active() || area.width < 10 || area.height < 3 {
        return;
    }

    let block = Block::default()
        .title(" Memory Stewardship (/memory list) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.style(ThemeToken::BorderActive));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if vm.items().is_empty() {
        let empty_p = Paragraph::new(vec![Line::from(vec![Span::styled(
            "  No memory records found in stewardship store.",
            theme
                .style(ThemeToken::TextMuted)
                .add_modifier(Modifier::ITALIC),
        )])]);
        frame.render_widget(empty_p, inner);
        return;
    }

    let mut lines = Vec::new();
    let selected_idx = vm.selected_index().unwrap_or(0);

    let categories = [
        ("[Pinned Context]", "Pinned Context"),
        ("[Runtime Context]", "Active Runtime Context"),
        ("[Consolidated Memory]", "Consolidated Memories"),
    ];

    for (cat_badge, cat_title) in &categories {
        let matching_items: Vec<(usize, &crate::ui::view_models::MemoryItemViewModel)> = vm
            .items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.category_badge == *cat_badge)
            .collect();

        if matching_items.is_empty() {
            continue;
        }

        lines.push(Line::from(vec![Span::styled(
            format!("── {} ──", cat_title),
            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
        )]));

        for (idx, item) in matching_items {
            let is_selected = idx == selected_idx;
            let prefix = if is_selected { "▶ " } else { "  " };

            let title_style = if is_selected {
                theme
                    .style(ThemeToken::HeaderPrimary)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.style(ThemeToken::TextPrimary)
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, theme.style(ThemeToken::Success)),
                Span::styled(&item.display_name, title_style),
                Span::raw(" "),
                Span::styled(&item.state_badge, theme.style(ThemeToken::Warning)),
            ]));

            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(&item.snippet, theme.style(ThemeToken::TextMuted)),
                Span::styled(
                    format!(" [{}]", item.source_kind),
                    theme
                        .style(ThemeToken::TextMuted)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));

            lines.push(Line::from(""));
        }
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}
