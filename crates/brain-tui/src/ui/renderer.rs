use crate::state::{ConnectionMode, FocusRegion, UiState};
use crate::ui::interaction::markdown::{SelectionState, ViewportIndex, VisualLine, VisualLineKind};
use crate::ui::status_footer::StatusFooterWidget;
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use crate::ui::widgets::{
    ambient_status,
    chat::{self, ChatView, VisibleChatLine},
    dialog::Dialog,
    header::{self, HeaderView},
    home_welcome, inspector, pinned_overlay,
    prompt::{self, PromptView},
    sidebar, workspace_dashboard,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;
use std::cell::RefCell;
use std::collections::HashMap;

/// Cache key identifying a compiled LayoutTree geometry block.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutCacheKey {
    /// Target message identifier string.
    pub message_id: String,
    /// Message content revision sequence.
    pub content_revision: u64,
    /// Viewport width constraint.
    pub width: usize,
}

/// Cache key identifying a solved NavigationIndex flat coordinates list.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavigationCacheKey {
    /// Target message identifier string.
    pub message_id: String,
    /// Message content revision sequence.
    pub content_revision: u64,
    /// Viewport width constraint.
    pub width: usize,
    /// ViewState expanded tool sections hash value.
    pub expansion_hash: u64,
}

/// Explicit layout mode taxonomy determining primary screen division and pane constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLayoutMode {
    /// Full-width task-oriented Welcome landing layout (no sidebar).
    Welcome,
    /// Multi-pane conversation workspace layout (with sessions sidebar).
    Workspace,
    /// Graph exploration layout.
    Exploration,
    /// Runtime dashboard layout.
    Dashboard,
    /// Explainability inspection layout.
    Explainability,
    /// Interactive reflection review layout.
    InteractiveReflection,
    /// Knowledge evolution layout.
    KnowledgeEvolution,
    /// Knowledge automation layout.
    KnowledgeAutomation,
}

/// Layout grid organizer dividing the screen cells and assembling widget view models.
pub struct AppRenderer {
    /// LayoutTree geometry cache.
    pub layout_tree_cache:
        RefCell<HashMap<LayoutCacheKey, crate::ui::interaction::layout_tree::LayoutTree>>,
    /// Flat navigation solved index cache.
    pub navigation_cache:
        RefCell<HashMap<NavigationCacheKey, crate::ui::interaction::navigation::NavigationIndex>>,
}

