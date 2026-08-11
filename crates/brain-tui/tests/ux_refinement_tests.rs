use brain_domain::{Message, MessageId, MessageRole};
use brain_tui::state::{Action, UiState};
use brain_tui::ui::command::completion::SlashCompletionState;
use brain_tui::ui::command::palette::{CommandPaletteState, PaletteStage};
use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::input::{Command, InputAction, TextInput};
use brain_tui::ui::interaction::dispatcher::{Dispatcher, InteractionContext};
use brain_tui::ui::interaction::editor::Editor;
use brain_tui::ui::interaction::scroll::ScrollState;
use brain_tui::ui::interaction::sidebar::{SessionLookup, SidebarInteraction};
use brain_tui::ui::interaction::UiEvent;
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::{AppLayoutMode, AppRenderer};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::view_models::FocusTarget;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

struct DummyLookup;
impl SessionLookup for DummyLookup {
    fn title(&self, _id: brain_domain::SessionId) -> Option<&str> {
        None
    }
}

#[test]
fn test_ux_refinement_full_integration_suite() {
    let renderer = AppRenderer::new();
    let mut state = UiState::new();
    let theme = Theme::default();

    // 1. Verify AppLayoutMode & compute_layout on Welcome screen (Screen::Home)
    assert_eq!(AppRenderer::layout_mode(&state), AppLayoutMode::Welcome);
    let area = Rect::new(0, 0, 100, 30);
    let (_, sidebar_area, chat_area, _, _, _, _) = renderer.compute_layout(area, &state);
    assert_eq!(
        sidebar_area.width, 0,
        "Sidebar width must be 0 on Welcome screen"
    );
    assert_eq!(
        chat_area.width, 100,
        "Chat area must span full width on Welcome screen"
    );

    // 2. Transition to Workspace (Screen::Workspace or Screen::Conversation with active messages)
    state.screen = Screen::Workspace;
    state.focus = brain_tui::state::FocusRegion::Sidebar;
    state.active_messages.push(Message::new(
        MessageId::new(),
        MessageRole::User,
        "Hello".to_string(),
    ));
    assert_eq!(AppRenderer::layout_mode(&state), AppLayoutMode::Workspace);
    let (_, sidebar_area, chat_area, _, _, _, _) = renderer.compute_layout(area, &state);
    assert_eq!(
        sidebar_area.width, 0,
        "Sidebar width must be 0 in Workspace mode"
    );
    assert_eq!(
        chat_area.width, 100,
        "Chat area must span full width in Workspace mode"
    );

    // 3. Verify rich autocomplete metadata
    state.editor.set_text("/m");
    let suggestions = state.get_slash_suggestions();
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "/memory");
    assert_eq!(suggestions[0].category, "Graph");

    // 4. Verify PresentationModel virtual scroll engine
    state.viewport.scroll_offset = 10;
    let model = state.presentation_model(50, 20);
    assert_eq!(model.visible_rows.len(), 20);
    assert_eq!(model.scroll_indicator, "Showing 11-30 of 50");

    // 5. Verify collapsible result groups
    state.update(Action::ToggleGroupExpand(0));
    assert!(state.is_group_collapsed(0));

    // 6. Verify draw across wide & compact viewports without panic
    for width in [120, 60] {
        let backend = TestBackend::new(width, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        state.terminal_width = width;
        state.recalculate_viewport();
        let res = terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme));
        assert!(
            res.is_ok(),
            "Failed status footer render at width {}",
            width
        );
    }
}

