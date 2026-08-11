//! Bordered evidence card widget displaying provenance, scores, and matched terms.

use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use brain_domain::retrieval::{EvidenceItem, EvidenceReason};
use brain_domain::RetrievalWeight;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Paragraph, Widget};

/// Renders a scannable bordered EvidenceCard representing query provenance.
pub struct EvidenceCard<'a> {
    /// Referenced evidence item domain model.
    pub item: &'a EvidenceItem,
    /// Selection index indicator.
    pub index: usize,
    /// Selection highlight flag.
    pub is_selected: bool,
}

impl<'a> EvidenceCard<'a> {
    /// Renders evidence card into buffer area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if !self.is_selected {
            // Single-line compact chip representation
            let line = ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("🧠 Recalled ", theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD)),
                ratatui::text::Span::styled(format!("#{} ", self.index + 1), theme.style(ThemeToken::TextPrimary)),
                ratatui::text::Span::styled(format!("({}) ", self.item.source), theme.style(ThemeToken::TextMuted)),
                ratatui::text::Span::styled(format!("score {:.2}", self.item.score), theme.style(ThemeToken::TextSecondary)),
            ]);
            let p = Paragraph::new(line);
            p.render(area, buf);
        } else {
            // Expanded full evidence panel representation
            let title = format!(
                " [{}] Evidence — Score {:.2} ",
                self.index + 1,
                self.item.score
            );
            let block = theme.panel(&title, self.is_selected);

            let weight_style = match self.item.weight {
                RetrievalWeight::Critical => {
                    theme.style(ThemeToken::Danger).add_modifier(Modifier::BOLD)
                }
                RetrievalWeight::High => theme
                    .style(ThemeToken::Success)
                    .add_modifier(Modifier::BOLD),
                RetrievalWeight::Normal => theme.style(ThemeToken::TextMuted),
            };

            let mut matched_terms = Vec::new();
            for reason in &self.item.explanation.reasons {
                if let EvidenceReason::KeywordMatch { term } = reason {
                    matched_terms.push(term.as_str());
                }
            }

            let lines = vec![
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("📄 Source: ", theme.style(ThemeToken::TextSecondary)),
                    ratatui::text::Span::styled(
                        format!("{}", self.item.source),
                        theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD),
                    ),
                    ratatui::text::Span::styled("  Weight: ", theme.style(ThemeToken::TextSecondary)),
                    ratatui::text::Span::styled(format!("{:?}", self.item.weight), weight_style),
                ]),
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("   Excerpt: ", theme.style(ThemeToken::TextMuted)),
                    ratatui::text::Span::styled(
                        &self.item.excerpt,
                        theme.style(ThemeToken::TextPrimary),
                    ),
                ]),
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(
                        "   Matched Terms: ",
                        theme.style(ThemeToken::TextSecondary),
                    ),
                    ratatui::text::Span::styled(
                        if matched_terms.is_empty() {
                            "None".to_string()
                        } else {
                            matched_terms.join(" • ")
                        },
                        theme.style(ThemeToken::CodeInline),
                    ),
                ]),
            ];

            let p = Paragraph::new(lines).block(block);
            p.render(area, buf);
        }
    }
}
