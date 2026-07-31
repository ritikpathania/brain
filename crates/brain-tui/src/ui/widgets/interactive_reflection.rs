use crate::ui::theme::Theme;
use crate::ui::widgets::screen_state::ScreenState;
use crate::ui::widgets::view_models::{
    InteractiveReflectionViewModel, ReflectionProposalDetailViewModel,
};
use brain_integrations::dto::v1::ReflectionProposalStatus;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

/// Lightweight proposal command dispatch state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProposalDispatchState {
    /// Idle, awaiting user interaction.
    #[default]
    Idle,
    /// Command dispatched to application service, processing.
    Dispatching,
    /// Proposal resolved successfully.
    Completed,
    /// Synchronizing read-model projections.
    ProjectionRefresh,
}

/// Active panel focus inside the Interactive Reflection screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractiveReflectionPanelFocus {
    /// Proposal review list table.
    #[default]
    ProposalList,
    /// Focused proposal breakdown pane.
    ProposalDetail,
    /// Modal confirmation dialog overlay.
    ConfirmationModal,
}

/// Strongly-typed interaction intents for Interactive Reflection screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveReflectionIntent {
    /// Select next proposal down.
    SelectNext,
    /// Select previous proposal up.
    SelectPrev,
    /// Trigger accept resolution on selected proposal.
    AcceptSelected,
    /// Trigger reject resolution on selected proposal.
    RejectSelected,
    /// Trigger defer resolution on selected proposal.
    DeferSelected,
    /// Filter proposals by status (`None` for All).
    FilterStatus(Option<ReflectionProposalStatus>),
    /// Focus specific panel.
    FocusPanel(InteractiveReflectionPanelFocus),
}

/// Stateful container holding Interactive Reflection session state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InteractiveReflectionState {
    /// Selected proposal index in list.
    pub selected_proposal_index: usize,
    /// Active panel focus.
    pub focused_panel: InteractiveReflectionPanelFocus,
    /// Command dispatch state machine.
    pub dispatch_state: ProposalDispatchState,
    /// Active proposal status filter.
    pub filter_status: Option<ReflectionProposalStatus>,
    /// Pending action confirmation modal state (Action name, Proposal ID).
    pub pending_confirmation: Option<(String, String)>,
}

impl InteractiveReflectionState {
    /// Creates a new `InteractiveReflectionState`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScreenState for InteractiveReflectionState {
    fn selected_index(&self) -> usize {
        self.selected_proposal_index
    }

    fn reset(&mut self) {
        self.selected_proposal_index = 0;
        self.focused_panel = InteractiveReflectionPanelFocus::ProposalList;
        self.dispatch_state = ProposalDispatchState::Idle;
        self.filter_status = None;
        self.pending_confirmation = None;
    }
}

/// Handler executing `InteractiveReflectionIntent` state transitions.
pub struct ReflectionProposalNavigator;

impl ReflectionProposalNavigator {
    /// Processes an `InteractiveReflectionIntent` to mutate session state cleanly.
    pub fn process_intent(
        state: &mut InteractiveReflectionState,
        intent: InteractiveReflectionIntent,
    ) {
        match intent {
            InteractiveReflectionIntent::SelectNext => {
                state.selected_proposal_index = state.selected_proposal_index.saturating_add(1);
            }
            InteractiveReflectionIntent::SelectPrev => {
                state.selected_proposal_index = state.selected_proposal_index.saturating_sub(1);
            }
            InteractiveReflectionIntent::AcceptSelected => {
                state.focused_panel = InteractiveReflectionPanelFocus::ConfirmationModal;
            }
            InteractiveReflectionIntent::RejectSelected => {
                state.focused_panel = InteractiveReflectionPanelFocus::ConfirmationModal;
            }
            InteractiveReflectionIntent::DeferSelected => {
                state.focused_panel = InteractiveReflectionPanelFocus::ConfirmationModal;
            }
            InteractiveReflectionIntent::FilterStatus(filter) => {
                state.filter_status = filter;
                state.selected_proposal_index = 0;
            }
            InteractiveReflectionIntent::FocusPanel(panel) => {
                state.focused_panel = panel;
            }
        }
    }
}

