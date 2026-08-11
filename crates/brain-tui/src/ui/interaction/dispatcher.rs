//! Input dispatcher coordinating mutations on interaction context.

use crate::ui::command::completion::SlashCompletionState;
use crate::ui::command::palette::{
    CollectedParameter, CommandPaletteState, PaletteStage, ParameterCollectionState, ParameterValue,
};
use crate::ui::command::{CommandAvailabilityContext, CommandInvocation, CommandRegistry};
use crate::ui::focus::FocusManager;
use crate::ui::input::{Command, InputAction, TextInput};
use crate::ui::interaction::editor::Editor;
use crate::ui::interaction::scroll::ScrollState;
use crate::ui::interaction::sidebar::{SessionLookup, SidebarEvent, SidebarInteraction};
use crate::ui::widgets::view_models::FocusTarget;
use brain_domain::SessionId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Collection of interaction sub-systems.
pub struct InteractionContext<'a> {
    /// Reference to the mutable text Editor.
    pub editor: &'a mut Editor,
    /// Reference to the mutable ScrollState.
    pub scroll: &'a mut ScrollState,
    /// Reference to the mutable FocusManager.
    pub focus: &'a mut FocusManager,
    /// Reference to the mutable SidebarInteraction.
    pub sidebar: &'a mut SidebarInteraction,
    /// Reference to the mutable SlashCompletionState.
    pub slash_completion: &'a mut SlashCompletionState,
    /// Reference to the mutable CommandPaletteState.
    pub command_palette: &'a mut CommandPaletteState,
    /// Whether the client is currently generating a response.
    pub is_generating: bool,
    /// Whether the client is currently connected.
    pub is_connected: bool,
    /// The visible session IDs in the sidebar.
    pub visible_ids: &'a [SessionId],
    /// The session lookup service.
    pub lookup: &'a dyn SessionLookup,
    /// Pending tool call approvals queue.
    pub pending_approvals: &'a mut Vec<crate::ui::command::tool::ToolApproval>,
    /// Full sessions list for global search contexts.
    pub sessions: &'a [crate::state::SessionViewModel],
    /// Loaded active session messages.
    pub active_messages: &'a [brain_domain::Message],
}

/// Abstract user interface intent events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// Intent to submit prompt text.
    SubmitPrompt(String),
    /// Intent to resize terminal window.
    Resize(u16, u16),
    /// Intent from sidebar action.
    Sidebar(SidebarEvent),
    /// Intent to execute command invocation.
    Command(CommandInvocation),
    /// Intent to approve or deny a tool call.
    ApproveToolCall {
        /// Unique identifier for the tool call.
        call_id: brain_core::events::ToolCallId,
        /// True if approved, false if denied.
        approved: bool,
    },
    /// Intent to select a search result action.
    SearchSelect(crate::ui::search::types::SearchResultAction),
}

/// Dispatcher result codes representing TUI state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    /// Request interface frame redrawing.
    pub needs_render: bool,
    /// Request client shutdown.
    pub should_exit: bool,
    /// Optional semantic application event emitted.
    pub ui_event: Option<UiEvent>,
}

impl DispatchResult {
    /// Instantiates a DispatchResult with no flags set.
    pub fn none() -> Self {
        Self {
            needs_render: false,
            should_exit: false,
            ui_event: None,
        }
    }

    /// Instantiates a DispatchResult requesting redraw.
    pub fn render() -> Self {
        Self {
            needs_render: true,
            should_exit: false,
            ui_event: None,
        }
    }

    /// Instantiates a DispatchResult requesting exit.
    pub fn exit() -> Self {
        Self {
            needs_render: false,
            should_exit: true,
            ui_event: None,
        }
    }

    /// Instantiates a DispatchResult wrapping a UI event.
    pub fn event(event: UiEvent) -> Self {
        Self {
            needs_render: true,
            should_exit: false,
            ui_event: Some(event),
        }
    }
}