impl AppRenderer {
    /// Creates a new `AppRenderer`.
    pub fn new() -> Self {
        Self {
            layout_tree_cache: RefCell::new(HashMap::new()),
            navigation_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Determines the explicit `AppLayoutMode` from the current UI state.
    pub fn layout_mode(state: &UiState) -> AppLayoutMode {
        match state.mode {
            crate::state::TuiMode::Conversation => {
                if state.screen == crate::ui::navigation::Screen::Home
                    && state.active_messages.is_empty()
                {
                    AppLayoutMode::Welcome
                } else {
                    AppLayoutMode::Workspace
                }
            }
            crate::state::TuiMode::Exploration => AppLayoutMode::Exploration,
            crate::state::TuiMode::RuntimeDashboard => AppLayoutMode::Dashboard,
            crate::state::TuiMode::Explainability => AppLayoutMode::Explainability,
            crate::state::TuiMode::InteractiveReflection => AppLayoutMode::InteractiveReflection,
            crate::state::TuiMode::KnowledgeEvolution => AppLayoutMode::KnowledgeEvolution,
            crate::state::TuiMode::KnowledgeAutomation => AppLayoutMode::KnowledgeAutomation,
        }
    }

    /// Computes constraints and returns partitioned area Rects for widgets.
    pub fn compute_layout(
        &self,
        area: Rect,
        state: &UiState,
    ) -> (Rect, Rect, Rect, Rect, Rect, Rect, Rect) {
        let layout_mode = Self::layout_mode(state);
        let slash_visible = state.slash_completion().visible;
        let (header_h, prompt_h, status_h) = match layout_mode {
            AppLayoutMode::Welcome => {
                let h_h = 0u16; // Header bar removed on Home landing page to eliminate top application chrome
                let p_h = 3u16;
                let s_h = if state.command_palette.open || slash_visible {
                    0u16
                } else {
                    1u16
                }; // Status footer hidden when palette or slash completion is open
                (h_h, p_h, s_h)
            }
            _ => {
                let s_h = if state.command_palette.open || slash_visible {
                    0u16
                } else if area.height >= 10 {
                    1u16
                } else {
                    0u16
                };
                if area.height >= 14 {
                    (2u16, 3u16, s_h)
                } else if area.height >= 10 {
                    (1u16, 3u16, s_h)
                } else {
                    (1u16, 1u16, s_h)
                }
            }
        };

        // Legacy bottom_pad kept for the ≤24 code path and palette_h clamp below.
        let bottom_pad_h = 0u16;

        let palette_h = if state.command_palette.open {
            6u16.min(
                area.height
                    .saturating_sub(header_h + prompt_h + bottom_pad_h + status_h),
            )
        } else if slash_visible {
            8u16.min(
                area.height
                    .saturating_sub(header_h + prompt_h + bottom_pad_h + status_h),
            )
        } else {
            0u16
        };

        // ─── Home prompt anchoring ────────────────────────────────────────────────
        // Choose mid constraint + filler strategy based on mode and terminal height.
        let (mid_constraint, filler_constraint, layout_bottom_pad) =
            if matches!(layout_mode, AppLayoutMode::Welcome) && area.height > 24 {
                // Tall Welcome: anchor prompt at ≈67% of screen height.
                let occupied = header_h + prompt_h + palette_h + status_h;
                let max_mid = area.height.saturating_sub(occupied.max(1));
                let anchor = (area.height as u32 * 67 / 100) as u16;
                let mid_h = anchor
                    .min(max_mid)
                    .max(11u16.min(max_mid));
                let filler_h = area
                    .height
                    .saturating_sub(header_h + mid_h + prompt_h + palette_h + status_h);
                (
                    Constraint::Length(mid_h),
                    Constraint::Length(filler_h),
                    0u16,
                )
            } else {
                // Short Welcome (≤24) and all Workspace/other modes: current behaviour.
                (Constraint::Min(1), Constraint::Length(0), bottom_pad_h)
            };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_h),
                mid_constraint,
                Constraint::Length(prompt_h),
                Constraint::Length(layout_bottom_pad),
                Constraint::Length(palette_h),
                filler_constraint,
                Constraint::Length(status_h),
            ])
            .split(area);

        let mid_area = chunks[1];
        let c = area.width;
        let layout_mode = Self::layout_mode(state);
        let (sb_w, chat_w, insp_w) = match layout_mode {
            AppLayoutMode::Welcome => (0u16, c, 0u16),
            AppLayoutMode::Workspace => {
                if state.screen == crate::ui::navigation::Screen::Home
                    || state.screen == crate::ui::navigation::Screen::Workspace
                    || state.focus != crate::state::FocusRegion::Sidebar
                {
                    (0u16, c, 0u16)
                } else if c >= 120 {
                    let sb = (c * 20 / 100).clamp(22, 28);
                    (sb, c.saturating_sub(sb), 0u16)
                } else if c > 70 {
                    (22u16, c.saturating_sub(22), 0u16)
                } else {
                    (0u16, c, 0u16)
                }
            }
            AppLayoutMode::Exploration => {
                if c >= 105 {
                    (20u16, 50u16, c.saturating_sub(70))
                } else if c >= 85 {
                    (0u16, c.saturating_sub(35), 35u16)
                } else {
                    (0u16, 0u16, c)
                }
            }
            AppLayoutMode::Dashboard
            | AppLayoutMode::Explainability
            | AppLayoutMode::InteractiveReflection
            | AppLayoutMode::KnowledgeEvolution
            | AppLayoutMode::KnowledgeAutomation => (0u16, c, 0u16),
        };

