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
};
use crate::ui::interaction::markdown::{
    SelectionState, CachedMessageLayout, MessageRevision, ViewportIndex,
    MarkdownParser, MarkdownLayout, KeywordSyntaxHighlighter,
    VisualLine, VisualLineKind, VisualSpan, VisualStyle
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
    layout_cache: RefCell<HashMap<String, CachedMessageLayout>>,
    /// LayoutTree geometry cache.
    pub layout_tree_cache: RefCell<HashMap<LayoutCacheKey, crate::ui::interaction::layout_tree::LayoutTree>>,
    /// Flat navigation solved index cache.
    pub navigation_cache: RefCell<HashMap<NavigationCacheKey, crate::ui::interaction::navigation::NavigationIndex>>,
}

impl AppRenderer {
    /// Creates a new `AppRenderer`.
    pub fn new() -> Self {
        Self {
            layout_cache: RefCell::new(HashMap::new()),
            layout_tree_cache: RefCell::new(HashMap::new()),
            navigation_cache: RefCell::new(HashMap::new()),
        }
    }

    fn get_or_create_layout(
        &self,
        key: &str,
        content: &str,
        revision: MessageRevision,
        width: usize,
        highlighter: &KeywordSyntaxHighlighter,
    ) -> CachedMessageLayout {
        let mut cache = self.layout_cache.borrow_mut();
        if let Some(cached) = cache.get(key) {
            if cached.revision == revision {
                return cached.clone();
            }
        }
        let ast = MarkdownParser::parse(content);
        let lines = MarkdownLayout::layout(&ast, width, highlighter);
        let height = lines.len();
        let new_layout = CachedMessageLayout {
            revision,
            visual_lines: lines,
            height,
        };
        cache.insert(key.to_string(), new_layout.clone());
        new_layout
    }

