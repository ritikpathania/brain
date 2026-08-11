//! Floating borderless overlay widget for the Claude-grade Command Palette.

use crate::ui::command::palette::CommandPaletteState;
use crate::ui::command::provider::PaletteProvider;
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Clear, Paragraph, Widget};

/// Renders floating borderless Command Palette overlay with muted category headers.
pub struct PaletteWidget<'a> {
    /// Active state of the command palette overlay.
    pub state: &'a CommandPaletteState,
    /// Provider to query for category-grouped sections.
    pub provider: &'a dyn PaletteProvider,
}

impl<'a> PaletteWidget<'a> {
    /// Renders floating borderless palette overlay into buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if !self.state.open {
            return;
        }

        let (sections, flat_items) = self.state.query_provider(self.provider);

        let max_lines = (area.height as usize).min(12);
        if max_lines == 0 {
            return;
        }

        let mut lines = Vec::new();
        let selected_idx = self
            .state
            .selected_index
            .min(flat_items.len().saturating_sub(1));
        let mut current_item_idx = 0;
        let show_category_titles = sections.len() > 1;

        for sec in &sections {
            if lines.len() >= max_lines {
                break;
            }

            if show_category_titles && !sec.title.is_empty() {
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("  ", theme.style(ThemeToken::TextMuted)),
                    ratatui::text::Span::styled(
                        sec.title,
                        theme
                            .style(ThemeToken::TextSecondary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            for item in &sec.items {
                if lines.len() >= max_lines {
                    break;
                }

                let is_selected = current_item_idx == selected_idx;
                let (cmd_style, desc_style) = if is_selected {
                    (
                        theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD),
                        theme
                            .style(ThemeToken::TextPrimary)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    (
                        theme.style(ThemeToken::TextPrimary),
                        theme.style(ThemeToken::TextMuted),
                    )
                };

                let name_prefix = item.name;

                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("  ", theme.style(ThemeToken::TextMuted)),
                    ratatui::text::Span::styled(format!("{:<20}", name_prefix), cmd_style),
                    ratatui::text::Span::styled(item.description, desc_style),
                ]));

                current_item_idx += 1;
            }
        }

        if flat_items.is_empty() {
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "  No matching items found",
                theme.style(ThemeToken::TextMuted),
            )));
        }

        let render_height = (lines.len() as u16).min(area.height);
        let render_area = Rect::new(area.x, area.y, area.width, render_height);

        Clear.render(render_area, buf);
        let p = Paragraph::new(lines);
        p.render(render_area, buf);
    }
}