        let mid_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(sb_w),
                Constraint::Length(chat_w),
                Constraint::Length(insp_w),
            ])
            .split(mid_area);

        (
            chunks[0],
            mid_chunks[0],
            mid_chunks[1],
            mid_chunks[2],
            chunks[2],
            chunks[4],
            chunks[6],
        )
    }

    /// Derives lightweight ViewModels from state and draws all TUI components.
    pub fn draw(&self, f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
        // Terminal capabilities are observed once per frame here and threaded
        // explicitly into widgets. Widgets must not read environment variables directly.
        let caps = crate::ui::render::RenderCapabilities::detect();
        let policy = crate::ui::render::CapabilityPolicy::default();
        let capabilities = crate::ui::render::CapabilityResolver::resolve(&caps, &policy);

        let (
            header_area,
            sidebar_area,
            chat_area,
            inspector_area,
            prompt_area,
            palette_area,
            footer_area,
        ) = self.compute_layout(area, state);

        if state.mode == crate::state::TuiMode::RuntimeDashboard {
            let default_report = brain_integrations::dto::v1::RuntimeDiagnosticsReport {
                snapshot_sequence: 0,
                snapshot_timestamp_ms: 0,
                health: "healthy".to_string(),
                health_reason: None,
                orchestrator: brain_integrations::dto::v1::OrchestratorStatsDto {
                    pending_tasks_count: 0,
                    tasks_queued: 0,
                    tasks_completed: 0,
                    tasks_failed: 0,
                    tasks_dropped: 0,
                    last_task_wait_ms: 0,
                    last_task_exec_ms: 0,
                    current_running_task: None,
                    task_history: Vec::new(),
                },
                projection_lags: Vec::new(),
                reflection: brain_integrations::dto::v1::ReflectionStatusReport {
                    background_enabled: true,
                    interval_secs: 300,
                    min_events_trigger: 10,
                    max_nodes_per_cycle: 100,
                    cycle_time_budget_ms: 5000,
                    reflections_executed: 0,
                    reflection_findings_count: 0,
                    reflection_commands_executed: 0,
                    reflection_commands_skipped: 0,
                    last_reflection_duration_ms: None,
                },
            };
            let report_ref = state.diagnostics_report.as_ref().unwrap_or(&default_report);
            let vm = crate::ui::widgets::view_models::RuntimeDashboardViewModel::from_report(
                report_ref,
                Some(state.runtime_dashboard_state.selected_history_index),
            );
            crate::ui::widgets::draw_runtime_dashboard(f, area, &vm, theme);
            return;
        }

        if state.mode == crate::state::TuiMode::Exploration {
            let vm = crate::ui::widgets::view_models::KnowledgeExplorerViewModel::from_report(
                &state.explorer_concepts,
                state.explorer_concept_detail.as_ref(),
                Some(state.knowledge_explorer_state.selected_concept_index),
                Some(state.knowledge_explorer_state.selected_relation_index),
            );
            crate::ui::widgets::draw_knowledge_explorer(
                f,
                area,
                &vm,
                &state.knowledge_explorer_state,
                theme,
            );
            return;
        }

        if state.mode == crate::state::TuiMode::Explainability {
            let vm = crate::ui::widgets::view_models::ExplanationViewModel::from_report(
                state.explanation_report.as_ref(),
                Some(state.explainability_state.selected_step_index),
            );
            crate::ui::widgets::draw_explainability_screen(
                f,
                area,
                &vm,
                &state.explainability_state,
                theme,
            );
            return;
        }

        if state.mode == crate::state::TuiMode::InteractiveReflection {
            let vm =
                crate::ui::widgets::view_models::InteractiveReflectionViewModel::from_proposals(
                    &state.reflection_proposals,
                    Some(state.interactive_reflection_state.selected_proposal_index),
                    state.interactive_reflection_state.filter_status,
                );
            crate::ui::widgets::draw_interactive_reflection_screen(
                f,
                area,
                &vm,
                &state.interactive_reflection_state,
                theme,
            );
            return;
        }

        if state.mode == crate::state::TuiMode::KnowledgeEvolution {
            let vm = crate::ui::widgets::view_models::KnowledgeEvolutionViewModel::from_data(
                &state.evolution_policies,
                Some(state.knowledge_evolution_state.selected_policy_index),
                state.active_evolution_plan.as_ref(),
                state.evolution_simulation_report.as_ref(),
                &state.evolution_audit_history,
            );
            crate::ui::widgets::draw_knowledge_evolution_screen(
                f,
                area,
                &vm,
                &state.knowledge_evolution_state,
                theme,
            );
            return;
        }

        if state.mode == crate::state::TuiMode::KnowledgeAutomation {
            let vm = crate::ui::widgets::view_models::KnowledgeAutomationViewModel::from_data(
                &state.automation_rules,
                Some(state.knowledge_automation_state.selected_rule_index),
                &state.automation_queue,
                &state.automation_execution_logs,
            );
            crate::ui::widgets::draw_knowledge_automation_screen(
                f,
                area,
                &vm,
                &state.knowledge_automation_state,
                theme,
            );
            return;
        }

        // 1. Build Header ViewModel
        let connection_status = match state.connection_mode {
            ConnectionMode::Daemon => "● Connected".to_string(),
            ConnectionMode::Embedded => "● Embedded".to_string(),
            ConnectionMode::Disconnected => "○ Offline".to_string(),
            ConnectionMode::Connecting => "◐ Connecting...".to_string(),
        };
        let connection_color_ok = matches!(
            state.connection_mode,
            ConnectionMode::Daemon | ConnectionMode::Embedded
        );
        let header_view = HeaderView {
            title: "BRAIN".to_string(),
            connection_status,
            connection_color_ok,
            enable_reflection_logs: state.enable_reflection_logs,
            pins_count: state.pinned_nodes.len(),
        };

        // 2. Draw Sidebar if visible
        if sidebar_area.width > 0 {
            let visible = state.visible_sessions();
            let selected_pos = state
                .sidebar
                .browse
                .selected
                .and_then(|sel_id| visible.iter().position(|s| s.id == sel_id));
            let sidebar_view = sidebar::SidebarView {
                sessions: &visible,
                selected_idx: selected_pos,
                has_focus: state.focus == FocusRegion::Sidebar,
                filter: state.sidebar.browse.filter,
                mode: state.sidebar.mode,
                search_active: state.sidebar.search.active,
                search_query: state.sidebar.search.editor.buffer(),
                search_cursor: state.sidebar.search.editor.cursor().visual_col as usize,
                rename_query: state.sidebar.rename.editor.buffer(),
                rename_cursor: state.sidebar.rename.editor.cursor().visual_col as usize,
            };
            sidebar::draw(f, sidebar_area, &sidebar_view, theme, caps.unicode);
        }

        // 3. Build Chat View with Virtualization & Cache checks if visible
        if chat_area.width > 0 {
            let chat_width = chat_area.width.saturating_sub(2) as usize;
            let blocks = state.build_timeline_blocks(chat_width);

            let mut heights = Vec::with_capacity(blocks.len());
            for block in &blocks {
                let h = if block.header.is_some() {
                    1 + block.visual_lines.len() + 1
                } else {
                    block.visual_lines.len() + 1
                };
                heights.push(h);
            }
            let index = ViewportIndex::rebuild(&heights);

            let start_offset = state.viewport.scroll_offset as u32;
            let viewport_height = chat_area.height.saturating_sub(2) as u32;

            let mut visible_lines = Vec::new();
            if let Some((mut block_idx, mut local_line)) = index.find_offset(start_offset) {
                let mut lines_collected = 0u32;
                while block_idx < blocks.len() && lines_collected < viewport_height {
                    let block = &blocks[block_idx];
                    let block_height = heights[block_idx] as u32;

                    while local_line < block_height && lines_collected < viewport_height {
                        if let Some(ref sender) = block.header {
                            if local_line == 0 {
                                visible_lines.push(VisibleChatLine {
                                    line: VisualLine {
                                        kind: VisualLineKind::Text,
                                        spans: vec![],
                                    },
                                    sender_header: Some(sender.clone()),
                                    is_user: block.is_user,
                                });
                            } else if local_line <= block.visual_lines.len() as u32 {
                                let line_idx = (local_line - 1) as usize;
                                visible_lines.push(VisibleChatLine {
                                    line: block.visual_lines[line_idx].clone(),
                                    sender_header: None,
                                    is_user: block.is_user,
                                });
                            } else {
                                visible_lines.push(VisibleChatLine {
                                    line: VisualLine {
                                        kind: VisualLineKind::Text,
                                        spans: vec![],
                                    },
                                    sender_header: None,
                                    is_user: block.is_user,
                                });
                            }
                        } else {
                            if local_line < block.visual_lines.len() as u32 {
                                visible_lines.push(VisibleChatLine {
                                    line: block.visual_lines[local_line as usize].clone(),
                                    sender_header: None,
                                    is_user: block.is_user,
                                });
                            } else {
                                visible_lines.push(VisibleChatLine {
                                    line: VisualLine {
                                        kind: VisualLineKind::Text,
                                        spans: vec![],
                                    },
                                    sender_header: None,
                                    is_user: block.is_user,
                                });
                            }
                        }
                        local_line += 1;
                        lines_collected += 1;
                    }
                    block_idx += 1;
                    local_line = 0;
                }
            }

            let available_w = (chat_area.width as usize).saturating_sub(25);
            let display_session_title =
                if state.session_title.chars().count() > available_w && available_w > 5 {
                    let truncated: String = state
                        .session_title
                        .chars()
                        .take(available_w.saturating_sub(3))
                        .collect();
                    format!("{}...", truncated)
                } else {
                    state.session_title.clone()
                };

            let chat_title = if state.pinned_nodes.is_empty() {
                format!(" Knowledge — {} ", display_session_title)
            } else {
                format!(
                    " Knowledge — {} [Context: {} pinned] ",
                    display_session_title,
                    state.pinned_nodes.len()
                )
            };

            if state.screen == crate::ui::navigation::Screen::Home
                && state.active_messages.is_empty()
            {
                let surface_rect = Rect::new(
                    chat_area.x.saturating_add(1),
                    2,
                    chat_area.width.saturating_sub(2),
                    9,
                );
                home_welcome::draw(f, surface_rect, state, theme);

                let status_y = prompt_area.y.saturating_sub(1);
                let status_rect = Rect::new(area.x, status_y, area.width, 1);
                ambient_status::draw(f, status_rect, state, theme);
            } else if state.screen == crate::ui::navigation::Screen::Workspace {
                workspace_dashboard::draw(f, chat_area, state, theme);
            } else {
                let welcome_h = (9usize).saturating_sub(state.viewport.scroll_offset) as u16;
                let chat_msg_area = if welcome_h > 0 && welcome_h < chat_area.height {
                    let surface_rect = Rect::new(
                        chat_area.x.saturating_add(1),
                        chat_area.y,
                        chat_area.width.saturating_sub(2),
                        welcome_h,
                    );
                    home_welcome::draw(f, surface_rect, state, theme);
                    Rect::new(
                        chat_area.x,
                        chat_area.y.saturating_add(welcome_h),
                        chat_area.width,
                        chat_area.height.saturating_sub(welcome_h),
                    )
                } else {
                    chat_area
                };

                let chat_view = ChatView {
                    title: chat_title,
                    visible_lines,
                    scroll_offset: state.viewport.scroll_offset.saturating_sub(9),
                    selection: SelectionState::new(),
                };
                chat::draw(f, chat_msg_area, &chat_view, theme);
            }
        }

        // 3b. Draw Inspector if visible
        if inspector_area.width > 0 {
            if let Some(ref active) = state.active_inspector {
                let is_pinned = state
                    .pinned_nodes
                    .iter()
                    .any(|pn| pn.node_id == active.node_id);
                inspector::draw(
                    f,
                    inspector_area,
                    active,
                    theme,
                    state.focus == FocusRegion::Inspector,
                    is_pinned,
                );
            }
        }

        // 4. Build Prompt ViewModel
        let is_welcome_mode = Self::layout_mode(state) == AppLayoutMode::Welcome;
        let prompt_view = PromptView {
            prompt_text: state.editor.text(),
            cursor_position: state.editor.cursor(),
            has_focus: state.focus == FocusRegion::Editor,
            submit_with_workspace: state.submit_with_workspace,
            is_welcome: is_welcome_mode,
        };

        // 6. Draw TUI widgets
        if header_area.height > 0 {
            header::draw(f, header_area, &header_view, theme);
        }
        prompt::draw(f, prompt_area, &prompt_view, theme);

        if state.command_palette.open && palette_area.height > 0 {
            let registry = crate::ui::command::DynamicCommandRegistry::new();
            let index = crate::ui::command::CommandIndex::build(&registry);
            let provider = crate::ui::command::CommandProvider::new(&index);
            let palette_widget = crate::ui::command::PaletteWidget {
                state: &state.command_palette,
                provider: &provider,
            };
            palette_widget.render(palette_area, f.buffer_mut(), theme);
        }

        if footer_area.height > 0 {
            StatusFooterWidget::draw(f, footer_area, state, theme);
        }

        // 7. Draw tool approval dialog overlay (Command Palette and Slash Completion are
        //    DEFERRED — they have no keyboard entry points wired in lib.rs yet and are
        //    intentionally excluded from the render path to avoid confusing dead UI.)
        if !state.pending_approvals.is_empty() {
            let first = &state.pending_approvals[0];
            let title = format!(" Tool Approval: {} ", first.tool_id.0);
            let message = format!("Arguments: {}", first.arguments);
            let buttons = [
                crate::ui::widgets::view_models::DialogButton {
                    label: "Yes (y)",
                    kind: crate::ui::widgets::view_models::ButtonKind::Primary,
                    enabled: true,
                },
                crate::ui::widgets::view_models::DialogButton {
                    label: "No (n)",
                    kind: crate::ui::widgets::view_models::ButtonKind::Secondary,
                    enabled: true,
                },
            ];
            let view = crate::ui::widgets::view_models::DialogView {
                title: &title,
                message: &message,
                buttons: &buttons,
                selected_index: 0,
            };
            let dialog = Dialog { view: &view };

            let dialog_width = 60;
            let dialog_height = 8;
            let dialog_area = Rect::new(
                area.x + (area.width.saturating_sub(dialog_width) / 2),
                area.y + (area.height.saturating_sub(dialog_height) / 2),
                dialog_width.min(area.width),
                dialog_height.min(area.height),
            );

            let buf = f.buffer_mut();
            let block =
                ratatui::widgets::Block::default().style(theme.style(ThemeToken::Background));
            ratatui::widgets::Widget::render(block, dialog_area, buf);

            // Use the capabilities already built at the top of draw() — not a second detect().
            let icons = crate::ui::render::IconSet::new(
                capabilities.nerd_fonts != crate::ui::render::NerdFontsSupport::None,
            );
            let ctx = crate::ui::render::RenderContext {
                theme,
                icons: &icons,
                capabilities,
                tick: 0,
            };

            crate::ui::widgets::brain_widget::BrainWidget::render(&dialog, dialog_area, buf, &ctx);
        }

        if let Some(ref modal) = state.help_overlay {
            let width = 64;
            let height = (modal.lines.len() as u16 + 4).min(area.height);
            let modal_area = Rect::new(
                area.x + (area.width.saturating_sub(width) / 2),
                area.y + (area.height.saturating_sub(height) / 2),
                width.min(area.width),
                height,
            );

            f.render_widget(ratatui::widgets::Clear, modal_area);
            let bg_style = theme.style(ThemeToken::Background);
            let block = theme.panel(&modal.title, true).style(bg_style);

            let items: Vec<ratatui::widgets::ListItem> = modal
                .lines
                .iter()
                .map(|line| {
                    ratatui::widgets::ListItem::new(line.as_str())
                        .style(theme.style(ThemeToken::TextPrimary))
                })
                .collect();
            let list = ratatui::widgets::List::new(items).block(block);
            f.render_widget(list, modal_area);
        }

        // 8. Draw Pinned Context overlay if active
        if state.overlay == crate::state::TuiOverlay::PinnedContext {
            pinned_overlay::draw(
                f,
                area,
                &state.pinned_nodes,
                state.pinned_overlay_cursor,
                theme,
            );
        }

        // 9. Draw Slash Completion popup overlay if visible
        let slash_state = state.slash_completion();
        if slash_state.visible {
            let popup_area = palette_area;
            crate::ui::widgets::completion::draw(f, popup_area, slash_state, theme);
        }
    }
}