    /// Computes constraints and returns partitioned area Rects for widgets.
    pub fn compute_layout(&self, area: Rect) -> (Rect, Rect, Rect, Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Logo / Header
                Constraint::Min(10),  // Mid section (Sidebar + Chat)
                Constraint::Length(3), // Prompt input editor
                Constraint::Length(1), // Footer status bar
            ])
            .split(area);

        let mid_area = chunks[1];
        let has_sidebar = area.width >= 80;

        let (sidebar_area, chat_area) = if has_sidebar {
            let mid_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(25), Constraint::Min(20)])
                .split(mid_area);
            (mid_chunks[0], mid_chunks[1])
        } else {
            (Rect::default(), mid_area)
        };

        (chunks[0], sidebar_area, chat_area, chunks[2], chunks[3])
    }

    /// Derives lightweight ViewModels from state and draws all TUI components.
    pub fn draw(&self, f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
        let (header_area, sidebar_area, chat_area, prompt_area, status_area) = self.compute_layout(area);

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

        // 3. Build Chat View with Virtualization & Cache checks
        let width = chat_area.width.saturating_sub(4) as usize;
        let highlighter = KeywordSyntaxHighlighter::new();

        struct TimelineBlock {
            header: Option<String>,
            visual_lines: Vec<VisualLine>,
        }

        let mut blocks = Vec::new();
        if !state.timeline.is_empty() {
            for (_ordinal, item) in state.timeline.clone() {
                match item {
                    crate::ui::interaction::timeline::TimelineItem::Message(msg_id) => {
                        if msg_id == crate::ui::interaction::MessageId(0) {
                            if !state.active_response.is_empty() || state.is_generating() {
                                let key = format!("active_response_w{}", width);
                                let revision = MessageRevision(state.active_response_revision);
                                let layout = self.get_or_create_layout(&key, &state.active_response, revision, width, &highlighter);
                                blocks.push(TimelineBlock {
                                    header: Some("Assistant".to_string()),
                                    visual_lines: layout.visual_lines,
                                });
                            }
                        } else {
                            let idx = (msg_id.0 - 1) as usize;
                            if idx < state.active_messages.len() {
                                let msg = &state.active_messages[idx];
                                let sender = match msg.role {
                                    brain_domain::MessageRole::User => "User".to_string(),
                                    brain_domain::MessageRole::Assistant => "Assistant".to_string(),
                                    brain_domain::MessageRole::System => "System".to_string(),
                                };
                                let rev = *state.message_revisions.get(&msg.id).unwrap_or(&0);
                                let key = format!("msg_{}_w{}", msg.id.0, width);
                                let layout = self.get_or_create_layout(&key, &msg.content, MessageRevision(rev), width, &highlighter);
                                blocks.push(TimelineBlock {
                                    header: Some(sender),
                                    visual_lines: layout.visual_lines,
                                });
                            }
                        }
                    }
                    crate::ui::interaction::timeline::TimelineItem::ToolExecution(ref call_id) => {
                        let tool_opt = state.active_tool_calls.iter().find(|t| t.call_id == *call_id)
                            .or_else(|| state.message_tool_calls.values().flatten().find(|t| &t.call_id == call_id));
                        if let Some(tool) = tool_opt {
                            let expanded = state.conversation_view.expanded_tool_sections.get(&tool.call_id);
                            blocks.push(TimelineBlock {
                                header: None,
                                visual_lines: format_tool_execution(tool, theme, expanded),
                            });
                        } else {
                            blocks.push(TimelineBlock {
                                header: None,
                                visual_lines: vec![VisualLine {
                                    kind: VisualLineKind::Text,
                                    spans: vec![VisualSpan::new("🔧 Tool: [Loading...]".to_string(), VisualStyle::Normal)],
                                }],
                            });
                        }
                    }
                    crate::ui::interaction::timeline::TimelineItem::Retrieval(ref retrieval_id) => {
                        if let Some(retrieval) = state.retrievals.get(retrieval_id) {
                            blocks.push(TimelineBlock {
                                header: None,
                                visual_lines: format_retrieval_info(retrieval, theme),
                            });
                        } else {
                            blocks.push(TimelineBlock {
                                header: None,
                                visual_lines: vec![VisualLine {
                                    kind: VisualLineKind::Text,
                                    spans: vec![VisualSpan::new("🧠 Memory: [Loading...]".to_string(), VisualStyle::Normal)],
                                }],
                            });
                        }
                    }
                }
            }
        } else {
            if state.active_messages.is_empty() && !state.is_generating() {
                blocks.push(TimelineBlock {
                    header: Some("System".to_string()),
                    visual_lines: vec![VisualLine {
                        kind: VisualLineKind::Text,
                        spans: vec![VisualSpan::new("No messages in this conversation.".to_string(), VisualStyle::Normal)],
                    }],
                });
            } else {
                for msg in &state.active_messages {
                    let sender = match msg.role {
                        brain_domain::MessageRole::User => "User".to_string(),
                        brain_domain::MessageRole::Assistant => "Assistant".to_string(),
                        brain_domain::MessageRole::System => "System".to_string(),
                    };
                    let rev = *state.message_revisions.get(&msg.id).unwrap_or(&0);
                    let key = format!("msg_{}_w{}", msg.id.0, width);
                    let layout = self.get_or_create_layout(&key, &msg.content, MessageRevision(rev), width, &highlighter);
                    blocks.push(TimelineBlock {
                        header: Some(sender),
                        visual_lines: layout.visual_lines,
                    });
                }
                if !state.active_response.is_empty() || state.is_generating() {
                    let key = format!("active_response_w{}", width);
                    let revision = MessageRevision(state.active_response_revision);
                    let layout = self.get_or_create_layout(&key, &state.active_response, revision, width, &highlighter);
                    let mut visual_lines = layout.visual_lines;
                    for tool in &state.active_tool_calls {
                        let expanded = state.conversation_view.expanded_tool_sections.get(&tool.call_id);
                        visual_lines.extend(format_tool_execution(tool, theme, expanded));
                    }
                    blocks.push(TimelineBlock {
                        header: Some("Assistant".to_string()),
                        visual_lines,
                    });
                }
            }
        }

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

        // 4. Build Prompt ViewModel
        let prompt_view = PromptView {
            prompt_text: state.editor.text(),
            cursor_position: state.editor.cursor(),
            has_focus: state.focus == FocusRegion::Editor,
        };

        // 5. Build Status ViewModel
        let status_view = StatusView {
            message: " Tab: Switch Focus | Esc: Exit | Ctrl+C: Cancel | Enter: Submit ".to_string(),
        };

        // 6. Draw TUI widgets
        header::draw(f, header_area, &header_view, theme);
        chat::draw(f, chat_area, &chat_view, theme);
        prompt::draw(f, prompt_area, &prompt_view, theme);
        status::draw(f, status_area, &status_view, theme);

        // 7. Draw active overlays (Command Palette modal takes precedence over Inline Slash completion)
        use crate::ui::layout::Overlay;
        if state.command_palette().is_visible() {
            let palette_area = state.command_palette().geometry(area);
            crate::ui::widgets::palette::draw(f, palette_area, state.command_palette(), theme);
        } else if state.slash_completion().is_visible() {
            let completion_area = state.slash_completion().geometry(area);
            crate::ui::widgets::completion::draw(f, completion_area, state.slash_completion(), theme);
        }

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
    }

}