/// Central interaction routing manager.
pub struct Dispatcher;

impl Dispatcher {
    /// Executes the InputAction against the given sub-systems context.
    pub fn dispatch(action: InputAction, ctx: &mut InteractionContext<'_>) -> DispatchResult {
        if ctx.focus.current() == FocusTarget::Dialog {
            if let Some(first_approval) = ctx.pending_approvals.first() {
                let call_id = first_approval.call_id.clone();
                match action {
                    InputAction::Text(TextInput::Char('y'))
                    | InputAction::Text(TextInput::Char('Y')) => {
                        return DispatchResult::event(UiEvent::ApproveToolCall {
                            call_id,
                            approved: true,
                        });
                    }
                    InputAction::Text(TextInput::Char('n'))
                    | InputAction::Text(TextInput::Char('N')) => {
                        return DispatchResult::event(UiEvent::ApproveToolCall {
                            call_id,
                            approved: false,
                        });
                    }
                    InputAction::Command(Command::Submit) => {
                        return DispatchResult::event(UiEvent::ApproveToolCall {
                            call_id,
                            approved: true,
                        });
                    }
                    InputAction::Command(Command::Exit) | InputAction::Command(Command::Escape) => {
                        return DispatchResult::event(UiEvent::ApproveToolCall {
                            call_id,
                            approved: false,
                        });
                    }
                    _ => {
                        return DispatchResult::none();
                    }
                }
            } else {
                ctx.focus.set_focus(FocusTarget::Prompt);
                return DispatchResult::render();
            }
        }

        if ctx.focus.current() == FocusTarget::Sidebar {
            let key_opt = match action {
                InputAction::Command(cmd) => match cmd {
                    Command::ScrollUp => Some(KeyEvent::new(KeyCode::Up, KeyModifiers::empty())),
                    Command::ScrollDown => {
                        Some(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()))
                    }
                    Command::Submit => Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
                    Command::Backspace => {
                        Some(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()))
                    }
                    Command::Delete => Some(KeyEvent::new(KeyCode::Delete, KeyModifiers::empty())),
                    Command::MoveLeft => Some(KeyEvent::new(KeyCode::Left, KeyModifiers::empty())),
                    Command::MoveRight => {
                        Some(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()))
                    }
                    _ => None,
                },
                InputAction::Text(TextInput::Char(c)) => {
                    Some(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
                }
                InputAction::None => None,
            };

            if let Some(key) = key_opt {
                let (handled, event) = ctx.sidebar.handle_key(key, ctx.visible_ids, ctx.lookup);
                if handled {
                    if let Some(sidebar_evt) = event {
                        return DispatchResult::event(UiEvent::Sidebar(sidebar_evt));
                    } else {
                        return DispatchResult::render();
                    }
                }
            }