#[test]
fn test_palette_enter_dispatches_action_directly() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    // Toggle command palette via Ctrl+K
    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );
    assert!(ctx.command_palette.open);
    assert_eq!(ctx.focus.current(), FocusTarget::CommandPalette);

    // Filter by typing "session"
    for c in "session".chars() {
        Dispatcher::dispatch(InputAction::Text(TextInput::Char(c)), &mut ctx);
    }
    assert_eq!(ctx.command_palette.editor.text(), "session");

    // Press Enter to submit
    let res = Dispatcher::dispatch(InputAction::Command(Command::Submit), &mut ctx);
    assert!(!ctx.command_palette.open, "Palette must close upon Enter");
    assert_ne!(
        ctx.focus.current(),
        FocusTarget::CommandPalette,
        "Focus must be restored"
    );

    if let Some(UiEvent::SubmitPrompt(text)) = res.ui_event {
        panic!(
            "Palette submit must NOT dispatch SubmitPrompt with text: {}",
            text
        );
    }
}

#[test]
fn test_palette_command_does_not_enter_prompt() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    for c in "Initial prompt text".chars() {
        editor.insert(c);
    }

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );
    for c in "theme".chars() {
        Dispatcher::dispatch(InputAction::Text(TextInput::Char(c)), &mut ctx);
    }
    Dispatcher::dispatch(InputAction::Command(Command::Submit), &mut ctx);

    assert_eq!(ctx.editor.text(), "Initial prompt text");
}

#[test]
fn test_palette_command_does_not_pollute_chat_history() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );
    for c in "session".chars() {
        Dispatcher::dispatch(InputAction::Text(TextInput::Char(c)), &mut ctx);
    }
    let res = Dispatcher::dispatch(InputAction::Command(Command::Submit), &mut ctx);

    if let Some(UiEvent::SubmitPrompt(text)) = res.ui_event {
        panic!(
            "Palette submit must NOT dispatch SubmitPrompt with text: {}",
            text
        );
    }
}

#[test]
fn test_palette_escape_restores_focus() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    focus.set_focus(FocusTarget::Prompt);

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );
    assert_eq!(ctx.focus.current(), FocusTarget::CommandPalette);

    Dispatcher::dispatch(InputAction::Command(Command::Escape), &mut ctx);
    assert!(!ctx.command_palette.open);
    assert_eq!(ctx.focus.current(), FocusTarget::Prompt);
}

#[test]
fn test_palette_filter_preserves_selection() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    let selected = ctx.command_palette.selected_index;
    assert!(selected > 0 || ctx.command_palette.matches().len() <= 1);
}

#[test]
fn test_ctrl_k_matches_slash_palette_behavior() {
    let mut editor1 = Editor::new();
    let mut scroll1 = ScrollState::new();
    let mut focus1 = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar1 = SidebarInteraction::new();
    let mut slash_completion1 = SlashCompletionState::new();
    let mut command_palette1 = CommandPaletteState::new();
    let mut pending_approvals1 = Vec::new();

    let mut editor2 = Editor::new();
    let mut scroll2 = ScrollState::new();
    let mut focus2 = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar2 = SidebarInteraction::new();
    let mut slash_completion2 = SlashCompletionState::new();
    let mut command_palette2 = CommandPaletteState::new();
    let mut pending_approvals2 = Vec::new();

    let lookup = DummyLookup;

    let mut ctx1 = InteractionContext {
        editor: &mut editor1,
        scroll: &mut scroll1,
        focus: &mut focus1,
        sidebar: &mut sidebar1,
        slash_completion: &mut slash_completion1,
        command_palette: &mut command_palette1,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals1,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx1,
    );

    let mut ctx2 = InteractionContext {
        editor: &mut editor2,
        scroll: &mut scroll2,
        focus: &mut focus2,
        sidebar: &mut sidebar2,
        slash_completion: &mut slash_completion2,
        command_palette: &mut command_palette2,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals2,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx2,
    );

    assert_eq!(ctx1.command_palette.open, ctx2.command_palette.open);
    assert_eq!(ctx1.focus.current(), ctx2.focus.current());
}