impl Default for AppRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{PresentationModel, VisibleRow};

    #[test]
    fn test_presentation_model_rendering() {
        let state = UiState::default();
        let model: PresentationModel = state.presentation_model(10, 5);
        assert_eq!(model.visible_rows.len(), 5);
        assert_eq!(model.scroll_indicator, "Showing 1-5 of 10");
        let first_row: Option<&VisibleRow> = model.visible_rows.first();
        assert_eq!(first_row.map(|r| r.index), Some(0));
    }

    #[test]
    fn test_layout_invariant_assertions_matrix() {
        let renderer = AppRenderer::new();
        let state = UiState::default();

        let benchmarks = [(60, 20), (80, 24), (100, 30), (160, 50)];

        for (w, h) in benchmarks {
            let area = Rect::new(0, 0, w, h);
            let (header, sidebar, chat, inspector, prompt, _palette, status) =
                renderer.compute_layout(area, &state);

            // Invariant 1: No widget exceeds terminal height or width bounds
            assert!(
                header.y + header.height <= area.height,
                "Header exceeds height at {}x{}",
                w,
                h
            );
            assert!(
                prompt.y + prompt.height <= area.height,
                "Prompt exceeds height at {}x{}",
                w,
                h
            );
            assert!(
                status.y + status.height <= area.height,
                "Status exceeds height at {}x{}",
                w,
                h
            );

            // Invariant 2: Prompt is always visible (height > 0)
            assert!(prompt.height > 0, "Prompt must be visible at {}x{}", w, h);

            // Invariant 3: Total height does not exceed terminal height
            let total_h = header.height
                + chat.height.max(sidebar.height).max(inspector.height)
                + prompt.height
                + status.height;
            assert!(
                total_h <= area.height,
                "Total layout height {} exceeds area height {} at {}x{}",
                total_h,
                area.height,
                w,
                h
            );
        }
    }

    #[test]
    fn test_home_container_height_clamp() {
        // Verifies proportional-capped box_h formula at every benchmarked terminal size.
        // Formula: (mid_h * 60 / 100).clamp(11, 18)
        //   • floor  11 → 80×24 and 96×24 unchanged
        //   • growth    → 120×30 produces box_h=13 (proportional, not floored)
        //   • ceiling 18 → 182×53 capped, never turns Home into a dashboard
        let renderer = AppRenderer::new();
        let state = UiState::default();

        let cases: &[(u16, u16, u16)] = &[
            (80, 24, 12),  // mid_h=20 → 12
            (96, 24, 12),  // mid_h=20 → 12
            (120, 30, 12), // proportional growth: mid_h=20 (67% anchor) → (20*60/100)=12
            (160, 50, 18), // ceiling cap: mid_h=33 (67% anchor) → (33*60/100)=19 → capped at 18
            (182, 53, 18), // ceiling cap: mid_h=35 (67% anchor) → (35*60/100)=21 → capped at 18
        ];

        for &(w, h, expected_box_h) in cases {
            let area = Rect::new(0, 0, w, h);
            let (_, _, chat, _, _, _, _) = renderer.compute_layout(area, &state);
            let mid_h = chat.height;
            let actual = (mid_h * 60 / 100).clamp(11, 18).min(mid_h);
            assert_eq!(
                actual, expected_box_h,
                "box_h mismatch at {}×{}: mid_h={}, formula gives {}, expected {}",
                w, h, mid_h, actual, expected_box_h
            );
            // Inner height must always fit right-panel content (5+1+3 = 9 rows)
            assert!(
                actual.saturating_sub(2) >= 9,
                "box inner height {} at {}×{} is too small for right-panel content (need ≥9)",
                actual.saturating_sub(2),
                w,
                h
            );
        }
    }

    #[test]
    fn test_welcome_prompt_anchored_on_tall_terminals() {
        let renderer = AppRenderer::new();
        let state = UiState::default(); // Welcome mode

        // (width, height, expected_prompt_row)
        let cases: &[(u16, u16, u16)] = &[
            (80, 24, 20),  // 80x24: prompt at y=20
            (96, 24, 20),  // 96x24: prompt at y=20
            (120, 30, 20), // 30 * 67/100 = 20
            (156, 52, 34), // 52 * 67/100 = 34
            (182, 53, 35), // 53 * 67/100 = 35
        ];

        for &(w, h, expected_row) in cases {
            let area = Rect::new(0, 0, w, h);
            let (_, _, _, _, prompt, _, _) = renderer.compute_layout(area, &state);

            assert_eq!(
                prompt.y, expected_row,
                "prompt.y mismatch at {}×{}: got {}, expected {}",
                w, h, prompt.y, expected_row
            );

            // Tall terminals must stay in the 60–70% visual band.
            if h > 24 {
                let pct = prompt.y as u32 * 100 / h as u32;
                assert!(
                    (60..=70).contains(&pct),
                    "prompt at {}% (row {}/{}) outside 60–70% band at {}×{}",
                    pct,
                    prompt.y,
                    h,
                    w,
                    h
                );
            }
        }
    }
}
