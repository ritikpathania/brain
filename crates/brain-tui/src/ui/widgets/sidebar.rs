use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;
use crate::ui::theme::Theme;
use crate::state::SessionViewModel;

/// ViewModel carrying session list and selection index.
pub struct SidebarView<'a> {
    /// List of sessions metadata to display.
    pub sessions: &'a [SessionViewModel],
    /// Highlight index of the selected row.
    pub selected_idx: usize,
    /// Whether the sidebar widget currently has input focus.
    pub has_focus: bool,
}

/// Renders the sidebar session browser panel.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &SidebarView<'_>, theme: &Theme) {
    let border_style = if view.has_focus {
        theme.border_active
    } else {
        theme.border
    };

    let block = Block::default()
        .title(" Conversations ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let items: Vec<ListItem> = view.sessions
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            let is_selected = idx == view.selected_idx;
            let prefix = if s.active { "● " } else { "  " };
            let text = format!("{}{}", prefix, s.title);

            let style = if is_selected {
                if view.has_focus {
                    theme.primary.add_modifier(ratatui::style::Modifier::REVERSED)
                } else {
                    theme.inactive.add_modifier(ratatui::style::Modifier::REVERSED)
                }
            } else {
                theme.text
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(block);

    f.render_widget(list, area);
}
