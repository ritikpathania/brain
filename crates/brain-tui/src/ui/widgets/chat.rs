use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;
use crate::ui::theme::Theme;

/// Individual chat message presentation format.
pub struct ChatMessageViewModel {
    /// The sender tag/role display text.
    pub sender: String,
    /// The body text content.
    pub content: String,
}

/// ViewModel carrying Message items and scroll parameters.
pub struct ChatView {
    /// Title of the conversation panel.
    pub title: String,
    /// Ordered list of chat messages for display.
    pub messages: Vec<ChatMessageViewModel>,
    /// Active scroll position offset.
    pub scroll_offset: usize,
}

/// Renders the scrollable message window viewport in the center.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &ChatView, theme: &Theme) {
    let block = Block::default()
        .title(view.title.as_str())
        .borders(Borders::ALL)
        .border_style(theme.border);


    let items: Vec<ListItem> = view.messages.iter().map(|msg| {
        let text = format!("{}: {}", msg.sender, msg.content);
        ListItem::new(text).style(theme.text)
    }).collect();

    let list = List::new(items)
        .block(block);

    f.render_widget(list, area);
}
