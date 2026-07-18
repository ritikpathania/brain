use crate::ui::interaction::markdown::{SelectionState, VisualLine, VisualSpan, VisualStyle};
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

/// Individual visual line element carrying slicing details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleChatLine {
    /// Styled layout line.
    pub line: VisualLine,
    /// Message sender header title.
    pub sender_header: Option<String>,
}

/// ViewModel carrying Message items and scroll parameters.
pub struct ChatView {
    /// Title of the conversation panel.
    pub title: String,
    /// Ordered list of virtualized visible chat lines for display.
    pub visible_lines: Vec<VisibleChatLine>,
    /// Active scroll position offset.
    pub scroll_offset: usize,
    /// Selection highlight state.
    pub selection: SelectionState,
}

/// Renders the scrollable message window viewport in the center.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &ChatView, theme: &Theme) {
    let block = Block::default()
        .title(view.title.as_str())
        .borders(Borders::ALL)
        .border_style(theme.border);

    let mut items = Vec::new();
    for (line_idx, visible) in (view.scroll_offset..).zip(&view.visible_lines) {
        let is_sel = view.selection.is_selected(line_idx);

        if let Some(ref sender) = visible.sender_header {
            let sender_style = if is_sel {
                Style::default().bg(Color::LightBlue).fg(Color::Black)
            } else {
                theme.accent.add_modifier(Modifier::BOLD)
            };
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("{}:", sender),
                sender_style,
            )])));
        } else {
            let spans: Vec<Span> = visible
                .line
                .spans
                .iter()
                .map(|span| map_span(span, theme, is_sel))
                .collect();
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    let list = List::new(items).block(block);

    f.render_widget(list, area);
}

fn map_span<'a>(span: &VisualSpan, theme: &Theme, is_selected: bool) -> Span<'a> {
    let mut style = Style::default();

    match span.style {
        VisualStyle::Heading1 => {
            style = style
                .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                .add_modifier(Modifier::BOLD);
        }
        VisualStyle::Heading2 => {
            style = style
                .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                .add_modifier(Modifier::BOLD);
        }
        VisualStyle::Heading3 => {
            style = style
                .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                .add_modifier(Modifier::BOLD);
        }
        VisualStyle::Bold => {
            style = style.add_modifier(Modifier::BOLD);
        }
        VisualStyle::Italic => {
            style = style.add_modifier(Modifier::ITALIC);
        }
        VisualStyle::InlineCode => {
            style = style.fg(Color::Yellow).bg(Color::DarkGray);
        }
        VisualStyle::CodeKeyword => {
            style = style.fg(Color::Magenta).add_modifier(Modifier::BOLD);
        }
        VisualStyle::CodeComment => {
            style = style.fg(Color::Gray);
        }
        VisualStyle::TableHeader => {
            style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
        }
        VisualStyle::TableCell => {
            style = style.fg(Color::White);
        }
        VisualStyle::Citation => {
            style = style.fg(Color::Blue).add_modifier(Modifier::UNDERLINED);
        }
        VisualStyle::Selected => {
            style = style.bg(Color::LightBlue).fg(Color::Black);
        }
        VisualStyle::EntityReference(_) => {
            style = style
                .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                .add_modifier(Modifier::UNDERLINED);
        }
        VisualStyle::Normal => {
            style = theme.text;
        }
    }

    if is_selected {
        style = style.bg(Color::LightBlue).fg(Color::Black);
    }

    Span::styled(span.text.to_string(), style)
}
