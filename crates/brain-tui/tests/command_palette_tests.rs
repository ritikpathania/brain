use brain_tui::ui::command::{
    Availability, AvailabilityReason, CommandAvailabilityContext, CommandPolicy, CommandRegistry,
    CHANGE_THEME, RENAME_SESSION, SWITCH_MODEL,
};

#[test]
fn test_command_registry_lookups() {
    // Lookup by ID
    let theme_cmd =
        CommandRegistry::find_by_id(CHANGE_THEME).expect("Should find Change Theme command");
    assert_eq!(theme_cmd.title, "Change Theme");
    assert_eq!(theme_cmd.aliases, &["theme"]);

    // Lookup by name
    let rename_cmd = CommandRegistry::find_by_name_or_alias("rename")
        .expect("Should find Rename Session command");
    assert_eq!(rename_cmd.id, RENAME_SESSION);

    // Lookup by alias (case-insensitive check)
    let switch_model_cmd =
        CommandRegistry::find_by_name_or_alias("MODEL").expect("Should find Switch Model command");
    assert_eq!(switch_model_cmd.id, SWITCH_MODEL);

    // Non-existent command
    assert!(
        CommandRegistry::find_by_id(brain_tui::ui::command::CommandId("nonexistent")).is_none()
    );
    assert!(CommandRegistry::find_by_name_or_alias("invalid_name").is_none());
}

#[test]
fn test_command_policy_availability() {
    let rename_cmd = CommandRegistry::find_by_id(RENAME_SESSION).unwrap();
    let switch_model_cmd = CommandRegistry::find_by_id(SWITCH_MODEL).unwrap();
    let theme_cmd = CommandRegistry::find_by_id(CHANGE_THEME).unwrap();

    // Context: No active session, connected, not generating
    let ctx_no_session = CommandAvailabilityContext {
        has_selected_session: false,
        is_connected: true,
        is_generating: false,
    };

    assert!(matches!(
        CommandPolicy::availability(rename_cmd, &ctx_no_session),
        Availability::Disabled(AvailabilityReason::NoSessionSelected)
    ));
    assert!(matches!(
        CommandPolicy::availability(theme_cmd, &ctx_no_session),
        Availability::Enabled
    ));

    // Context: Session selected, connected, generating
    let ctx_generating = CommandAvailabilityContext {
        has_selected_session: true,
        is_connected: true,
        is_generating: true,
    };

    assert!(matches!(
        CommandPolicy::availability(rename_cmd, &ctx_generating),
        Availability::Enabled
    ));
    assert!(matches!(
        CommandPolicy::availability(switch_model_cmd, &ctx_generating),
        Availability::Disabled(AvailabilityReason::StreamingInProgress)
    ));
}

#[test]
fn test_slash_completion_matching() {
    use brain_tui::ui::command::completion::SlashCompletionEngine;

    // Matches /theme
    let matches: Vec<_> = SlashCompletionEngine::matches("/th").collect();
    assert!(!matches.is_empty());
    assert_eq!(matches[0].title, "Change Theme");

    // Case-insensitivity check
    let matches_upper: Vec<_> = SlashCompletionEngine::matches("/TH").collect();
    assert_eq!(matches_upper[0].title, "Change Theme");

    // Empty for non-slash inputs
    let no_matches: Vec<_> = SlashCompletionEngine::matches("theme").collect();
    assert!(no_matches.is_empty());
}

#[test]
fn test_focus_restoration_cycle() {
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::widgets::view_models::FocusTarget;

    let mut fm = FocusManager::new(FocusTarget::Sidebar, FocusProfile::Chat);

    // First cycle
    let saved1 = fm.current();
    fm.save_focus(saved1);
    fm.set_focus(FocusTarget::CommandPalette);
    assert_eq!(fm.current(), FocusTarget::CommandPalette);

    let restored1 = fm.pop_saved_focus().expect("Should have saved focus");
    fm.set_focus(restored1);
    assert_eq!(fm.current(), FocusTarget::Sidebar);
    assert!(
        fm.pop_saved_focus().is_none(),
        "Saved focus must be cleared after restoration"
    );

    // Second cycle
    let saved2 = fm.current();
    fm.save_focus(saved2);
    fm.set_focus(FocusTarget::CommandPalette);
    assert_eq!(fm.current(), FocusTarget::CommandPalette);

    let restored2 = fm.pop_saved_focus().expect("Should have saved focus");
    fm.set_focus(restored2);
    assert_eq!(fm.current(), FocusTarget::Sidebar);
}

