use brain_domain::SessionId;
use brain_tui::ui::command::completion::SlashCompletionState;
use brain_tui::ui::command::palette::CommandPaletteState;
use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::input::{Command, InputAction};
use brain_tui::ui::interaction::dispatcher::{Dispatcher, InteractionContext, UiEvent};
use brain_tui::ui::interaction::editor::Editor;
use brain_tui::ui::interaction::scroll::ScrollState;
use brain_tui::ui::interaction::sidebar::{SessionLookup, SidebarInteraction};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::palette::CommandPaletteWidget;
use brain_tui::ui::widgets::view_models::FocusTarget;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;

struct DummyLookup;
impl SessionLookup for DummyLookup {
    fn title(&self, _id: SessionId) -> Option<&str> {
        None
    }
}

#[test]
fn test_command_palette_3_column_reconstruction_rendering() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();
    let mut state = CommandPaletteState::new();
    state.open = true;

    let area = Rect::new(0, 0, 80, 10);

    terminal
        .draw(|f| {
            let widget = CommandPaletteWidget::new(&state, &theme);
            widget.draw(f, area);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    let mut full_text = String::new();
    for y in 0..10 {
        let line = (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>();
        full_text.push_str(&line);
        full_text.push('\n');
    }

    // 1. Assert floating dropdown layout: MUST NOT contain old modal border title " Command Palette "
    assert!(
        !full_text.contains("Command Palette"),
        "Command palette should be floating dropdown without modal title border. Buffer:\n{}",
        full_text
    );

    // 2. Assert command items exist
    assert!(
        full_text.contains("/session") || full_text.contains("/help") || full_text.contains("/search"),
        "Command palette layout must render commands. Rendered buffer:\n{}",
        full_text
    );

    // 3. Assert Column 2 category string ("command ·" or "skill ·") exists
    assert!(
        full_text.contains("command ·") || full_text.contains("skill ·"),
        "Command palette Column 2 must contain 'command ·' or 'skill ·'. Rendered buffer:\n{}",
        full_text
    );

    // 4. Find row with "command ·" or "skill ·" and check Column 2 fg color (#AFB9F9 -> Rgb(177, 185, 249))
    let mut found_cat_color = false;
    let mut found_selected_bg = false;

    for y in 0..10 {
        for x in 0..70 {
            let cell = buf.get(x, y);
            if cell.symbol() == "c" && x + 8 < 80 {
                let s: String = (x..x + 9).map(|col| buf.get(col, y).symbol()).collect();
                if s == "command ·" {
                    if cell.fg == Color::Rgb(177, 185, 249) {
                        found_cat_color = true;
                    }
                    if cell.bg == Color::Rgb(38, 79, 120) {
                        found_selected_bg = true;
                    }
                }
            }
        }
    }

    assert!(
        found_cat_color,
        "Column 2 ('command ·') must be styled with fg Color::Rgb(177, 185, 249) (#AFB9F9)."
    );

    assert!(
        found_selected_bg,
        "Selected row must be rendered with bg Color::Rgb(38, 79, 120) (#264F78)."
    );
}

#[test]
fn test_command_palette_key_navigation_and_dispatch() {
    let mut editor = Editor::new();
    let mut scroll = ScrollState::new();
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let mut sidebar = SidebarInteraction::new();
    let mut slash_completion = SlashCompletionState::default();
    let mut command_palette = CommandPaletteState::new();
    let mut pending_approvals = Vec::new();
    let lookup = DummyLookup;
    let visible_ids = Vec::new();
    let sessions = Vec::new();
    let active_messages = Vec::new();

    // Open palette and set focus
    command_palette.open = true;
    focus.set_focus(FocusTarget::CommandPalette);
    command_palette.selected_index = 0;

    let mut ctx = InteractionContext {
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
        sessions: &sessions,
        active_messages: &active_messages,
    };

    // 1. Test Down Arrow moves selection down
    let res_down = Dispatcher::dispatch(InputAction::Command(Command::ScrollDown), &mut ctx);
    assert!(res_down.needs_render);
    assert_eq!(ctx.command_palette.selected_index, 1, "Down arrow should move selection to index 1");

    // 2. Test Up Arrow moves selection up
    let res_up = Dispatcher::dispatch(InputAction::Command(Command::ScrollUp), &mut ctx);
    assert!(res_up.needs_render);
    assert_eq!(ctx.command_palette.selected_index, 0, "Up arrow should move selection back to index 0");

    // 3. Test Enter key dispatches command and closes palette
    let res_submit = Dispatcher::dispatch(InputAction::Command(Command::Submit), &mut ctx);
    assert!(res_submit.needs_render);
    assert!(!ctx.command_palette.open, "Enter key should close command palette");
    assert!(
        matches!(res_submit.ui_event, Some(UiEvent::Command(_))),
        "Enter key should dispatch a Command UiEvent"
    );

    // 4. Test Esc key closes palette
    ctx.command_palette.open = true;
    ctx.focus.set_focus(FocusTarget::CommandPalette);
    let res_esc = Dispatcher::dispatch(InputAction::Command(Command::Escape), &mut ctx);
    assert!(res_esc.needs_render);
    assert!(!ctx.command_palette.open, "Esc key should close command palette");
}
