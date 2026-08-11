use crate::ui::interaction::markdown::{SelectionState, VisualLine, VisualSpan, VisualStyle};
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

/// Individual visual line element carrying slicing details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleChatLine {
    /// Styled layout line.
    pub line: VisualLine,
    /// Message sender header title.
    pub sender_header: Option<String>,
    /// Indicates if line is part of a user message bubble.
    pub is_user: bool,
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
    let block = ratatui::widgets::Block::default();

    let mut items = Vec::new();
    let lines_to_process = view.visible_lines.iter().skip(view.scroll_offset);
    for (line_idx, visible) in (view.scroll_offset..).zip(lines_to_process) {
        let is_sel = view.selection.is_selected(line_idx);

        if let Some(ref sender) = visible.sender_header {
            let sender_style = if is_sel {
                theme.style(ThemeToken::Selection)
            } else if sender.eq_ignore_ascii_case("You") {
                theme.secondary
            } else if sender.eq_ignore_ascii_case("Claude") || sender.eq_ignore_ascii_case("Brain") {
                theme.style(ThemeToken::HeaderPrimary)
            } else {
                theme.accent.add_modifier(Modifier::BOLD)
            };
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("{}:", sender),
                sender_style,
            )])));
        } else {
            let user_style = if visible.is_user {
                Some(theme.style(ThemeToken::User))
            } else {
                None
            };

            let spans: Vec<Span> = visible
                .line
                .spans
                .iter()
                .map(|span| {
                    let mut s = map_span(span, theme, is_sel);
                    if let Some(style) = user_style {
                        s.style = s.style.patch(style);
                    }
                    s
                })
                .collect();
            let mut line = Line::from(spans);
            if let Some(style) = user_style {
                line = line.style(style);
            }
            items.push(ListItem::new(line));
        }
    }

    let list = List::new(items).block(block);

    f.render_widget(list, area);
}

fn map_span<'a>(span: &VisualSpan, theme: &Theme, is_selected: bool) -> Span<'a> {
    let mut style = match span.style {
        VisualStyle::Heading1 | VisualStyle::Heading2 | VisualStyle::Heading3 => {
            theme.style(ThemeToken::HeaderSecondary)
        }
        VisualStyle::Bold => theme
            .style(ThemeToken::TextPrimary)
            .add_modifier(Modifier::BOLD),
        VisualStyle::Italic => theme
            .style(ThemeToken::TextPrimary)
            .add_modifier(Modifier::ITALIC),
        VisualStyle::InlineCode => theme.style(ThemeToken::CodeInline),
        VisualStyle::CodeKeyword => theme
            .style(ThemeToken::Secondary)
            .add_modifier(Modifier::BOLD),
        VisualStyle::CodeComment => theme.style(ThemeToken::TextMuted),
        VisualStyle::TableHeader => theme.style(ThemeToken::HeaderSecondary),
        VisualStyle::TableCell => theme.style(ThemeToken::TextPrimary),
        VisualStyle::Citation | VisualStyle::EntityReference(_) => theme.style(ThemeToken::Link),
        VisualStyle::Selected => theme.style(ThemeToken::Selection),
        VisualStyle::Normal => theme.style(ThemeToken::TextPrimary),
    };

    if is_selected {
        style = theme.style(ThemeToken::Selection);
    }

    Span::styled(span.text.to_string(), style)
}
