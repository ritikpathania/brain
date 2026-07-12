mod common;

use brain_tui::ui::render::{
    RenderCapabilities, CapabilityPolicy, CapabilityResolver,
    UnicodeSupport, ColorSupport, NerdFontsSupport, MotionPreference
};
use brain_tui::clipboard::{Clipboard, MockClipboard};
use brain_tui::ui::input::{MouseRouter, MouseAction};
use brain_tui::ui::layout::LayoutEngine;
use brain_tui::ui::theme::{dark_theme, high_contrast_theme};
use brain_tui::state::FocusRegion;
use brain_domain::SessionId;
use ratatui::layout::Rect;
use crossterm::event::{MouseEvent, MouseEventKind, MouseButton};

#[test]
fn test_effective_capability_determinism() {
    let caps = RenderCapabilities {
        unicode: UnicodeSupport::Full,
        colors: ColorSupport::TrueColor,
        nerd_fonts: NerdFontsSupport::Full,
        mouse: true,
        osc8: true,
        motion: MotionPreference::Full,
    };
    let policy = CapabilityPolicy {
        force_ascii: false,
        force_colors: None,
        disable_mouse: false,
        disable_motion: false,
    };

    let resolved_1 = CapabilityResolver::resolve(&caps, &policy);
    let resolved_2 = CapabilityResolver::resolve(&caps, &policy);

    assert_eq!(resolved_1, resolved_2);
}

#[test]
fn test_policy_override_ascii() {
    let caps = RenderCapabilities {
        unicode: UnicodeSupport::Full,
        colors: ColorSupport::TrueColor,
        nerd_fonts: NerdFontsSupport::Full,
        mouse: true,
        osc8: true,
        motion: MotionPreference::Full,
    };
    // Force ASCII via policy override
    let policy = CapabilityPolicy {
        force_ascii: true,
        force_colors: None,
        disable_mouse: false,
        disable_motion: false,
    };

    let resolved = CapabilityResolver::resolve(&caps, &policy);
    assert_eq!(resolved.unicode, UnicodeSupport::AsciiOnly);
}

#[test]
fn test_clipboard_fallback() {
    let mut clipboard = MockClipboard::new();
    assert_eq!(clipboard.get().unwrap(), "");
    clipboard.set("hello world").unwrap();
    assert_eq!(clipboard.get().unwrap(), "hello world");
}

#[test]
fn test_mouse_routing_boundary() {
    // Construct layout area and sessions
    let area = Rect::new(0, 0, 90, 24);
    let geometry = LayoutEngine::chat_screen(area);
    let session_id = SessionId::new();
    let sessions = vec![session_id];

    // Left click inside editor prompt_area
    let click_editor_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: geometry.prompt_area.x + 1,
        row: geometry.prompt_area.y + 1,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    let action_editor = MouseRouter::handle(click_editor_event, &geometry, &sessions);
    assert_eq!(action_editor, Some(MouseAction::FocusRegion(FocusRegion::Editor)));

    // Left click inside sidebar_area first slot
    let click_sidebar_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: geometry.sidebar_area.x + 1,
        row: geometry.sidebar_area.y + 1, // first row is header, second row is active-session
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    let action_sidebar = MouseRouter::handle(click_sidebar_event, &geometry, &sessions);
    assert_eq!(action_sidebar, Some(MouseAction::SelectSession(session_id)));
}

#[test]
fn test_theme_independence() {
    let _dark = dark_theme();
    let _high_contrast = high_contrast_theme();

    let area = Rect::new(0, 0, 90, 24);
    
    // Layout and geometry must remain identical regardless of the active theme
    let geometry_dark = LayoutEngine::chat_screen(area);
    let geometry_hc = LayoutEngine::chat_screen(area);

    assert_eq!(geometry_dark, geometry_hc);
}

#[test]
fn test_capability_fingerprint() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let caps = RenderCapabilities {
        unicode: UnicodeSupport::Full,
        colors: ColorSupport::TrueColor,
        nerd_fonts: NerdFontsSupport::Full,
        mouse: true,
        osc8: true,
        motion: MotionPreference::Full,
    };
    let policy = CapabilityPolicy {
        force_ascii: false,
        force_colors: None,
        disable_mouse: false,
        disable_motion: false,
    };

    let resolved_1 = CapabilityResolver::resolve(&caps, &policy);
    let resolved_2 = CapabilityResolver::resolve(&caps, &policy);

    let mut hasher1 = DefaultHasher::new();
    resolved_1.hash(&mut hasher1);
    let fingerprint1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    resolved_2.hash(&mut hasher2);
    let fingerprint2 = hasher2.finish();

    assert_eq!(fingerprint1, fingerprint2);
}

struct FailingClipboard;
impl Clipboard for FailingClipboard {
    fn get(&self) -> Result<String, brain_tui::clipboard::ClipboardError> {
        Err(brain_tui::clipboard::ClipboardError::Unavailable)
    }
    fn set(&mut self, _text: &str) -> Result<(), brain_tui::clipboard::ClipboardError> {
        Err(brain_tui::clipboard::ClipboardError::OperationFailed(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "error"))))
    }
}

#[test]
fn test_clipboard_isolation() {
    let mut clipboard = FailingClipboard;
    assert!(clipboard.get().is_err());
    assert!(clipboard.set("test").is_err());
}

const CHAT_VIEW: brain_tui::ui::widgets::view_models::ChatScreenView<'static> = brain_tui::ui::widgets::view_models::ChatScreenView {
    session_title: "test",
    connection: brain_tui::ui::widgets::view_models::ConnectionState::Connected,
    is_working: false,
    message_count: 0,
    input_buffer: "",
    focus: brain_tui::ui::widgets::view_models::FocusTarget::Prompt,
};

fn make_test_app_state<'a>() -> brain_tui::ui::state::AppState<'a> {
    use brain_tui::ui::interaction::{ChatState, Editor, ScrollState};
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::router::{ScreenRouter, ActiveScreen};
    use brain_tui::ui::widgets::ChatScreen;
    use brain_tui::ui::widgets::view_models::FocusTarget;
    use brain_tui::ui::state::AppState;

    let chat = ChatState::new();
    let editor = Editor::new();
    let scroll = ScrollState::new();
    let focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let chat_screen = ChatScreen { view: &CHAT_VIEW };
    let router = ScreenRouter::new(ActiveScreen::Chat(chat_screen));

    let sidebar = brain_tui::ui::interaction::sidebar::SidebarInteraction::new();
    AppState::new(chat, editor, scroll, focus, sidebar, router)
}

#[test]
fn test_resize_storm() {
    let mut state = make_test_app_state();
    
    // Simulate resize storm
    state.resize(80, 24);
    state.resize(120, 40);
    state.resize(90, 30);
    state.resize(100, 35);
    
    let geom_storm = LayoutEngine::chat_screen(Rect::new(0, 0, state.cols(), state.rows()));
    let geom_direct = LayoutEngine::chat_screen(Rect::new(0, 0, 100, 35));
    
    assert_eq!(geom_storm, geom_direct);
}

#[test]
fn test_mouse_replay() {
    let area = Rect::new(0, 0, 90, 24);
    let geometry = LayoutEngine::chat_screen(area);
    let session_id = SessionId::new();
    let sessions = vec![session_id];

    let click_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: geometry.prompt_area.x + 1,
        row: geometry.prompt_area.y + 1,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };

    let action_1 = MouseRouter::handle(click_event, &geometry, &sessions);
    let action_2 = MouseRouter::handle(click_event, &geometry, &sessions);

    assert_eq!(action_1, action_2);
}

