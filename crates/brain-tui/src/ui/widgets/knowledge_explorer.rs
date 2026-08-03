use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use crate::ui::widgets::view_models::{
    ConceptDetailsViewModel, ConceptListViewModel, KnowledgeExplorerViewModel, PropertiesViewModel,
    ProvenanceViewModel, RelationsViewModel,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Row, Table};
use ratatui::Frame;

/// Active panel focus inside the Knowledge Explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplorerPanelFocus {
    /// Concept list sidebar.
    #[default]
    ConceptList,
    /// Relationship edges table.
    Relations,
    /// Properties key-value table.
    Properties,
    /// Provenance origin panel.
    Provenance,
}

/// Strongly-typed navigation and interaction intents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerIntent {
    /// Jump navigation into target concept node ID.
    JumpToTarget {
        /// Unique target concept identifier string.
        target_id: String,
    },
    /// Breadcrumb navigation Back ('b').
    NavigateBack,
    /// Breadcrumb navigation Forward ('Shift+B').
    NavigateForward,
    /// Change focused panel layout.
    FocusPanel(ExplorerPanelFocus),
    /// Scroll cursor down / select next.
    SelectNext,
    /// Scroll cursor up / select previous.
    SelectPrevious,
}

/// Stateful aggregate holding Knowledge Explorer session navigation state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnowledgeExplorerState {
    /// Selected concept ID string.
    pub selected_concept_id: Option<String>,
    /// Selected concept index in catalog list.
    pub selected_concept_index: usize,
    /// Active panel focus.
    pub focused_panel: ExplorerPanelFocus,
    /// Selected relationship edge index inside relation list.
    pub selected_relation_index: usize,
    /// History stack of previously visited concept IDs (Back 'b').
    pub history_stack: Vec<String>,
    /// Forward stack of undone concept IDs (Forward 'Shift+B').
    pub forward_stack: Vec<String>,
    /// Error message string if target node is missing/unavailable.
    pub missing_node_error: Option<String>,
}

