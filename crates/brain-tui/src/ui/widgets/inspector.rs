use crate::state::{InspectorLoadState, InspectorSession};
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

/// Stateless drawing function for the interactive Knowledge Inspector widget.
pub fn draw(
    f: &mut Frame<'_>,
    area: Rect,
    active: &InspectorSession,
    theme: &Theme,
    has_focus: bool,
    is_pinned: bool,
) {
    let title_suffix = if has_focus { " (Active Focus)" } else { "" };
    let block = theme.panel(&format!("Knowledge Inspector{}", title_suffix), has_focus);

    let inner_area = block.inner(area);
    let max_width = inner_area.width as usize;

    let mut lines = Vec::new();

    match &active.load_state {
        InspectorLoadState::Loading => {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  ⏳  Loading graph entity details...",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::ITALIC),
            )]));
        }
        InspectorLoadState::Error(err) => {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  ⚠  Error fetching entity:",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![Span::styled(
                format!("     {}", err),
                Style::default().fg(Color::Red),
            )]));
        }
        InspectorLoadState::Loaded(model) => {
            // Title Header
            let mut entity_spans = vec![
                Span::styled("Entity: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &model.entity.label,
                    Style::default()
                        .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            if is_pinned {
                entity_spans.push(Span::styled(
                    " [PINNED]",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(entity_spans));
            lines.push(Line::from(vec![
                Span::styled("Type:   ", Style::default().fg(Color::Gray)),
                Span::styled(&model.entity.node_type, Style::default().fg(Color::Yellow)),
            ]));
            lines.push(Line::from("─".repeat(max_width)));

            // 1. Relationships Section
            lines.push(Line::from(vec![Span::styled(
                "Relationships & Adjacency",
                Style::default()
                    .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                    .add_modifier(Modifier::BOLD),
            )]));
            if model.relationships.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "  (No connections recorded)",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )]));
            } else {
                for (idx, rel) in model.relationships.iter().enumerate() {
                    let is_selected = idx == active.selected_relation_idx;

                    let prefix = if is_selected { " ▶ " } else { "   " };
                    let style = if is_selected {
                        Style::default()
                            .bg(Color::LightBlue)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    let direction_symbol = if rel.direction == "outgoing" {
                        "→"
                    } else {
                        "←"
                    };

                    lines.push(Line::from(vec![
                        Span::styled(
                            prefix,
                            if is_selected {
                                Style::default().fg(Color::LightBlue)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        ),
                        Span::styled(
                            format!("{} [{}] ", direction_symbol, rel.direction),
                            style.fg(Color::Yellow),
                        ),
                        Span::styled(format!("({}) ", rel.relation), style.fg(Color::Magenta)),
                        Span::styled(&rel.target_label, style.fg(Color::White)),
                        Span::styled(format!(" ({})", rel.target_type), style.fg(Color::DarkGray)),
                        Span::styled(format!(" [w: {:.2}]", rel.weight), style.fg(Color::Green)),
                    ]));
                }
            }
            lines.push(Line::from("─".repeat(max_width)));

            // 2. Metadata Section
            lines.push(Line::from(vec![Span::styled(
                "Properties & Metadata",
                Style::default()
                    .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                    .add_modifier(Modifier::BOLD),
            )]));
            // Dump basic properties
            if let serde_json::Value::Object(map) = &model.entity.attributes {
                for (k, v) in map {
                    let val_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}: ", k), Style::default().fg(Color::Gray)),
                        Span::styled(val_str, Style::default().fg(Color::White)),
                    ]));
                }
            }
            // Dump remaining system metadata
            for (k, v) in &model.metadata {
                if k != "id" {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}: ", k), Style::default().fg(Color::DarkGray)),
                        Span::styled(v, Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
            lines.push(Line::from("─".repeat(max_width)));

            // 3. Retrieval Explanation Section (Phase 1 Dedicated Section)
            if let Some(ref explanation) = model.retrieval_explanation {
                lines.push(Line::from(vec![Span::styled(
                    "Retrieval Explanation",
                    Style::default()
                        .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.push(Line::from(vec![
                    Span::styled("  Score: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{:.4}", explanation.score),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        format!(" (raw: {:.4})", explanation.raw_score),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                if explanation.semantic_distance > 0.0 {
                    lines.push(Line::from(vec![
                        Span::styled("  Semantic Distance: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            format!("{:.4}", explanation.semantic_distance),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                }
                if !explanation.keyword_boosts.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  Matched Terms:     ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            explanation.keyword_boosts.join(", "),
                            Style::default().fg(Color::Magenta),
                        ),
                    ]));
                }
                lines.push(Line::from(vec![
                    Span::styled("  Reasoning: ", Style::default().fg(Color::Gray)),
                    Span::styled(&explanation.reasoning, Style::default().fg(Color::White)),
                ]));
                lines.push(Line::from("─".repeat(max_width)));
            }

            // 4. Provenance Section
            lines.push(Line::from(vec![Span::styled(
                "Provenance & Source History",
                Style::default()
                    .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![
                Span::styled("  Origin:   ", Style::default().fg(Color::Gray)),
                Span::styled(&model.provenance.source, Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Location: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &model.provenance.location,
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            if let Some(text_span) = model.provenance.extra_info.get("text_span") {
                lines.push(Line::from(vec![
                    Span::styled("  Text Span: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("\"{}\"", text_span),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
            lines.push(Line::from("─".repeat(max_width)));

            // 5. Recent Activity Section
            lines.push(Line::from(vec![Span::styled(
                "Recent Activity Logs",
                Style::default()
                    .fg(theme.accent.fg.unwrap_or(Color::Cyan))
                    .add_modifier(Modifier::BOLD),
            )]));
            for log in &model.recent_activity {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  [{}] ", log.action),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(&log.details, Style::default().fg(Color::Gray)),
                ]));
            }
        }
    }

    // Convert formatted lines to ListItems, respecting scroll offset
    let mut list_items = Vec::new();
    let scroll_offset = active.scroll_offset.min(lines.len().saturating_sub(1));
    for line in lines.into_iter().skip(scroll_offset) {
        list_items.push(ListItem::new(line));
    }

    let list = List::new(list_items).block(block).style(theme.text);

    f.render_widget(list, area);
}
