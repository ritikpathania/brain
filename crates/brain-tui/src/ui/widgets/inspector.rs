use crate::state::{InspectorLoadState, InspectorSession};
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
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
                theme.style(ThemeToken::Info).add_modifier(Modifier::ITALIC),
            )]));
        }
        InspectorLoadState::Error(err) => {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  ⚠  Error fetching entity:",
                theme.style(ThemeToken::Danger).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![Span::styled(
                format!("     {}", err),
                theme.style(ThemeToken::Danger),
            )]));
        }
        InspectorLoadState::Loaded(model) => {
            let vm = crate::ui::view_models::InspectorViewModel::from_domain(model);

            // Title Header
            let mut entity_spans = vec![
                Span::styled("Entity: ", theme.style(ThemeToken::TextMuted)),
                Span::styled(
                    vm.display_name.clone(),
                    theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
                ),
            ];
            if is_pinned {
                entity_spans.push(Span::styled(
                    " [PINNED]",
                    theme
                        .style(ThemeToken::Secondary)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(entity_spans));
            lines.push(Line::from("─".repeat(max_width)));

            for section in &vm.sections {
                match section {
                    crate::ui::view_models::EntitySection::Identity {
                        id,
                        display_name: _,
                        node_type,
                    } => {
                        lines.push(Line::from(vec![Span::styled(
                            "Identity",
                            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(vec![
                            Span::styled("  ID:   ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(id.clone(), theme.style(ThemeToken::TextPrimary)),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled("  Type: ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(node_type.clone(), theme.style(ThemeToken::Warning)),
                        ]));
                        lines.push(Line::from("─".repeat(max_width)));
                    }
                    crate::ui::view_models::EntitySection::Source {
                        kind,
                        producer,
                        location,
                        timestamp,
                        workspace,
                    } => {
                        lines.push(Line::from(vec![Span::styled(
                            section.heading(),
                            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(vec![
                            Span::styled("  Kind:      ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(kind.clone(), theme.style(ThemeToken::TextPrimary)),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled("  Producer:  ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(producer.clone(), theme.style(ThemeToken::Secondary)),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled("  Location:  ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(location.clone(), theme.style(ThemeToken::Warning)),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled("  Workspace: ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(workspace.clone(), theme.style(ThemeToken::TextMuted)),
                        ]));
                        if *timestamp > 0 {
                            lines.push(Line::from(vec![
                                Span::styled("  Time:      ", theme.style(ThemeToken::TextMuted)),
                                Span::styled(
                                    timestamp.to_string(),
                                    theme.style(ThemeToken::TextMuted),
                                ),
                            ]));
                        }
                        lines.push(Line::from("─".repeat(max_width)));
                    }
                    crate::ui::view_models::EntitySection::RetrievalExplanation { explanation } => {
                        lines.push(Line::from(vec![Span::styled(
                            section.heading(),
                            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(vec![Span::styled(
                            "  Matched:",
                            theme.style(ThemeToken::TextMuted),
                        )]));
                        for reason in &explanation.matched_elements {
                            lines.push(Line::from(vec![
                                Span::styled("    ✓ ", theme.style(ThemeToken::Success)),
                                Span::styled(reason.label(), theme.style(ThemeToken::TextPrimary)),
                            ]));
                        }
                        lines.push(Line::from(vec![
                            Span::styled("  Confidence: ", theme.style(ThemeToken::TextMuted)),
                            Span::styled(
                                explanation.confidence.badge_text(),
                                theme
                                    .style(ThemeToken::Success)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                        lines.push(Line::from("─".repeat(max_width)));
                    }
                    crate::ui::view_models::EntitySection::ActivityFeed { entries } => {
                        lines.push(Line::from(vec![Span::styled(
                            section.heading(),
                            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
                        )]));
                        for entry in entries {
                            lines.push(Line::from(vec![
                                Span::styled("  • ", theme.style(ThemeToken::Success)),
                                Span::styled(
                                    entry.action.clone(),
                                    theme
                                        .style(ThemeToken::HeaderPrimary)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(": "),
                                Span::styled(
                                    entry.details.clone(),
                                    theme.style(ThemeToken::TextPrimary),
                                ),
                            ]));
                        }
                        lines.push(Line::from("─".repeat(max_width)));
                    }
                    crate::ui::view_models::EntitySection::Metadata { attributes } => {
                        lines.push(Line::from(vec![Span::styled(
                            section.heading(),
                            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
                        )]));
                        if attributes.is_empty() {
                            lines.push(Line::from(vec![Span::styled(
                                "  (No attributes recorded)",
                                theme
                                    .style(ThemeToken::TextMuted)
                                    .add_modifier(Modifier::ITALIC),
                            )]));
                        } else {
                            for (k, v) in attributes {
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        format!("  {}: ", k),
                                        theme.style(ThemeToken::TextMuted),
                                    ),
                                    Span::styled(v.clone(), theme.style(ThemeToken::TextPrimary)),
                                ]));
                            }
                        }
                        lines.push(Line::from("─".repeat(max_width)));
                    }
                    crate::ui::view_models::EntitySection::Relationships { connections } => {
                        render_relationship_section(
                            &mut lines,
                            connections,
                            active.selected_relation_idx,
                            theme,
                            max_width,
                        );
                    }
                }
            }
            if let Some(text_span) = model.provenance.extra_info.get("text_span") {
                lines.push(Line::from(vec![
                    Span::styled("  Text Span: ", theme.style(ThemeToken::TextMuted)),
                    Span::styled(
                        format!("\"{}\"", text_span),
                        theme
                            .style(ThemeToken::TextMuted)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
            lines.push(Line::from("─".repeat(max_width)));

            // 5. Recent Activity Section
            lines.push(Line::from(vec![Span::styled(
                "Recent Activity Logs",
                theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
            )]));
            for log in &model.recent_activity {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  [{}] ", log.action),
                        theme.style(ThemeToken::Warning),
                    ),
                    Span::styled(log.details.clone(), theme.style(ThemeToken::TextMuted)),
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

/// Renders the relationship adjacency section with localized selection highlights.
fn render_relationship_section<'a>(
    lines: &mut Vec<Line<'a>>,
    connections: &[crate::ui::view_models::RelationshipViewModel],
    selected_idx: usize,
    theme: &Theme,
    max_width: usize,
) {
    lines.push(Line::from(vec![Span::styled(
        "Relationships & Adjacency",
        theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
    )]));
    if connections.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (No connections recorded)",
            theme
                .style(ThemeToken::TextMuted)
                .add_modifier(Modifier::ITALIC),
        )]));
    } else {
        for (idx, rel) in connections.iter().enumerate() {
            let is_selected = selected_idx == idx;
            let prefix = if is_selected { " ▶ " } else { "   " };
            let style = if is_selected {
                theme.style(ThemeToken::Selection)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    prefix.to_string(),
                    if is_selected {
                        theme.style(ThemeToken::Info)
                    } else {
                        theme.style(ThemeToken::TextMuted)
                    },
                ),
                Span::styled(
                    format!("({}) ", rel.relation_kind),
                    style.patch(theme.style(ThemeToken::Secondary)),
                ),
                Span::styled(
                    rel.target_label.clone(),
                    style.patch(theme.style(ThemeToken::TextPrimary)),
                ),
            ]));
        }
    }
    lines.push(Line::from("─".repeat(max_width)));
}