#[test]
fn test_slash_completion_dispatch_trapping() {
    use brain_domain::SessionId;
    use brain_tui::ui::command::completion::SlashCompletionState;
    use brain_tui::ui::command::palette::CommandPaletteState;
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::input::{Command, InputAction, TextInput};
    use brain_tui::ui::interaction::{
        Dispatcher, Editor, InteractionContext, ScrollState, SidebarInteraction,
    };
    use brain_tui::ui::widgets::view_models::FocusTarget;

    struct DummyLookup;
    impl brain_tui::ui::interaction::sidebar::SessionLookup for DummyLookup {
        fn title(&self, _id: SessionId) -> Option<&str> {
            None
        }
    }

    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    let mut pending_approvals = vec![];

    // 1. Type '/' -> Should show autocomplete
    let _ = Dispatcher::dispatch(
        InputAction::Text(TextInput::Char('/')),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );

    assert!(slash_completion.visible);
    assert_eq!(slash_completion.query, "/");
    assert_eq!(slash_completion.selected_index, 0);

    // 2. Type 't' -> Should stay visible, query="/t"
    let _ = Dispatcher::dispatch(
        InputAction::Text(TextInput::Char('t')),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );

    assert!(slash_completion.visible);
    assert_eq!(slash_completion.query, "/t");

    // 3. Arrow down -> should increment selected_index (if multiple matches)
    let _ = Dispatcher::dispatch(
        InputAction::Command(Command::ScrollDown),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );

    // 4. Tab key -> should autocomplete to "/model " because we scrolled down
    let _ = Dispatcher::dispatch(
        InputAction::Command(Command::FocusNext),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );

    assert!(!slash_completion.visible);
    assert_eq!(editor.text(), "/model ");
}

#[test]
fn test_command_palette_geometry_clamps() {
    use brain_tui::ui::layout::CommandPaletteGeometry;
    use ratatui::layout::Rect;

    // Small terminal: should clamp width and height to their minimums
    let small_term = Rect::new(0, 0, 20, 5);
    let area_small = CommandPaletteGeometry::compute(small_term);
    assert_eq!(area_small.width, 20); // clamped to terminal bounds (which are smaller than min width 40)
    assert_eq!(area_small.height, 5); // clamped to terminal bounds (which are smaller than min height 8)

    // Standard/Large terminal: should clamp width to [40, 80] and height to [8, 15]
    let large_term = Rect::new(0, 0, 120, 40);
    let area_large = CommandPaletteGeometry::compute(large_term);
    assert_eq!(area_large.width, 80); // max width clamp
    assert_eq!(area_large.height, 15); // max height clamp
    assert_eq!(area_large.x, (120 - 80) / 2);
    assert_eq!(area_large.y, (40 - 15) / 2);
}

#[test]
fn test_command_palette_filtering() {
    use brain_tui::ui::command::palette::CommandPaletteState;

    let mut state = CommandPaletteState::new();
    state.editor.insert('t');
    state.editor.insert('h');

    // Searching 'th' should match Change Theme command
    let matches: Vec<_> = state.matches().collect();
    assert!(!matches.is_empty());
    assert_eq!(matches[0].title, "Change Theme");
}