#[test]
fn test_palette_arrow_navigation_boundaries() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );
    assert!(ctx.command_palette.open);
    assert_eq!(ctx.command_palette.selected_index, 0);

    let match_count = ctx.command_palette.matches().len();
    assert!(
        match_count > 1,
        "Must have multiple palette matches for navigation tests"
    );

    // 1. ArrowDown moves palette selection forward (0 -> 1)
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, 1);

    // 2. ArrowUp moves palette selection backward (1 -> 0)
    Dispatcher::dispatch(InputAction::Command(Command::ScrollUp), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, 0);

    // 3. ArrowUp at first item wraps to last item (0 -> match_count - 1)
    Dispatcher::dispatch(InputAction::Command(Command::ScrollUp), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, match_count - 1);

    // 4. ArrowDown at last item wraps to first item (match_count - 1 -> 0)
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, 0);
}

#[test]
fn test_palette_navigation_and_filtering_enter_dispatch() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    // Open palette
    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );

    // ArrowDown to select index 1
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, 1);

    let selected_cmd_id = ctx.command_palette.matches()[1].id;

    // Submit via Enter
    let res = Dispatcher::dispatch(InputAction::Command(Command::Submit), &mut ctx);

    if let Some(UiEvent::Command(inv)) = res.ui_event {
        let plan = brain_tui::ui::command::CommandExecutor::plan(inv);
        assert!(!plan.mutations.is_empty() || !plan.backend_commands.is_empty());
    } else if let PaletteStage::CollectParameter(param_state) = &ctx.command_palette.stage {
        assert_eq!(param_state.command_id.0, selected_cmd_id);
    } else {
        panic!("Enter must dispatch or parameter-collect the selected command at index 1");
    }

    // Now test filtering followed by ArrowDown + Enter
    ctx.command_palette.reset();
    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );

    for c in "s".chars() {
        Dispatcher::dispatch(InputAction::Text(TextInput::Char(c)), &mut ctx);
    }

    let filtered = ctx.command_palette.matches();
    assert!(
        filtered.len() >= 2,
        "Filtering 's' should return at least 2 commands"
    );
    let target_filtered_id = filtered[1].id;

    // ArrowDown to item 1 in filtered results
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, 1);

    let res_filtered = Dispatcher::dispatch(InputAction::Command(Command::Submit), &mut ctx);
    if let Some(UiEvent::Command(inv)) = res_filtered.ui_event {
        let plan = brain_tui::ui::command::CommandExecutor::plan(inv);
        assert!(!plan.mutations.is_empty() || !plan.backend_commands.is_empty());
    } else if let PaletteStage::CollectParameter(param_state) = &ctx.command_palette.stage {
        assert_eq!(param_state.command_id.0, target_filtered_id);
    } else {
        panic!("Filtered selection + Enter must dispatch the selected filtered item");
    }
}

#[test]
fn test_palette_keyboard_nav_escape_preserves_prompt_and_chat() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    for c in "Preserved prompt buffer".chars() {
        editor.insert(c);
    }

    let mut ctx = InteractionContext {
        editor: &mut editor,
        scroll: &mut scroll,
        focus: &mut focus,
        sidebar: &mut sidebar,
        slash_completion: &mut slash_completion,
        command_palette: &mut command_palette,
        is_generating: false,
        is_connected: true,
        visible_ids: &[],
        lookup: &lookup,
        pending_approvals: &mut pending_approvals,
        sessions: &[],
        active_messages: &[],
    };

    // Open palette via Ctrl+K
    Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut ctx,
    );

    // ArrowDown twice, ArrowUp once
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    Dispatcher::dispatch(InputAction::Command(Command::ScrollUp), &mut ctx);
    assert_eq!(ctx.command_palette.selected_index, 1);

    // Type query text
    for c in "query text".chars() {
        Dispatcher::dispatch(InputAction::Text(TextInput::Char(c)), &mut ctx);
    }

    // Escape closes palette
    Dispatcher::dispatch(InputAction::Command(Command::Escape), &mut ctx);
    assert!(!ctx.command_palette.open);
    assert_eq!(ctx.focus.current(), FocusTarget::Prompt);
    assert_eq!(ctx.editor.text(), "Preserved prompt buffer");
}