/// Renders the proposals review list table.
pub fn draw_reflection_proposals_list(
    frame: &mut Frame,
    area: Rect,
    vm: &InteractiveReflectionViewModel,
    has_focus: bool,
    theme: &Theme,
) {
    let border_style = if has_focus {
        theme.border_active
    } else {
        theme.border
    };

    let title = format!(
        " REFLECTION PROPOSALS [Pending: {} | Accepted: {} | Rejected: {} | Deferred: {}] ",
        vm.pending_count, vm.accepted_count, vm.rejected_count, vm.deferred_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(border_style);

    if vm.items.is_empty() {
        let p = Paragraph::new(Span::styled(
            "No reflection proposals match current filter.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header_cells = [
        "Cursor",
        "Status",
        "Action",
        "Confidence",
        "Source Concept",
        "Target Concept",
        "Explanation Summary",
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
                Span::styled(cursor_str, Style::default().fg(Color::Yellow)),
                Span::styled(&item.status_badge, style.fg(item.status_color)),
                Span::styled(&item.action_badge, style.fg(item.action_color)),
                Span::styled(&item.confidence_text, style.fg(Color::Yellow)),
                Span::styled(&item.source_concept_id, style.fg(Color::Cyan)),
                Span::styled(&item.target_concept_id_text, style.fg(Color::Gray)),
                Span::styled(
                    &item.explanation_summary,
                    style.add_modifier(Modifier::BOLD),
                ),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(12),
        Constraint::Percentage(14),
        Constraint::Percentage(10),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
        Constraint::Percentage(24),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Renders the focused proposal detail breakdown pane.
pub fn draw_reflection_proposal_detail(
    frame: &mut Frame,
    area: Rect,
    vm: Option<&ReflectionProposalDetailViewModel>,
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
        .title(" PROPOSAL EXECUTION BREAKDOWN ")
        .border_style(border_style);

    let vm = match vm {
        Some(v) => v,
        None => {
            let p = Paragraph::new(Span::styled(
                "Select a proposal to view evidence breakdown.",
                Style::default().fg(Color::DarkGray),
            ))
            .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let lines = vec![
        Line::from(vec![
            Span::raw("Proposal ID:  "),
            Span::styled(
                &vm.proposal_id,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Finding Kind: "),
            Span::styled(&vm.finding_kind, Style::default().fg(Color::Yellow)),
            Span::raw("  |  Confidence: "),
            Span::styled(
                &vm.confidence_text,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Action Type:  "),
            Span::styled(
                &vm.action_type_text,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  Status: "),
            Span::styled(&vm.status_text, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("Source Node:  "),
            Span::styled(&vm.source_concept_id, Style::default().fg(Color::Cyan)),
            Span::raw("  |  Target Node: "),
            Span::styled(&vm.target_concept_id_text, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::raw("Explanation:  "),
            Span::styled(&vm.explanation_summary, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("Created At:   "),
            Span::styled(&vm.created_at_text, Style::default().fg(Color::DarkGray)),
            Span::raw("  |  Resolved At: "),
            Span::styled(&vm.resolved_at_text, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Renders the action confirmation modal dialog overlay.
pub fn draw_reflection_confirmation_modal(
    frame: &mut Frame,
    area: Rect,
    action_name: &str,
    proposal_id: &str,
    theme: &Theme,
) {
    let popup_area = centered_rect(60, 25, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(
            " CONFIRM PROPOSAL ACTION: {} ",
            action_name.to_uppercase()
        ))
        .border_style(theme.border_active);

    let text = vec![
        Line::from(vec![
            Span::raw("Are you sure you want to "),
            Span::styled(
                action_name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" proposal '"),
            Span::styled(proposal_id, Style::default().fg(Color::Cyan)),
            Span::raw("'?"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [Enter] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Confirm & Dispatch Command     "),
            Span::styled(
                " [Esc] ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, popup_area);
}

/// Renders the Command Hint Footer bar for Interactive Reflection screen.
pub fn draw_reflection_command_hint_footer(frame: &mut Frame, area: Rect, _theme: &Theme) {
    let hints = Line::from(vec![
        Span::styled(
            " ↑↓ / jk ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Select Proposal   "),
        Span::styled(
            " a ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Accept   "),
        Span::styled(
            " r ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Reject   "),
        Span::styled(
            " d ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Defer   "),
        Span::styled(
            " f ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Filter Status   "),
        Span::styled(
            " q / Esc ",
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Exit "),
    ]);

    let paragraph = Paragraph::new(hints).style(Style::default());
    frame.render_widget(paragraph, area);
}

/// Top-level layout coordinator for Interactive Reflection screen.
pub fn draw_interactive_reflection_screen(
    frame: &mut Frame,
    area: Rect,
    vm: &InteractiveReflectionViewModel,
    state: &InteractiveReflectionState,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Proposal List
            Constraint::Min(7),         // Detail Breakdown Pane
            Constraint::Length(1),      // Footer Hints
        ])
        .split(area);

    // 1. Draw Proposal List
    draw_reflection_proposals_list(
        frame,
        chunks[0],
        vm,
        state.focused_panel == InteractiveReflectionPanelFocus::ProposalList,
        theme,
    );

    // 2. Draw Proposal Detail
    draw_reflection_proposal_detail(
        frame,
        chunks[1],
        vm.detail_pane.as_ref(),
        state.focused_panel == InteractiveReflectionPanelFocus::ProposalDetail,
        theme,
    );

    // 3. Draw Command Hint Footer
    draw_reflection_command_hint_footer(frame, chunks[2], theme);

    // 4. Draw Modal Overlay if active
    if state.focused_panel == InteractiveReflectionPanelFocus::ConfirmationModal {
        if let Some((action, prop_id)) = &state.pending_confirmation {
            draw_reflection_confirmation_modal(frame, area, action, prop_id, theme);
        }
    }
}

/// Helper calculating centered popup rectangle bounds.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
