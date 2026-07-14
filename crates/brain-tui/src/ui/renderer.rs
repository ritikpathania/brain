use std::cell::RefCell;
use std::collections::HashMap;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;
use crate::state::{UiState, ConnectionMode, FocusRegion};
use crate::ui::theme::Theme;
use crate::ui::widgets::{
    header::{self, HeaderView},
    chat::{self, ChatView, VisibleChatLine},
    prompt::{self, PromptView},
    status::{self, StatusView},
    sidebar,
    dialog::Dialog,
    inspector,
    pinned_overlay,
};
use crate::ui::interaction::markdown::{
    SelectionState, ViewportIndex, VisualLine, VisualLineKind
};



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

/// Layout grid organizer dividing the screen cells and assembling widget view models.
pub struct AppRenderer {
    /// LayoutTree geometry cache.
    pub layout_tree_cache: RefCell<HashMap<LayoutCacheKey, crate::ui::interaction::layout_tree::LayoutTree>>,
    /// Flat navigation solved index cache.
    pub navigation_cache: RefCell<HashMap<NavigationCacheKey, crate::ui::interaction::navigation::NavigationIndex>>,
}

impl AppRenderer {
    /// Creates a new `AppRenderer`.
    pub fn new() -> Self {
        Self {
            layout_tree_cache: RefCell::new(HashMap::new()),
            navigation_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Computes constraints and returns partitioned area Rects for widgets.
    pub fn compute_layout(&self, area: Rect, state: &UiState) -> (Rect, Rect, Rect, Rect, Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Logo / Header
                Constraint::Min(10),  // Mid section (Sidebar + Chat + Inspector)
                Constraint::Length(3), // Prompt input editor
                Constraint::Length(1), // Footer status bar
            ])
            .split(area);

        let mid_area = chunks[1];
        // Use the actual rendered width (area.width) rather than state.terminal_width so
        // that compute_layout is always correct even when state hasn't been updated yet.
        let c = area.width;
        let (sb_w, chat_w, insp_w) = match state.mode {
            crate::state::TuiMode::Conversation => {
                if c > 70 {
                    (25u16, c.saturating_sub(25), 0u16)
                } else {
                    (0u16, c, 0u16)
                }
            }
            crate::state::TuiMode::Exploration => {
                if c >= 105 {
                    (20u16, 50u16, c.saturating_sub(70))
                } else if c >= 85 {
                    (0u16, c.saturating_sub(35), 35u16)
                } else {
                    (0u16, 0u16, c)
                }
            }
        };

        let mid_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(sb_w),
                Constraint::Length(chat_w),
                Constraint::Length(insp_w),
            ])
            .split(mid_area);

        (chunks[0], mid_chunks[0], mid_chunks[1], mid_chunks[2], chunks[2], chunks[3])
    }

    /// Derives lightweight ViewModels from state and draws all TUI components.
    pub fn draw(&self, f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
        let (header_area, sidebar_area, chat_area, inspector_area, prompt_area, status_area) = self.compute_layout(area, state);

        // 1. Build Header ViewModel
        let connection_status = match state.connection_mode {
            ConnectionMode::Daemon => "[Connected: Daemon]".to_string(),
            ConnectionMode::Embedded => "[Connected: In-Process]".to_string(),
            ConnectionMode::Disconnected => "[Disconnected]".to_string(),
            ConnectionMode::Connecting => "[Connecting...]".to_string(),
        };
        let connection_color_ok = matches!(
            state.connection_mode,
            ConnectionMode::Daemon | ConnectionMode::Embedded
        );
        let header_view = HeaderView {
            title: "BRAIN v2 Engine".to_string(),
            connection_status,
            connection_color_ok,
            enable_reflection_logs: state.enable_reflection_logs,
            pins_count: state.pinned_nodes.len(),
        };

        // 2. Draw Sidebar if visible
        if sidebar_area.width > 0 {
            let visible = state.visible_sessions();
            let selected_pos = state.sidebar.browse.selected
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
            sidebar::draw(f, sidebar_area, &sidebar_view, theme);
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
                                    line: VisualLine { kind: VisualLineKind::Text, spans: vec![] },
                                    sender_header: Some(sender.clone()),
                                });
                            } else if local_line <= block.visual_lines.len() as u32 {
                                let line_idx = (local_line - 1) as usize;
                                visible_lines.push(VisibleChatLine {
                                    line: block.visual_lines[line_idx].clone(),
                                    sender_header: None,
                                });
                            } else {
                                visible_lines.push(VisibleChatLine {
                                    line: VisualLine { kind: VisualLineKind::Text, spans: vec![] },
                                    sender_header: None,
                                });
                            }
                        } else {
                            if local_line < block.visual_lines.len() as u32 {
                                visible_lines.push(VisibleChatLine {
                                    line: block.visual_lines[local_line as usize].clone(),
                                    sender_header: None,
                                });
                            } else {
                                visible_lines.push(VisibleChatLine {
                                    line: VisualLine { kind: VisualLineKind::Text, spans: vec![] },
                                    sender_header: None,
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

            let title = if state.session_load_state == crate::state::SessionLoadState::Loading {
                format!(" Conversation (Loading...) - {} ", state.session_title)
            } else {
                format!(" Conversation - {} ", state.session_title)
            };

            let chat_view = ChatView {
                title,
                visible_lines,
                scroll_offset: state.viewport.scroll_offset,
                selection: SelectionState::new(),
            };
            chat::draw(f, chat_area, &chat_view, theme);
        }

        // 3b. Draw Inspector if visible
        if inspector_area.width > 0 {
            if let Some(ref active) = state.active_inspector {
                let is_pinned = state.pinned_nodes.iter().any(|pn| pn.node_id == active.node_id);
                inspector::draw(f, inspector_area, active, theme, state.focus == FocusRegion::Inspector, is_pinned);
            }
        }

        // 4. Build Prompt ViewModel
        let prompt_view = PromptView {
            prompt_text: state.editor.text(),
            cursor_position: state.editor.cursor(),
            has_focus: state.focus == FocusRegion::Editor,
            submit_with_workspace: state.submit_with_workspace,
        };

        // 5. Build Status ViewModel — message derives from runtime state so users
        //    always see what the app is doing rather than static keyboard hints.
        let status_message = {
            if let Some((ref msg, _)) = state.transient_message {
                format!(" 📌 {} ", msg)
            } else if state.connection_mode == crate::state::ConnectionMode::Disconnected {
                " ⚠  Connection lost — press Enter to retry".to_string()
            } else if state.connection_mode == crate::state::ConnectionMode::Connecting
                && !matches!(state.generation_state, crate::state::GenerationState::Starting | crate::state::GenerationState::Streaming { .. })
            {
                " ⚡ Connecting to daemon...".to_string()
            } else if state.mode == crate::state::TuiMode::Exploration {
                let p_hint = if state.active_inspector.as_ref().map(|ai| state.pinned_nodes.iter().any(|pn| pn.node_id == ai.node_id)).unwrap_or(false) {
                    "p: Unpin"
                } else {
                    "p: Pin"
                };
                format!(" Backspace: Back  |  Tab: Focus  |  Esc: Close  |  ↑/↓: Nav Relations  |  Enter: Inspect  |  Ctrl+P: Context  |  {}", p_hint)
            } else {
                let context_hint = if !state.pinned_nodes.is_empty() {
                    format!("  |  Ctrl+P: Context ({})", state.pinned_nodes.len())
                } else {
                    "".to_string()
                };
                match &state.generation_state {
                    crate::state::GenerationState::Starting => " 🔍 Searching...".to_string(),
                    crate::state::GenerationState::Streaming { .. } => " 📥 Receiving results...".to_string(),
                    crate::state::GenerationState::Finished => format!(" ✓  Done  |  Tab: Switch Focus  |  Esc: New query{}  |  Ctrl+C: Quit", context_hint),
                    crate::state::GenerationState::Cancelled(_) => " ✕  Cancelled  |  Tab: Switch Focus  |  Enter: Submit".to_string(),
                    crate::state::GenerationState::Error(msg) => format!(" ⚠  Error: {}  |  Enter: Retry", msg.chars().take(60).collect::<String>()),
                    crate::state::GenerationState::Idle => format!(" Tab: Switch Focus  |  Esc: Quit  |  Ctrl+C: Cancel{}  |  Enter: Submit", context_hint),
                }
            }
        };
        let status_view = StatusView {
            message: status_message,
        };

        // 6. Draw TUI widgets
        header::draw(f, header_area, &header_view, theme);
        prompt::draw(f, prompt_area, &prompt_view, theme);
        status::draw(f, status_area, &status_view, theme);

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
            let block = ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));
            ratatui::widgets::Widget::render(block, dialog_area, buf);

            let caps = crate::ui::render::RenderCapabilities::detect();
            let policy = crate::ui::render::CapabilityPolicy::default();
            let capabilities = crate::ui::render::CapabilityResolver::resolve(&caps, &policy);
            let icons = crate::ui::render::IconSet::new(capabilities.nerd_fonts != crate::ui::render::NerdFontsSupport::None);
            let ctx = crate::ui::render::RenderContext {
                theme,
                icons: &icons,
                capabilities,
                tick: 0,
            };

            crate::ui::widgets::brain_widget::BrainWidget::render(&dialog, dialog_area, buf, &ctx);
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
    }

}

impl Default for AppRenderer {
    fn default() -> Self {
        Self::new()
    }
}



