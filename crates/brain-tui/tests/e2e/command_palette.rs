//! E2E Command Palette behavioral tests.
//!
//! Verifies slash command completion, Escape dismissal caching, Tab form-advance in CollectParameter,
//! and focus transitions.

use brain_tui::ui::command::completion::SlashCompletionEngine;
use brain_tui::ui::command::completion::SlashCompletionState;
use brain_tui::ui::command::palette::{
    CommandPaletteState, PaletteStage, ParameterCollectionState,
};
use brain_tui::ui::command::CommandId;
use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::input::{Command, InputAction};
use brain_tui::ui::interaction::dispatcher::{Dispatcher, InteractionContext};
use brain_tui::ui::interaction::editor::Editor;
use brain_tui::ui::interaction::scroll::ScrollState;
use brain_tui::ui::interaction::sidebar::SidebarInteraction;
use brain_tui::ui::widgets::view_models::FocusTarget;

struct DummyLookup;
impl brain_tui::ui::interaction::sidebar::SessionLookup for DummyLookup {
    fn title(&self, _id: brain_domain::SessionId) -> Option<&str> {
        None
    }
}

#[test]
fn test_slash_completion_popup_and_escape_dismissal() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    focus.set_focus(FocusTarget::Prompt);

    // Type "/theme" into prompt
    for c in "/theme".chars() {
        editor.insert(c);
    }

    // Run dispatcher to simulate post-processing update
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

    Dispatcher::dispatch(InputAction::None, &mut ctx);

    // Popup should be visible
    assert!(
        ctx.slash_completion.visible,
        "Slash completion popup should be visible for '/theme'"
    );

    // ── Act: Press Escape to dismiss ─────────────────────────────────────────
    let res = Dispatcher::dispatch(InputAction::Command(Command::Escape), &mut ctx);

    // ── Assert ───────────────────────────────────────────────────────────────
    assert!(
        !ctx.slash_completion.visible,
        "Escape MUST hide the slash completion popup"
    );
    assert_eq!(
        ctx.slash_completion.dismissed_query.as_deref(),
        Some("/theme"),
        "Escape MUST cache the exact dismissed query string"
    );
    assert!(
        res.needs_render,
        "Escape dismissal MUST request a render frame"
    );

    // Act: Type another character making it "/themes" via dispatcher -> dismissed_query should clear
    use brain_tui::ui::input::TextInput;
    Dispatcher::dispatch(InputAction::Text(TextInput::Char('s')), &mut ctx);

    // Assert: re-armed for new text
    assert_eq!(
        ctx.slash_completion.dismissed_query, None,
        "Typing new text MUST clear the dismissed_query cache"
    );
}

#[test]
fn test_collect_parameter_tab_advance_journey() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::new();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;

    focus.set_focus(FocusTarget::CommandPalette);
    command_palette.open = true;
    command_palette.stage =
        PaletteStage::CollectParameter(ParameterCollectionState::new(CommandId("theme")));

    // Type parameter value into palette editor
    for c in "dark".chars() {
        command_palette.editor.insert(c);
    }

    // ── Act: Press Tab (FocusNext) to advance field ─────────────────────────
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

    let res = Dispatcher::dispatch(InputAction::Command(Command::FocusNext), &mut ctx);

    // ── Assert ───────────────────────────────────────────────────────────────
    assert!(
        res.needs_render,
        "Tab in CollectParameter MUST return a render signal for the next field prompt"
    );
}

#[test]
fn test_slash_completion_engine_matches() {
    let matches: Vec<_> = SlashCompletionEngine::matches("/t").collect();
    assert!(
        !matches.is_empty(),
        "SlashCompletionEngine should find matches for prefix '/t'"
    );
}
