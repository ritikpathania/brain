use brain_tui::ui::command::{
    CommandRegistry, CommandPolicy, CommandAvailabilityContext, Availability, AvailabilityReason,
    CHANGE_THEME, RENAME_SESSION, SWITCH_MODEL,
};


#[test]
fn test_command_registry_lookups() {
    // Lookup by ID
    let theme_cmd = CommandRegistry::find_by_id(CHANGE_THEME).expect("Should find Change Theme command");
    assert_eq!(theme_cmd.title, "Change Theme");
    assert_eq!(theme_cmd.aliases, &["theme"]);

    // Lookup by name
    let rename_cmd = CommandRegistry::find_by_name_or_alias("rename").expect("Should find Rename Session command");
    assert_eq!(rename_cmd.id, RENAME_SESSION);

    // Lookup by alias (case-insensitive check)
    let switch_model_cmd = CommandRegistry::find_by_name_or_alias("MODEL").expect("Should find Switch Model command");
    assert_eq!(switch_model_cmd.id, SWITCH_MODEL);

    // Non-existent command
    assert!(CommandRegistry::find_by_id(brain_tui::ui::command::CommandId("nonexistent")).is_none());
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
fn test_slash_completion_dispatch_trapping() {
    use brain_tui::ui::interaction::{Editor, ScrollState, Dispatcher, InteractionContext, SidebarInteraction};
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::widgets::view_models::FocusTarget;
    use brain_tui::ui::input::{InputAction, Command, TextInput};
    use brain_tui::ui::command::completion::SlashCompletionState;
    use brain_domain::SessionId;

    struct DummyLookup;
    impl brain_tui::ui::interaction::sidebar::SessionLookup for DummyLookup {
        fn title(&self, _id: SessionId) -> Option<&str> { None }
    }


    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let visible_ids = vec![];
    let lookup = DummyLookup;

    // 1. Type '/' -> Should show autocomplete
    let _ = Dispatcher::dispatch(
        InputAction::Text(TextInput::Char('/')),
        &mut InteractionContext {
            editor: &mut editor,
            scroll: &mut scroll,
            focus: &mut focus,
            sidebar: &mut sidebar,
            slash_completion: &mut slash_completion,
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
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
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
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
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
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
            visible_ids: &visible_ids,
            lookup: &lookup,
        }
    );
    assert!(!slash_completion.visible);
    assert_eq!(editor.text(), "/model ");
}



