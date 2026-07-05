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
