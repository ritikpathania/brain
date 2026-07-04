use ratatui::layout::{Rect, Layout, Constraint, Direction};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::ui::theme::Theme;
use crate::state::SessionViewModel;
use crate::ui::interaction::sidebar::{SidebarMode, SessionFilter};

/// ViewModel carrying session list, selection, filters, and editor states.
pub struct SidebarView<'a> {
    /// List of sessions metadata to display.
    pub sessions: &'a [SessionViewModel],
    /// Highlight index of the selected row.
    pub selected_idx: Option<usize>,
    /// Whether the sidebar widget currently has input focus.
    pub has_focus: bool,
    /// Active filter mode (Active or Archived).
    pub filter: SessionFilter,
    /// Sidebar interaction mode (Browse or Rename).
    pub mode: SidebarMode,
    /// Search active state.
    pub search_active: bool,
    /// Search editor text content.
    pub search_query: &'a str,
    /// Visual cursor position in characters for search input.
    pub search_cursor: usize,
    /// The inline rename input query string.
    pub rename_query: &'a str,
    /// Visual cursor position in characters for rename input.
    pub rename_cursor: usize,
}

/// Renders the sidebar session browser panel.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &SidebarView<'_>, theme: &Theme) {
    let border_style = if view.has_focus {
        theme.border_active
    } else {
        theme.border
    };

    let title = match view.filter {
        SessionFilter::Active => " Sessions (Active) ",
        SessionFilter::Archived => " Sessions (Archived) ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner_area = block.inner(area);

    // Draw the block frame
    f.render_widget(block, area);

    if inner_area.width == 0 || inner_area.height == 0 {
        return;
    }

    let use_unicode = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_CTYPE"))
        .map(|s| s.to_uppercase().contains("UTF-8"))
        .unwrap_or(false)
        && std::env::var("ASCII").is_err();

    let list_area = if view.search_active {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner_area);

        let cursor_char = if use_unicode { "▋" } else { "|" };
        let search_text = format!("Search: {}{}", view.search_query, cursor_char);
        let search_p = Paragraph::new(search_text).style(theme.text);
        f.render_widget(search_p, chunks[0]);

        chunks[1]
    } else {
        inner_area
    };

    let items: Vec<ListItem> = view.sessions
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            let is_selected = Some(idx) == view.selected_idx;

            let text = if is_selected && view.mode == SidebarMode::Rename {
                let cursor_char = if use_unicode { "▋" } else { "|" };
                format!("▶ [{}{}]", view.rename_query, cursor_char)
            } else {
                let pin_prefix = if s.pinned && view.filter == SessionFilter::Active {
                    if use_unicode { "📌 " } else { "[P] " }
                } else {
                    ""
                };
                let prefix = if s.active { "● " } else { "  " };
                format!("{}{}{}", prefix, pin_prefix, s.title)
            };

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

    let list = List::new(items);
    f.render_widget(list, list_area);
}
