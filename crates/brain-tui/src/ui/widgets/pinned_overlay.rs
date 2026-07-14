//! Modal context overlay listing pinned nodes.

use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::style::{Stylize};
use crate::ui::theme::Theme;
use crate::state::PinnedNode;

/// Renders the Pinned Context list as a modal overlay centered in the area.
pub fn draw(
    f: &mut ratatui::Frame<'_>,
    area: Rect,
    pinned_nodes: &[PinnedNode],
    selected_idx: usize,
    theme: &Theme,
) {
    let popup_width = 80.min(area.width);
    let popup_height = 16.min(area.height);
    
    let popup_area = Rect::new(
        area.x + (area.width.saturating_sub(popup_width) / 2),
        area.y + (area.height.saturating_sub(popup_height) / 2),
        popup_width,
        popup_height,
    );

    // 1. Clear the screen background underneath the overlay
    f.render_widget(Clear, popup_area);

    // 2. Draw border container
    let overlay_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_active)
        .title(" 📌 Pinned Context ")
        .title_alignment(Alignment::Center);

    let inner_area = overlay_block.inner(popup_area);
    f.render_widget(overlay_block, popup_area);

    if inner_area.height == 0 || inner_area.width == 0 {
        return;
    }

    // 3. Split inner area for list and keyboard layout guide
    let list_height = inner_area.height.saturating_sub(2);
    let list_area = Rect::new(inner_area.x, inner_area.y, inner_area.width, list_height);
    let footer_area = Rect::new(
        inner_area.x,
        inner_area.y + list_height,
        inner_area.width,
        inner_area.height.saturating_sub(list_height),
    );

    // 4. Render content or empty message
    if pinned_nodes.is_empty() {
        let empty_msg = Paragraph::new(vec![
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                "No nodes pinned in the working context yet.",
                theme.inactive
            )),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                "To pin a node, inspect it (Enter) from a link, then press 'p'.",
                theme.inactive
            )),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty_msg, list_area);
    } else {
        let items: Vec<ListItem> = pinned_nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                let is_selected = idx == selected_idx;
                
                let prefix = if is_selected {
                    " 👉 "
                } else {
                    "    "
                };

                let node_kind_str = format!("[{}]", node.node_type);
                let line = ratatui::text::Line::from(vec![
                    ratatui::text::Span::raw(prefix),
                    ratatui::text::Span::styled(format!("{:<15}", node_kind_str), theme.accent),
                    ratatui::text::Span::raw(" "),
                    ratatui::text::Span::styled(node.label.clone(), theme.primary),
                ]);

                let mut item = ListItem::new(line);
                if is_selected {
                    item = item.style(theme.primary.bold().bg(ratatui::style::Color::DarkGray));
                } else {
                    item = item.style(theme.text);
                }
                item
            })
            .collect();

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(selected_idx));

        let list_widget = List::new(items);
        f.render_stateful_widget(list_widget, list_area, &mut list_state);
    }

    // 5. Render footer controls guide
    let helper_text = " Esc: Close  |  Enter: Inspect  |  x: Unpin  |  c: Clear All ";
    let helper_p = Paragraph::new(ratatui::text::Line::from(
        ratatui::text::Span::styled(helper_text, theme.status)
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP).border_style(theme.border));

    f.render_widget(helper_p, footer_area);
}