impl Default for AppRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn format_tool_execution(
    tool: &crate::ui::command::tool::ToolExecution,
    _theme: &Theme,
    expanded_sections: Option<&std::collections::HashSet<crate::ui::interaction::navigation::ToolSection>>,
) -> Vec<VisualLine> {
    use crate::ui::command::tool::ToolExecutionStatus;
    use crate::ui::interaction::markdown::{VisualSpan, VisualStyle};
    use crate::ui::interaction::navigation::ToolSection;

    let mut lines = Vec::new();
    
    // Header line
    let tool_name = &tool.tool_id.0;
    let (status_text, style) = match &tool.status {
        ToolExecutionStatus::PendingApproval => ("Awaiting Approval", VisualStyle::Bold),
        ToolExecutionStatus::Approved => ("Approved", VisualStyle::Bold),
        ToolExecutionStatus::Denied => ("Denied", VisualStyle::Normal),
        ToolExecutionStatus::Running { .. } => ("Running", VisualStyle::Bold),
        ToolExecutionStatus::Completed { .. } => ("Completed", VisualStyle::Bold),
        ToolExecutionStatus::Failed { .. } => ("Failed", VisualStyle::Normal),
    };

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![
            VisualSpan::new(
                format!("🔧 Tool: {} [ {} ]", tool_name, status_text),
                style,
            )
        ],
    });

    // Progress/details line
    match &tool.status {
        ToolExecutionStatus::Running { progress } => {
            let progress_text = match progress {
                brain_core::events::ToolProgressDetail::Determinate { completed, total, unit: _ } => {
                    let pct = if *total > 0 { ((*completed as f64) / (*total as f64)) * 100.0 } else { 0.0 };
                    let fill = ((pct / 10.0) as usize).min(10);
                    let empty = 10 - fill;
                    let bar = format!("[{}{}]", "█".repeat(fill), "░".repeat(empty));
                    format!("  Progress: {} {:.1}%", bar, pct)
                }
                brain_core::events::ToolProgressDetail::Indeterminate => {
                    "  Running...".to_string()
                }
            };

            lines.push(VisualLine {
                kind: VisualLineKind::Text,
                spans: vec![
                    VisualSpan::new(
                        progress_text,
                        VisualStyle::Normal,
                    )
                ],
            });
        }
        ToolExecutionStatus::Failed { error } => {
            lines.push(VisualLine {
                kind: VisualLineKind::Text,
                spans: vec![
                    VisualSpan::new(
                        format!("  Error: {}", error),
                        VisualStyle::Normal,
                    )
                ],
            });
        }
        _ => {}
    }

    // Lazy logs rendering
    let show_logs = expanded_sections.map(|s| s.contains(&ToolSection::Logs)).unwrap_or(false);
    if show_logs {
        for log in &tool.logs {
            lines.push(VisualLine {
                kind: VisualLineKind::Text,
                spans: vec![
                    VisualSpan::new(
                        format!("    • {}", log.message),
                        VisualStyle::CodeComment,
                    )
                ],
            });
        }
    } else if !tool.logs.is_empty() {
        lines.push(VisualLine {
            kind: VisualLineKind::Text,
            spans: vec![
                VisualSpan::new(
                    format!("    ▶ Logs collapsed ({} entries)", tool.logs.len()),
                    VisualStyle::Normal,
                )
            ],
        });
    }

    lines
}

fn format_retrieval_info(
    retrieval: &brain_domain::bkf::retrieval::RetrievalInfo,
    _theme: &Theme,
) -> Vec<VisualLine> {
    use crate::ui::interaction::markdown::{VisualSpan, VisualStyle};
    use brain_domain::bkf::retrieval::RetrievalWeight;

    let mut lines = Vec::new();

    let weight_badge = match retrieval.explanation.weight {
        RetrievalWeight::Critical => "CRITICAL",
        RetrievalWeight::High => "HIGH",
        RetrievalWeight::Normal => "NORMAL",
    };
    
    let weight_style = match retrieval.explanation.weight {
        RetrievalWeight::Critical => VisualStyle::Bold,
        RetrievalWeight::High => VisualStyle::Bold,
        RetrievalWeight::Normal => VisualStyle::Normal,
    };

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![
            VisualSpan::new(
                format!("🧠 Memory: {} [ {} ]", retrieval.title, weight_badge),
                weight_style,
            )
        ],
    });

    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![
            VisualSpan::new(
                format!("  > {}", retrieval.excerpt),
                VisualStyle::Italic,
            )
        ],
    });

    let keywords_str = retrieval.explanation.matched_keywords.join(", ");
    let recency_str = if retrieval.explanation.recency_boost {
        " (+recency boost)"
    } else {
        ""
    };
    
    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![
            VisualSpan::new(
                format!(
                    "  Matches: [{}] Similarity: {:?}{}",
                    keywords_str, retrieval.explanation.semantic_similarity, recency_str
                ),
                VisualStyle::CodeComment,
            )
        ],
    });

    let provenance_line = format!(
        "  Source: {:?} at {}",
        retrieval.explanation.provenance.kind, retrieval.explanation.provenance.location
    );
    lines.push(VisualLine {
        kind: VisualLineKind::Text,
        spans: vec![
            VisualSpan::new(provenance_line, VisualStyle::Citation),
        ],
    });

    lines
}