            // If the sidebar is focused, prevent general editing and scrolling actions from falling through to the prompt editor or history scroll
            match action {
                InputAction::Command(cmd) => match cmd {
                    Command::Exit | Command::FocusNext | Command::FocusPrevious => {}
                    _ => return DispatchResult::none(),
                },
                InputAction::Text(_) => return DispatchResult::none(),
                InputAction::None => {}
            }
        }

        // Handle Command Palette toggle/opening globally
        if let InputAction::Command(Command::ToggleCommandPalette) = action {
            if ctx.command_palette.open {
                ctx.command_palette.reset();
                if let Some(saved) = ctx.focus.pop_saved_focus() {
                    ctx.focus.set_focus(saved);
                }
            } else {
                ctx.focus.save_focus(ctx.focus.current());
                ctx.focus.set_focus(FocusTarget::CommandPalette);
                ctx.command_palette.reset();
                ctx.command_palette.open = true;

                // Trigger initial search with empty text
                let search_context = crate::ui::search::types::SearchContext {
                    sessions: ctx.sessions.to_vec(),
                    active_messages: ctx.active_messages.to_vec(),
                };
                ctx.command_palette
                    .trigger_search("", &search_context);
            }
            return DispatchResult::render();
        }

        // If Command Palette is focused, intercept all keys
        if ctx.focus.current() == FocusTarget::CommandPalette {
            let _avail_ctx = CommandAvailabilityContext {
                has_selected_session: ctx.sidebar.browse.selected.is_some(),
                is_connected: ctx.is_connected,
                is_generating: ctx.is_generating,
            };

            match action {
                InputAction::Command(cmd) => match cmd {
                    Command::Exit | Command::Escape => {
                        ctx.command_palette.reset();
                        if let Some(saved) = ctx.focus.pop_saved_focus() {
                            ctx.focus.set_focus(saved);
                        }
                        return DispatchResult::render();
                    }
                    Command::ScrollUp => {
                        let count = match &ctx.command_palette.stage {
                            PaletteStage::Search => {
                                if ctx.command_palette.search_aggregator.is_some() {
                                    ctx.command_palette.results().len()
                                } else {
                                    ctx.command_palette.matches().len()
                                }
                            }
                            PaletteStage::CollectParameter(state) => {
                                let descriptor = CommandRegistry::find_by_id(state.command_id);
                                if let Some(desc) = descriptor {
                                    if let Some(param_desc) = state.current_parameter(desc) {
                                        match param_desc.kind {
                                            crate::ui::command::ParameterKind::Theme => 4,
                                            _ => 0,
                                        }
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                }
                            }
                            _ => 0,
                        };
                        if count > 0 {
                            if ctx.command_palette.selected_index == 0 {
                                ctx.command_palette.selected_index = count - 1;
                            } else {
                                ctx.command_palette.selected_index -= 1;
                            }
                        }
                        return DispatchResult::render();
                    }
                    Command::ScrollDown => {
                        let count = match &ctx.command_palette.stage {
                            PaletteStage::Search => {
                                if ctx.command_palette.search_aggregator.is_some() {
                                    ctx.command_palette.results().len()
                                } else {
                                    ctx.command_palette.matches().len()
                                }
                            }
                            PaletteStage::CollectParameter(state) => {
                                let descriptor = CommandRegistry::find_by_id(state.command_id);
                                if let Some(desc) = descriptor {
                                    if let Some(param_desc) = state.current_parameter(desc) {
                                        match param_desc.kind {
                                            crate::ui::command::ParameterKind::Theme => 4,
                                            _ => 0,
                                        }
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                }
                            }
                            _ => 0,
                        };
                        if count > 0 {
                            ctx.command_palette.selected_index =
                                (ctx.command_palette.selected_index + 1) % count;
                        }
                        return DispatchResult::render();
                    }
                    Command::Submit => {
                        match &mut ctx.command_palette.stage {
                            PaletteStage::Search => {
                                let matched_cmd = ctx
                                    .command_palette
                                    .matches()
                                    .get(ctx.command_palette.selected_index)
                                    .cloned();
                                if let Some(cmd) = matched_cmd {
                                    let command_id = crate::ui::command::CommandId(cmd.id);
                                    if let Some(desc) = CommandRegistry::find_by_id(command_id) {
                                        if !desc.parameters.is_empty() {
                                            ctx.command_palette.stage =
                                                PaletteStage::CollectParameter(
                                                    ParameterCollectionState::new(command_id),
                                                );
                                            ctx.command_palette.editor.clear();
                                            ctx.command_palette.selected_index = 0;
                                            return DispatchResult::render();
                                        }
                                        let inv_opt = CommandInvocation::build(
                                            command_id,
                                            &[],
                                            ctx.sidebar.browse.selected,
                                        );
                                        ctx.command_palette.reset();
                                        if let Some(saved) = ctx.focus.pop_saved_focus() {
                                            ctx.focus.set_focus(saved);
                                        }
                                        if let Some(inv) = inv_opt {
                                            return DispatchResult::event(UiEvent::Command(inv));
                                        }
                                    }
                                }
                            }
                            PaletteStage::CollectParameter(state) => {
                                let descriptor = CommandRegistry::find_by_id(state.command_id);
                                if let Some(desc) = descriptor {
                                    if let Some(param_desc) = state.current_parameter(desc) {
                                         let mut text =
                                             ctx.command_palette.editor.text().trim().to_string();
                                         if text.is_empty() {
                                             text = match param_desc.kind {
                                                 crate::ui::command::ParameterKind::Theme => {
                                                     match ctx.command_palette.selected_index {
                                                         0 => "dark".to_string(),
                                                         1 => "light".to_string(),
                                                         2 => "terminal".to_string(),
                                                         3 => "high_contrast".to_string(),
                                                         _ => "dark".to_string(),
                                                     }
                                                 }
                                                 _ => "default".to_string(),
                                             };
                                         }
                                         if !text.is_empty() {
                                            let val = match param_desc.kind {
                                                crate::ui::command::ParameterKind::String => {
                                                    Some(ParameterValue::String(text))
                                                }
                                                crate::ui::command::ParameterKind::Theme => {
                                                    let lower = text.to_lowercase();
                                                    let theme_str = if lower.contains("contrast") {
                                                        "high_contrast"
                                                    } else if lower.contains("light") {
                                                        "light"
                                                    } else if lower.contains("terminal") {
                                                        "terminal"
                                                    } else {
                                                        "dark"
                                                    };
                                                    Some(ParameterValue::Theme(
                                                        crate::ui::command::ThemeId(theme_str),
                                                    ))
                                                }
                                                _ => Some(ParameterValue::String(text)),
                                            };
                                            if let Some(param_val) = val {
                                                state.collected.push(CollectedParameter {
                                                    id: param_desc.id,
                                                    value: param_val,
                                                });
                                                ctx.command_palette.editor.clear();
                                                if state.collected.len() == desc.parameters.len() {
                                                    // All parameters collected — build and dispatch.
                                                    let inv_opt = CommandInvocation::build(
                                                        state.command_id,
                                                        &state.collected,
                                                        ctx.sidebar.browse.selected,
                                                    );
                                                    ctx.command_palette.reset();
                                                    if let Some(saved) = ctx.focus.pop_saved_focus()
                                                    {
                                                        ctx.focus.set_focus(saved);
                                                    }
                                                    if let Some(inv) = inv_opt {
                                                        return DispatchResult::event(
                                                            UiEvent::Command(inv),
                                                        );
                                                    }
                                                }
                                                // Render the prompt for the next parameter.
                                                // The editor has been cleared; the widget reads
                                                // state.current_parameter() to know what label
                                                // to display for the new parameter slot.
                                                return DispatchResult::render();
                                            }
                                        }
                                    }
                                }
                            }

                            PaletteStage::Confirm { .. } => {
                                // TODO: The Confirm stage is currently an unimplemented stub.
                                // Pressing Enter here resets the palette without dispatching
                                // any command. This stage was reserved for destructive-action
                                // confirmation UX but has not been designed or implemented yet.
                                // Either implement confirmation flow or remove the variant in a
                                // future milestone.
                                ctx.command_palette.reset();
                                if let Some(saved) = ctx.focus.pop_saved_focus() {
                                    ctx.focus.set_focus(saved);
                                }
                            }
                        }
                        return DispatchResult::render();
                    }
                    // Tab (FocusNext) in CollectParameter acts as "advance to next parameter".
                    // If there is a next parameter, Tab behaves identically to Enter —
                    // collecting the current value and moving to the next field.
                    // If the current parameter is the last, Tab also behaves like Enter
                    // (submits and dispatches the command).
                    //
                    // In Search stage, Tab is a no-op (no suggestion cycling in the palette).
                    Command::FocusNext => {
                        if let PaletteStage::CollectParameter(state) =
                            &mut ctx.command_palette.stage
                        {
                            let descriptor = CommandRegistry::find_by_id(state.command_id);
                            if let Some(desc) = descriptor {
                                if let Some(param_desc) = state.current_parameter(desc) {
                                    let mut text = ctx.command_palette.editor.text().trim().to_string();
                                    if text.is_empty() {
                                        text = match param_desc.kind {
                                            crate::ui::command::ParameterKind::Theme => {
                                                match ctx.command_palette.selected_index {
                                                    0 => "dark".to_string(),
                                                    1 => "light".to_string(),
                                                    2 => "terminal".to_string(),
                                                    3 => "high_contrast".to_string(),
                                                    _ => "dark".to_string(),
                                                }
                                            }
                                            _ => "default".to_string(),
                                        };
                                    }
                                    if !text.is_empty() {
                                        let val = match param_desc.kind {
                                            crate::ui::command::ParameterKind::String => {
                                                Some(ParameterValue::String(text))
                                            }
                                            crate::ui::command::ParameterKind::Theme => {
                                                let lower = text.to_lowercase();
                                                let theme_str = if lower.contains("contrast") {
                                                    "high_contrast"
                                                } else if lower.contains("light") {
                                                    "light"
                                                } else if lower.contains("terminal") {
                                                    "terminal"
                                                } else {
                                                    "dark"
                                                };
                                                Some(ParameterValue::Theme(
                                                    crate::ui::command::ThemeId(theme_str),
                                                ))
                                            }
                                            _ => Some(ParameterValue::String(text)),
                                        };
                                        if let Some(param_val) = val {
                                            state.collected.push(CollectedParameter {
                                                id: param_desc.id,
                                                value: param_val,
                                            });
                                            ctx.command_palette.editor.clear();
                                            if state.collected.len() == desc.parameters.len() {
                                                // Last parameter — build and dispatch.
                                                let inv_opt = CommandInvocation::build(
                                                    state.command_id,
                                                    &state.collected,
                                                    ctx.sidebar.browse.selected,
                                                );
                                                ctx.command_palette.reset();
                                                if let Some(saved) = ctx.focus.pop_saved_focus() {
                                                    ctx.focus.set_focus(saved);
                                                }
                                                if let Some(inv) = inv_opt {
                                                    return DispatchResult::event(
                                                        UiEvent::Command(inv),
                                                    );
                                                }
                                            }
                                            // Advance to next parameter field.
                                            return DispatchResult::render();
                                        }
                                    }
                                }
                            }
                        }
                        return DispatchResult::render();
                    }
                    Command::Backspace => {
                        ctx.command_palette.editor.backspace();
                        if let PaletteStage::Search = ctx.command_palette.stage {
                            let query_text = ctx.command_palette.editor.text().to_string();
                            let search_context = crate::ui::search::types::SearchContext {
                                sessions: ctx.sessions.to_vec(),
                                active_messages: ctx.active_messages.to_vec(),
                            };
                            ctx.command_palette
                                .trigger_search(query_text, &search_context);
                        }
                        return DispatchResult::render();
                    }
                    Command::Delete => {
                        ctx.command_palette.editor.delete();
                        if let PaletteStage::Search = ctx.command_palette.stage {
                            let query_text = ctx.command_palette.editor.text().to_string();
                            let search_context = crate::ui::search::types::SearchContext {
                                sessions: ctx.sessions.to_vec(),
                                active_messages: ctx.active_messages.to_vec(),
                            };
                            ctx.command_palette
                                .trigger_search(query_text, &search_context);
                        }
                        return DispatchResult::render();
                    }
                    Command::MoveLeft => {
                        ctx.command_palette.editor.move_cursor_left();
                        return DispatchResult::render();
                    }
                    Command::MoveRight => {
                        ctx.command_palette.editor.move_cursor_right();
                        return DispatchResult::render();
                    }
                    _ => {}
                },
                InputAction::Text(TextInput::Char(c)) => {
                    ctx.command_palette.editor.insert(c);
                    if let PaletteStage::Search = ctx.command_palette.stage {
                        let query_text = ctx.command_palette.editor.text().to_string();
                        let search_context = crate::ui::search::types::SearchContext {
                            sessions: ctx.sessions.to_vec(),
                            active_messages: ctx.active_messages.to_vec(),
                        };
                        ctx.command_palette
                            .trigger_search(query_text, &search_context);
                    }
                    return DispatchResult::render();
                }
                _ => {}
            }
            return DispatchResult::none();
        }

        // If Prompt is focused and slash completion is visible, Arrow keys, Tab, and Escape
        // interact with suggestions.
        //
        // Escape dismissal semantic:
        //   One Escape dismisses the completion for the *current exact query string* only.
        //   dismissed_query caches at most one query — typing any different text clears it.
        //   See SlashCompletionState::dismissed_query and sync_slash_completion() in state.rs.
        if ctx.focus.current() == FocusTarget::Prompt && ctx.slash_completion.visible {
            let count = crate::ui::command::completion::SlashCompletionEngine::matches(
                &ctx.slash_completion.query,
            )
            .count();

            // Escape: dismiss popup and cache the dismissed query to prevent re-opening
            // while the user is still on the same text. Typing a different character
            // (handled in the post-processing block below) clears dismissed_query.
            if let InputAction::Command(Command::Escape) = action {
                ctx.slash_completion.dismissed_query = Some(ctx.slash_completion.query.clone());
                ctx.slash_completion.visible = false;
                return DispatchResult::render();
            }

            if count > 0 {
                if let InputAction::Command(cmd) = action {
                    match cmd {
                        Command::ScrollUp => {
                            if ctx.slash_completion.selected_index == 0 {
                                ctx.slash_completion.selected_index = count - 1;
                            } else {
                                ctx.slash_completion.selected_index -= 1;
                            }
                            return DispatchResult::render();
                        }
                        Command::ScrollDown => {
                            ctx.slash_completion.selected_index =
                                (ctx.slash_completion.selected_index + 1) % count;
                            return DispatchResult::render();
                        }
                        Command::FocusNext => {
                            let matched_cmd =
                                crate::ui::command::completion::SlashCompletionEngine::matches(
                                    &ctx.slash_completion.query,
                                )
                                .nth(ctx.slash_completion.selected_index);
                            if let Some(cmd_desc) = matched_cmd {
                                ctx.editor.clear();
                                let mut alias_text =
                                    format!("/{}", cmd_desc.aliases.first().unwrap_or(&""));
                                if !cmd_desc.parameters.is_empty() {
                                    alias_text.push(' ');
                                }
                                for c in alias_text.chars() {
                                    ctx.editor.insert(c);
                                }
                                ctx.editor.move_to_end();
                            }
                            ctx.slash_completion.visible = false;
                            return DispatchResult::render();
                        }
                        _ => {}
                    }
                }
            }
        }

        let res = match action {
            InputAction::Command(cmd) => match cmd {
                Command::Exit => DispatchResult::exit(),
                Command::FocusNext => {
                    ctx.focus.next();
                    DispatchResult::render()
                }
                Command::FocusPrevious => {
                    ctx.focus.prev();
                    DispatchResult::render()
                }
                Command::Backspace => {
                    ctx.editor.backspace();
                    DispatchResult::render()
                }
                Command::Delete => {
                    ctx.editor.delete();
                    DispatchResult::render()
                }
                Command::MoveHome => {
                    ctx.editor.move_to_home();
                    DispatchResult::render()
                }
                Command::MoveEnd => {
                    ctx.editor.move_to_end();
                    DispatchResult::render()
                }
                Command::MoveLeft => {
                    ctx.editor.move_cursor_left();
                    DispatchResult::render()
                }
                Command::MoveRight => {
                    ctx.editor.move_cursor_right();
                    DispatchResult::render()
                }
                Command::ScrollUp => {
                    ctx.scroll.scroll_up();
                    DispatchResult::render()
                }
                Command::ScrollDown => {
                    ctx.scroll.scroll_down();
                    DispatchResult::render()
                }
                Command::Submit => {
                    let text = ctx.editor.text().trim().to_string();
                    if text.is_empty() {
                        DispatchResult::none()
                    } else if let Some(stripped) = text.strip_prefix('/') {
                        let clean_command = stripped.trim_start_matches('/');
                        let parts: Vec<&str> = clean_command.split_whitespace().collect();
                        let command_name = parts.first().copied().unwrap_or("");
                        let arg = parts.get(1).copied().unwrap_or("");

                        ctx.editor.clear();
                        ctx.slash_completion.visible = false;

                        match command_name.to_lowercase().as_str() {
                            "session" | "new" => DispatchResult::event(UiEvent::Command(
                                CommandInvocation::CreateSession,
                            )),
                            "theme" => {
                                let theme_id = match arg.to_lowercase().as_str() {
                                    "light" => crate::ui::command::THEME_LIGHT,
                                    "terminal" => crate::ui::command::THEME_TERMINAL,
                                    "high_contrast" | "contrast" => {
                                        crate::ui::command::THEME_HIGH_CONTRAST
                                    }
                                    _ => crate::ui::command::THEME_DARK,
                                };
                                DispatchResult::event(UiEvent::Command(
                                    CommandInvocation::ChangeTheme { theme: theme_id },
                                ))
                            }
                            "help" | "info" => {
                                DispatchResult::event(UiEvent::Command(CommandInvocation::ShowHelp))
                            }
                            "clear" | "cls" => DispatchResult::event(UiEvent::Command(
                                CommandInvocation::ClearChat,
                            )),
                            "toggle" | "reflection" => DispatchResult::event(UiEvent::Command(
                                CommandInvocation::ToggleReflection,
                            )),
                            "rename" if !arg.is_empty() => {
                                if let Some(session_id) = ctx.sidebar.browse.selected {
                                    DispatchResult::event(UiEvent::Command(
                                        CommandInvocation::RenameSession {
                                            session_id,
                                            title: crate::ui::command::SessionTitle(
                                                arg.to_string(),
                                            ),
                                        },
                                    ))
                                } else {
                                    DispatchResult::none()
                                }
                            }
                            _ => {
                                // For unrecognized slash commands, submit as system help note rather than sending /command as a retrieval query
                                DispatchResult::event(UiEvent::SubmitPrompt(format!("System: Unknown slash command '/{}'. Type /help to view available commands.", command_name)))
                            }
                        }
                    } else {
                        DispatchResult::event(UiEvent::SubmitPrompt(text))
                    }
                }
                Command::ToggleCommandPalette => DispatchResult::render(),
                Command::Escape => DispatchResult::none(),
            },

            InputAction::Text(text_input) => match text_input {
                TextInput::Char(c) => {
                    ctx.editor.insert(c);
                    DispatchResult::render()
                }
            },
            InputAction::None => DispatchResult::none(),
        };

        // Post-processing for prompt edit to toggle/update slash completion
        if ctx.focus.current() == FocusTarget::Prompt {
            let text = ctx.editor.text();
            if text.starts_with('/') && !text.contains(' ') {
                if ctx.slash_completion.dismissed_query.as_deref() == Some(text) {
                    ctx.slash_completion.visible = false;
                } else {
                    ctx.slash_completion.dismissed_query = None;
                    ctx.slash_completion.visible = true;
                    ctx.slash_completion.query = text.to_string();
                    let count = crate::ui::command::completion::SlashCompletionEngine::matches(
                        &ctx.slash_completion.query,
                    )
                    .count();
                    if count == 0 {
                        ctx.slash_completion.visible = false;
                    } else if ctx.slash_completion.selected_index >= count {
                        ctx.slash_completion.selected_index = 0;
                    }
                }
            } else {
                ctx.slash_completion.visible = false;
                ctx.slash_completion.dismissed_query = None;
            }
        }

        res
    }
}
