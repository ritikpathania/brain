use ratatui::layout::{Rect, Layout, Constraint, Direction};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::ui::theme::Theme;
use crate::state::SessionViewModel;
use crate::ui::interaction::sidebar::{SidebarMode, SessionFilter};
use std::sync::OnceLock;

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

fn use_unicode() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .or_else(|_| std::env::var("LC_CTYPE"))
            .map(|s| s.to_uppercase().contains("UTF-8"))
            .unwrap_or(false)
            && std::env::var("ASCII").is_err()
    })
}

/// Formats text by inserting a cursor character at the specified index.
pub fn format_with_cursor(text: &str, cursor_idx: usize, cursor_char: &str) -> String {
    let char_count = text.chars().count();
    let safe_cursor = cursor_idx.min(char_count);
    
    let mut result = String::new();
    for (i, c) in text.chars().enumerate() {
        if i == safe_cursor {
            result.push_str(cursor_char);
        }
        result.push(c);
    }
    if safe_cursor == char_count {
        result.push_str(cursor_char);
    }
    result
}

/// Slices the input string to a sliding window of the specified maximum width centered around the cursor position.
pub fn slice_text_viewport(text: &str, cursor_idx: usize, max_width: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor_idx.min(chars.len());
    
    if chars.len() <= max_width {
        return (text.to_string(), cursor);
    }
    
    let start = if cursor >= max_width {
        cursor - max_width + 1
    } else {
        0
    };
    let end = (start + max_width).min(chars.len());
    let sliced: String = chars[start..end].iter().collect();
    let new_cursor = cursor - start;
    (sliced, new_cursor)
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

    let unicode_mode = use_unicode();

    let list_area = if view.search_active {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner_area);

        let cursor_char = if unicode_mode { "▋" } else { "|" };
        let max_search_width = inner_area.width.saturating_sub(9) as usize;
        let (sliced_query, new_cursor) = slice_text_viewport(view.search_query, view.search_cursor, max_search_width);
        let search_text = format!("Search: {}", format_with_cursor(&sliced_query, new_cursor, cursor_char));
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
                let cursor_char = if unicode_mode { "▋" } else { "|" };
                let max_rename_width = inner_area.width.saturating_sub(6) as usize;
                let (sliced_rename, new_cursor) = slice_text_viewport(view.rename_query, view.rename_cursor, max_rename_width);
                format!("▶ [{}]", format_with_cursor(&sliced_rename, new_cursor, cursor_char))
            } else {
                let pin_prefix = if s.pinned && view.filter == SessionFilter::Active {
                    if unicode_mode { "📌 " } else { "[P] " }
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