impl KnowledgeExplorerState {
    /// Creates a new `KnowledgeExplorerState`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Handler executing `ExplorerIntent` transitions on `KnowledgeExplorerState`.
pub struct GraphNavigator;

impl GraphNavigator {
    /// Applies a navigation intent to update explorer session state cleanly.
    pub fn process_intent(state: &mut KnowledgeExplorerState, intent: ExplorerIntent) {
        match intent {
            ExplorerIntent::JumpToTarget { target_id } => {
                state.missing_node_error = None;
                if let Some(current) = state.selected_concept_id.take() {
                    // Push active concept to history stack only if distinct
                    if state.history_stack.last() != Some(&current) {
                        state.history_stack.push(current);
                    }
                }
                state.forward_stack.clear();
                state.selected_concept_id = Some(target_id);
                state.selected_relation_index = 0;
            }
            ExplorerIntent::NavigateBack => {
                state.missing_node_error = None;
                if let Some(previous_id) = state.history_stack.pop() {
                    if let Some(current) = state.selected_concept_id.take() {
                        state.forward_stack.push(current);
                    }
                    state.selected_concept_id = Some(previous_id);
                    state.selected_relation_index = 0;
                }
            }
            ExplorerIntent::NavigateForward => {
                state.missing_node_error = None;
                if let Some(next_id) = state.forward_stack.pop() {
                    if let Some(current) = state.selected_concept_id.take() {
                        state.history_stack.push(current);
                    }
                    state.selected_concept_id = Some(next_id);
                    state.selected_relation_index = 0;
                }
            }
            ExplorerIntent::FocusPanel(panel) => {
                state.focused_panel = panel;
            }
            ExplorerIntent::SelectNext => match state.focused_panel {
                ExplorerPanelFocus::ConceptList => {
                    state.selected_concept_index = state.selected_concept_index.saturating_add(1);
                }
                ExplorerPanelFocus::Relations => {
                    state.selected_relation_index = state.selected_relation_index.saturating_add(1);
                }
                _ => {}
            },
            ExplorerIntent::SelectPrevious => match state.focused_panel {
                ExplorerPanelFocus::ConceptList => {
                    state.selected_concept_index = state.selected_concept_index.saturating_sub(1);
                }
                ExplorerPanelFocus::Relations => {
                    state.selected_relation_index = state.selected_relation_index.saturating_sub(1);
                }
                _ => {}
            },
        }
    }
}

/// Renders the Concept List browser widget.
pub fn draw_concept_list_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &ConceptListViewModel,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" CONCEPTS CATALOG ")
        .border_style(border_style);

    if vm.items.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No concept nodes in graph catalog.",
            theme.style(ThemeToken::TextMuted),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = ["Cursor", "Label", "Type", "ID", "Edges"]
        .iter()
        .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = vm.selected_index == Some(idx);
            let cursor_str = if is_selected { " ▶ " } else { "   " };
            let style = if is_selected {
                theme.border_active.add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cells = vec![
                Span::styled(cursor_str, theme.style(ThemeToken::Warning)),
                Span::styled(&item.label, style),
                Span::styled(
                    &item.node_type,
                    style.patch(theme.style(ThemeToken::Warning)),
                ),
                Span::styled(&item.id, style.patch(theme.style(ThemeToken::TextMuted))),
                Span::styled(
                    &item.relationships_count_text,
                    style.patch(theme.style(ThemeToken::Info)),
                ),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(16),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the Concept Details header widget.
pub fn draw_concept_details_widget(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&ConceptDetailsViewModel>,
    missing_error: Option<&str>,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" CONCEPT DETAILS ")
        .border_style(theme.border);

    if let Some(err) = missing_error {
        let lines = vec![
            Line::from(vec![Span::styled(
                "  ⚠  Target Concept Unavailable",
                theme.style(ThemeToken::Danger).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                format!("     Reason: {}", err),
                theme.style(ThemeToken::Warning),
            )]),
            Line::from(vec![Span::styled(
                "     Press 'b' to navigate back to previous concept.",
                theme.style(ThemeToken::TextMuted),
            )]),
        ];
        let p = Paragraph::new(lines).block(block);
        frame.render_widget(p, area);
        return;
    }

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "Select a concept node to view details.",
                theme.style(ThemeToken::TextMuted),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let lines = vec![
        Line::from(vec![
            Span::raw("Canonical Label: "),
            Span::styled(
                &vm.label,
                theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Node Type:       "),
            Span::styled(&vm.node_type, theme.style(ThemeToken::Warning)),
            Span::raw("  |  ID: "),
            Span::styled(&vm.id, theme.style(ThemeToken::TextMuted)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the Relationship Edges widget table.
pub fn draw_relations_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &RelationsViewModel,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" RELATIONSHIPS & ADJACENCY (Press Enter to Jump Target) ")
        .border_style(border_style);

    if vm.items.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No relationship edges recorded for this concept.",
            theme.style(ThemeToken::TextMuted),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = [
        "Cursor",
        "Direction",
        "Relation",
        "Target Label",
        "Target Type",
        "Weight",
    ]
    .iter()
    .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = vm.selected_index == Some(idx);
            let cursor_str = if is_selected { " ▶ " } else { "   " };
            let style = if is_selected {
                theme.border_active.add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cells = vec![
                Span::styled(cursor_str, theme.style(ThemeToken::Warning)),
                Span::styled(
                    &item.direction,
                    style.patch(theme.style(item.direction_token)),
                ),
                Span::styled(
                    &item.relation,
                    style.patch(theme.style(ThemeToken::Warning)),
                ),
                Span::styled(&item.target_label, style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    &item.target_type,
                    style.patch(theme.style(ThemeToken::TextMuted)),
                ),
                Span::styled(
                    &item.weight_text,
                    style.patch(theme.style(ThemeToken::Secondary)),
                ),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(16),
        Constraint::Percentage(22),
        Constraint::Percentage(30),
        Constraint::Percentage(16),
        Constraint::Percentage(12),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the Properties key-value widget table.
pub fn draw_properties_widget(
    frame: &mut Frame,
    area: Rect,
    vm: &PropertiesViewModel,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" CONCEPT PROPERTIES ")
        .border_style(border_style);

    if vm.items.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No key-value attributes recorded.",
            theme.style(ThemeToken::TextMuted),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = ["Group", "Key", "Value"]
        .iter()
        .map(|h| Span::styled(*h, theme.accent.add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = vm
        .items
        .iter()
        .map(|item| {
            let group_token = match item.group.as_str() {
                "System" => ThemeToken::Danger,
                "Canonical" => ThemeToken::Info,
                "User" => ThemeToken::Success,
                _ => ThemeToken::TextMuted,
            };
            let cells = vec![
                Span::styled(&item.group, theme.style(group_token)),
                Span::styled(&item.key, theme.style(ThemeToken::Warning)),
                Span::styled(&item.value, theme.style(ThemeToken::TextPrimary)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(50),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the Provenance origin metadata widget.
pub fn draw_provenance_widget(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&ProvenanceViewModel>,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" PROVENANCE & ORIGIN HISTORY ")
        .border_style(border_style);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "No provenance origin recorded.",
                theme.style(ThemeToken::TextMuted),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let mut lines = vec![
        Line::from(vec![
            Span::raw("Source: "),
            Span::styled(&vm.source, theme.style(ThemeToken::Success)),
            Span::raw("  |  Compiler Pass: "),
            Span::styled(&vm.compiler_pass, theme.style(ThemeToken::Warning)),
        ]),
        Line::from(vec![
            Span::raw("Location:  "),
            Span::styled(&vm.location, theme.style(ThemeToken::Info)),
            Span::raw("  |  Timestamp: "),
            Span::styled(&vm.timestamp_text, theme.style(ThemeToken::TextMuted)),
        ]),
    ];

    if !vm.extra_info.is_empty() {
        let extra_str = vm
            .extra_info
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("  ");
        lines.push(Line::from(vec![
            Span::raw("Annotations: "),
            Span::styled(extra_str, theme.style(ThemeToken::TextMuted)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the bottom Command Hint Footer bar for Knowledge Explorer.
pub fn draw_explorer_command_hint_footer(frame: &mut Frame, area: Rect, theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            theme.style(ThemeToken::Info).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Scroll Cursor   "),
        Span::styled(
            " Enter ",
            theme
                .style(ThemeToken::Success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Jump Target Node   "),
        Span::styled(
            " b / Shift+B ",
            theme
                .style(ThemeToken::Secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Back/Forward   "),
        Span::styled(
            " Tab ",
            theme
                .style(ThemeToken::Warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Switch Focus Panel   "),
        Span::styled(
            " q ",
            theme
                .style(ThemeToken::TextMuted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Back "),
    ]);

    let paragraph = Paragraph::new(hints).style(Style::default());
    frame.render_widget(paragraph, area);
}

/// Top-level coordinator rendering the complete KnowledgeExplorer screen.
pub fn draw_knowledge_explorer(
    frame: &mut Frame,
    area: Rect,
    vm: &KnowledgeExplorerViewModel,
    state: &KnowledgeExplorerState,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Concept Details Header
            Constraint::Min(8),    // Middle Split (Concept Catalog + Relations/Properties)
            Constraint::Length(5), // Provenance History Footer
            Constraint::Length(1), // Command Hint Footer
        ])
        .split(area);

    // 1. Draw Concept Details Header
    draw_concept_details_widget(
        frame,
        chunks[0],
        vm.details.as_ref(),
        state.missing_node_error.as_deref(),
        theme,
    );

    // 2. Partition Middle Section (Left: Catalog | Right: Relations + Properties)
    let mid_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    draw_concept_list_widget(
        frame,
        mid_chunks[0],
        &vm.concept_list,
        state.focused_panel == ExplorerPanelFocus::ConceptList,
        theme,
    );

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(mid_chunks[1]);

    draw_relations_widget(
        frame,
        right_chunks[0],
        &vm.relations,
        state.focused_panel == ExplorerPanelFocus::Relations,
        theme,
    );
    draw_properties_widget(
        frame,
        right_chunks[1],
        &vm.properties,
        state.focused_panel == ExplorerPanelFocus::Properties,
        theme,
    );

    // 3. Draw Provenance Panel
    draw_provenance_widget(
        frame,
        chunks[2],
        vm.provenance.as_ref(),
        state.focused_panel == ExplorerPanelFocus::Provenance,
        theme,
    );

    // 4. Draw Command Hint Footer
    draw_explorer_command_hint_footer(frame, chunks[3], theme);
}