#[test]
fn test_palette_parameter_collection_transitions() {
    use brain_domain::SessionId;
    use brain_tui::ui::command::completion::SlashCompletionState;
    use brain_tui::ui::command::palette::{CommandPaletteState, PaletteStage};
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::input::{Command, InputAction, TextInput};
    use brain_tui::ui::interaction::{
        Dispatcher, Editor, InteractionContext, ScrollState, SidebarInteraction,
    };
    use brain_tui::ui::widgets::view_models::FocusTarget;

    struct DummyLookup;
    impl brain_tui::ui::interaction::sidebar::SessionLookup for DummyLookup {
        fn title(&self, _id: SessionId) -> Option<&str> {
            None
        }
    }

    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    let mut pending_approvals = vec![];

    // 1. Toggle command palette open via shortcut
    let _ = Dispatcher::dispatch(
        InputAction::Command(Command::ToggleCommandPalette),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );
    assert!(command_palette.open);
    assert_eq!(focus.current(), FocusTarget::CommandPalette);
    assert!(matches!(command_palette.stage, PaletteStage::Search));

    // 2. Search for 'theme' by typing chars into command palette editor
    for c in "theme".chars() {
        let _ = Dispatcher::dispatch(
            InputAction::Text(TextInput::Char(c)),
            &mut InteractionContext {
                editor: &mut editor,
                scroll: &mut scroll,
                focus: &mut focus,
                sidebar: &mut sidebar,
                slash_completion: &mut slash_completion,
                command_palette: &mut command_palette,
                is_generating: false,
                is_connected: true,
                visible_ids: &visible_ids,
                lookup: &lookup,
                pending_approvals: &mut pending_approvals,
                sessions: &[],
                active_messages: &[],
            },
        );
    }
    assert_eq!(command_palette.editor.text(), "theme");

    // Check first match is Change Theme
    let matches: Vec<_> = command_palette.matches().collect();
    assert!(!matches.is_empty());
    assert_eq!(matches[0].title, "Change Theme");

    // 3. Press Enter to select/commit "Change Theme".
    // Since "Change Theme" has parameters, it should transition to CollectParameter!
    let _ = Dispatcher::dispatch(
        InputAction::Command(Command::Submit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );
    assert!(command_palette.open);
    if let PaletteStage::CollectParameter(state) = &command_palette.stage {
        assert_eq!(state.command_id, brain_tui::ui::command::CHANGE_THEME);
        assert_eq!(state.collected.len(), 0);
    } else {
        panic!("Should have transitioned to CollectParameter stage");
    }
    assert_eq!(command_palette.editor.text(), ""); // Editor is cleared to collect theme parameter

    // 4. Type the parameter value 'dark' and press Enter to commit parameter
    for c in "dark".chars() {
        let _ = Dispatcher::dispatch(
            InputAction::Text(TextInput::Char(c)),
            &mut InteractionContext {
                editor: &mut editor,
                scroll: &mut scroll,
                focus: &mut focus,
                sidebar: &mut sidebar,
                slash_completion: &mut slash_completion,
                command_palette: &mut command_palette,
                is_generating: false,
                is_connected: true,
                visible_ids: &visible_ids,
                lookup: &lookup,
                pending_approvals: &mut pending_approvals,
                sessions: &[],
                active_messages: &[],
            },
        );
    }
    assert_eq!(command_palette.editor.text(), "dark");

    let _ = Dispatcher::dispatch(
        InputAction::Command(Command::Submit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );

    // After all parameters are collected, command palette closes and focus is restored to Prompt!
    assert!(!command_palette.open);
    assert_eq!(focus.current(), FocusTarget::Prompt);
}

#[test]
fn test_command_execution_pipeline() {
    use brain_tui::ui::command::palette::CommandPaletteState;
    use brain_tui::ui::command::{CommandExecutor, CommandInvocation, LocalStateMutation, ThemeId};

    use brain_domain::SessionId;
    use brain_tui::ui::command::completion::SlashCompletionState;
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::input::{Command, InputAction, TextInput};
    use brain_tui::ui::interaction::{
        Dispatcher, Editor, InteractionContext, ScrollState, SidebarInteraction, UiEvent,
    };
    use brain_tui::ui::widgets::view_models::FocusTarget;

    // 1. Verify CommandExecutor::plan outputs
    let theme_inv = CommandInvocation::ChangeTheme {
        theme: ThemeId("dark"),
    };
    let plan = CommandExecutor::plan(theme_inv);
    assert_eq!(
        plan.mutations,
        vec![LocalStateMutation::ApplyTheme(ThemeId("dark"))]
    );
    assert!(plan.backend_commands.is_empty());

    let session_id = SessionId::new();
    let rename_inv = CommandInvocation::RenameSession {
        session_id,
        title: brain_tui::ui::command::SessionTitle("New Name".to_string()),
    };
    let plan_rename = CommandExecutor::plan(rename_inv);
    assert_eq!(
        plan_rename.mutations,
        vec![LocalStateMutation::RenameSession(
            session_id,
            "New Name".to_string()
        )]
    );
    assert_eq!(
        plan_rename.backend_commands,
        vec![brain_tui::ui::protocol::BackendCommand::RenameSession {
            session_id,
            title: Some("New Name".to_string()),
        }]
    );

    // 2. Verify Dispatcher emits UiEvent::Command for no-parameter commands immediately
    struct DummyLookup;
    impl brain_tui::ui::interaction::sidebar::SessionLookup for DummyLookup {
        fn title(&self, _id: SessionId) -> Option<&str> {
            None
        }
    }

    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::CommandPalette, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    let mut pending_approvals = vec![];

    // Input "clear" to select Clear Chat
    for c in "clear".chars() {
        let _ = Dispatcher::dispatch(
            InputAction::Text(TextInput::Char(c)),
            &mut InteractionContext {
                editor: &mut editor,
                scroll: &mut scroll,
                focus: &mut focus,
                sidebar: &mut sidebar,
                slash_completion: &mut slash_completion,
                command_palette: &mut command_palette,
                is_generating: false,
                is_connected: true,
                visible_ids: &visible_ids,
                lookup: &lookup,
                pending_approvals: &mut pending_approvals,
                sessions: &[],
                active_messages: &[],
            },
        );
    }

    let res = Dispatcher::dispatch(
        InputAction::Command(Command::Submit),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            command_palette: &mut command_palette,
            is_generating: false,
            is_connected: true,
            visible_ids: &visible_ids,
            lookup: &lookup,
            pending_approvals: &mut pending_approvals,
            sessions: &[],
            active_messages: &[],
        },
    );

    // Should close palette and emit ClearChat command event!
    assert!(!command_palette.open);
    assert_eq!(
        res.ui_event,
        Some(UiEvent::Command(CommandInvocation::ClearChat))
    );
}

#[test]
fn test_command_palette_widget_rendering() {
    use brain_tui::ui::command::completion::SlashCompletionState;
    use brain_tui::ui::command::palette::{
        CommandPaletteState, PaletteStage, ParameterCollectionState,
    };
    use brain_tui::ui::theme::Theme;
    use brain_tui::ui::widgets::completion;
    use brain_tui::ui::widgets::palette;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // 1. Render Search Stage
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    let mut state = CommandPaletteState::new();
    state.stage = PaletteStage::Search;
    state.editor.insert('t');
    state.editor.insert('h');

    let area = ratatui::layout::Rect::new(10, 2, 60, 15);
    terminal
        .draw(|f| {
            palette::draw(f, area, &state, &theme);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    // Validate centering/sizing and title text exists in buffer
    let mut title_found = false;
    for y in 0..20 {
        for x in 0..80 {
            let cell = buffer.get(x, y);
            if cell.symbol() == "P" {
                // "Palette" contains 'P'
                title_found = true;
            }
        }
    }
    assert!(title_found);

    // 2. Render CollectParameter Stage
    let mut state_param = CommandPaletteState::new();
    state_param.stage = PaletteStage::CollectParameter(ParameterCollectionState::new(
        brain_tui::ui::command::CHANGE_THEME,
    ));
    terminal
        .draw(|f| {
            palette::draw(f, area, &state_param, &theme);
        })
        .unwrap();

    // 3. Render Slash Completion Popup
    let mut completion_state = SlashCompletionState::new();
    completion_state.visible = true;
    completion_state.query = "/t".to_string();
    terminal
        .draw(|f| {
            completion::draw(
                f,
                ratatui::layout::Rect::new(0, 0, 40, 10),
                &completion_state,
                &theme,
            );
        })
        .unwrap();
}
