use brain_domain::SessionId;
use brain_tui::ui::interaction::sidebar::{
    SidebarInteraction, SidebarMode, SessionFilter, ParsedQuery, SessionLookup, SidebarEvent
};

struct MockLookup;
impl SessionLookup for MockLookup {
    fn title(&self, _id: SessionId) -> Option<&str> {
        Some("Brain Architecture RFC")
    }
}

#[test]
fn test_search_and_rename_transitions() {
    let mut interaction = SidebarInteraction::new();
    assert_eq!(interaction.mode, SidebarMode::Browse);
    assert_eq!(interaction.browse.filter, SessionFilter::Active);
    assert!(!interaction.search.active);

    interaction.enter_search();
    assert!(interaction.search.active);

    interaction.leave_search(true);
    assert!(!interaction.search.active);
}

#[test]
fn test_parsed_query_matching() {
    let mut query = ParsedQuery::default();
    assert!(query.is_empty());
    
    // Empty query matches anything
    assert!(query.matches("Brain Architecture RFC"));

    // Update query with terms
    query.update("Brain RFC");
    assert!(!query.is_empty());
    assert_eq!(query.terms, vec!["brain".to_string(), "rfc".to_string()]);

    // Matches case-insensitively and allows terms in any order / substring
    assert!(query.matches("Brain Architecture RFC"));
    assert!(query.matches("rfc for brain"));
    assert!(!query.matches("Brain Architecture")); // missing "rfc"

    // Clear query
    query.clear();
    assert!(query.is_empty());
    assert!(query.matches("Brain Architecture"));
}

#[test]
fn test_rename_flow_initialization_and_leaving() {
    let mut interaction = SidebarInteraction::new();
    assert_eq!(interaction.mode, SidebarMode::Browse);

    // Enter rename mode with a title
    interaction.enter_rename("My Session");
    assert_eq!(interaction.mode, SidebarMode::Rename);
    assert_eq!(interaction.rename.editor.text(), "My Session");
    assert_eq!(interaction.rename.editor.cursor().visual_col, 10);

    // Leave rename mode
    interaction.leave_rename();
    assert_eq!(interaction.mode, SidebarMode::Browse);
    assert_eq!(interaction.rename.editor.text(), "");
}

#[test]
fn test_search_clearing() {
    let mut interaction = SidebarInteraction::new();
    
    interaction.enter_search();
    interaction.search.editor.insert_char('f');
    interaction.search.editor.insert_char('o');
    interaction.search.editor.insert_char('o');
    interaction.search.parsed.update(interaction.search.editor.text());

    assert_eq!(interaction.search.editor.text(), "foo");
    assert!(!interaction.search.parsed.is_empty());

    // Leave search with clear = true
    interaction.leave_search(true);
    assert!(!interaction.search.active);
    assert_eq!(interaction.search.editor.text(), "");
    assert!(interaction.search.parsed.is_empty());

    // Leave search with clear = false
    interaction.enter_search();
    interaction.search.editor.insert_char('b');
    interaction.search.editor.insert_char('a');
    interaction.search.editor.insert_char('r');
    interaction.search.parsed.update(interaction.search.editor.text());
    
    interaction.leave_search(false);
    assert!(!interaction.search.active);
    assert_eq!(interaction.search.editor.text(), "bar");
    assert!(!interaction.search.parsed.is_empty());
}

#[test]
fn test_mock_lookup() {
    let lookup = MockLookup;
    let id = SessionId::new();
    assert_eq!(lookup.title(id), Some("Brain Architecture RFC"));
}

#[test]
fn test_editor_utf8_interactions() {
    use brain_tui::ui::interaction::editor::Editor;
    let mut editor = Editor::new();
    
    // Insert multi-byte characters
    editor.insert_char('あ');
    editor.insert_char('い');
    editor.insert_char('う');
    
    assert_eq!(editor.text(), "あいう");
    assert_eq!(editor.cursor().visual_col, 3);
    assert_eq!(editor.cursor().byte_index, 9); // 'あ', 'い', 'う' each take 3 bytes
    
    // Move left
    editor.move_cursor_left();
    assert_eq!(editor.cursor().visual_col, 2);
    assert_eq!(editor.cursor().byte_index, 6);
    
    // Move left again
    editor.move_cursor_left();
    assert_eq!(editor.cursor().visual_col, 1);
    assert_eq!(editor.cursor().byte_index, 3);
    
    // Move right
    editor.move_cursor_right();
    assert_eq!(editor.cursor().visual_col, 2);
    assert_eq!(editor.cursor().byte_index, 6);
    
    // Backspace
    editor.backspace();
    assert_eq!(editor.text(), "あう");
    assert_eq!(editor.cursor().visual_col, 1);
    assert_eq!(editor.cursor().byte_index, 3);
    
    // Delete (removes 'う')
    editor.delete();
    assert_eq!(editor.text(), "あ");
    assert_eq!(editor.cursor().visual_col, 1);
    assert_eq!(editor.cursor().byte_index, 3);
}

#[test]
fn test_sidebar_key_events_emission() {
    use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, KeyEventKind, KeyEventState};

    let mut interaction = SidebarInteraction::new();
    let session_id = SessionId::new();
    let visible_ids = vec![session_id];
    interaction.browse.selected = Some(session_id);

    struct Lookup;
    impl SessionLookup for Lookup {
        fn title(&self, _id: SessionId) -> Option<&str> { Some("Test Session") }
    }
    let lookup = Lookup;

    // Press 'c' to archive
    let key_c = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    let (handled, event) = interaction.handle_key(key_c, &visible_ids, &lookup);
    assert!(handled);
    assert_eq!(event, Some(SidebarEvent::Archive(session_id)));
}

#[test]
fn test_sidebar_rendering_modes() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use brain_tui::ui::widgets::sidebar::{self, SidebarView};
    use brain_tui::ui::interaction::sidebar::{SidebarMode, SessionFilter};
    use brain_tui::ui::theme::Theme;
    use brain_tui::state::SessionViewModel;
    use std::time::SystemTime;

    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();

    let sessions = vec![
        SessionViewModel {
            id: SessionId::new(),
            title: "Active Session 1".to_string(),
            updated_at: SystemTime::now(),
            active: true,
            preview: None,
            pinned: true,
            archived: false,
        },
        SessionViewModel {
            id: SessionId::new(),
            title: "Archived Session 2".to_string(),
            updated_at: SystemTime::now(),
            active: false,
            preview: None,
            pinned: false,
            archived: true,
        },
    ];

    let view = SidebarView {
        sessions: &sessions,
        selected_idx: Some(0),
        has_focus: true,
        filter: SessionFilter::Active,
        mode: SidebarMode::Browse,
        search_active: true,
        search_query: "Active",
        search_cursor: 6,
        rename_query: "Renaming...",
        rename_cursor: 11,
    };

    terminal.draw(|f| {
        let area = f.size();
        sidebar::draw(f, area, &view, &theme);
    }).unwrap();

    let buffer = terminal.backend().buffer();
    assert!(buffer.area.width > 0);
}

#[test]
fn test_sidebar_cursor_formatting_and_slicing() {
    use brain_tui::ui::widgets::sidebar::{format_with_cursor, slice_text_viewport};

    // Test format_with_cursor
    assert_eq!(format_with_cursor("abc", 0, "|"), "|abc");
    assert_eq!(format_with_cursor("abc", 1, "|"), "a|bc");
    assert_eq!(format_with_cursor("abc", 3, "|"), "abc|");
    assert_eq!(format_with_cursor("abc", 10, "|"), "abc|"); // clamp safe

    // Test slice_text_viewport when width fits
    let (sliced, new_cursor) = slice_text_viewport("hello", 3, 10);
    assert_eq!(sliced, "hello");
    assert_eq!(new_cursor, 3);

    // Test slice_text_viewport when width is exceeded
    // text: "abcdef", cursor at 5 ('f'), max_width: 3.
    // should slide viewport to only show "def" and place cursor at 2
    let (sliced, new_cursor) = slice_text_viewport("abcdef", 5, 3);
    assert_eq!(sliced, "def");
    assert_eq!(new_cursor, 2);
}

#[tokio::test]
async fn test_sidebar_event_loop_orchestration() {
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::mpsc;
    use brain_tui::ui::interaction::{
        Editor, ScrollState, ChatState, UiEvent
    };
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::widgets::view_models::{FocusTarget, ChatScreenView, ConnectionState};
    use brain_tui::ui::widgets::ChatScreen;
    use brain_tui::ui::router::{ScreenRouter, ActiveScreen};
    use brain_tui::ui::state::AppState;
    use brain_tui::ui::protocol::{BackendCommand, BackendEvent};
    use brain_tui::ui::scheduler::MockRenderScheduler;
    use brain_tui::ui::application::{Application, UiEventSource, DaemonClient, ApplicationError};
    use brain_tui::ui::interaction::sidebar::{SidebarInteraction, SidebarEvent};

    const CHAT_VIEW: ChatScreenView<'static> = ChatScreenView {
        session_title: "test",
        connection: ConnectionState::Connected,
        is_working: false,
        message_count: 0,
        input_buffer: "",
        focus: FocusTarget::Prompt,
    };

    struct TestUiSource {
        rx: mpsc::Receiver<UiEvent>,
    }
    #[async_trait::async_trait]
    impl UiEventSource for TestUiSource {
        async fn next_event(&mut self) -> Option<UiEvent> {
            self.rx.recv().await
        }
    }

    struct TestDaemonClient {
        commands: Arc<StdMutex<Vec<BackendCommand>>>,
    }
    #[async_trait::async_trait]
    impl DaemonClient for TestDaemonClient {
        async fn send(&self, command: BackendCommand) -> Result<(), ApplicationError> {
            self.commands.lock().unwrap().push(command);
            Ok(())
        }
        async fn next_event(&self) -> Option<BackendEvent> {
            std::future::pending::<()>().await;
            None
        }
    }

    let chat = ChatState::new();
    let editor = Editor::new();
    let scroll = ScrollState::new();
    let focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let chat_screen = ChatScreen { view: &CHAT_VIEW };
    let router = ScreenRouter::new(ActiveScreen::Chat(chat_screen));
    let sidebar = SidebarInteraction::new();
    let state = AppState::new(chat, editor, scroll, focus, sidebar, router);

    let scheduler = MockRenderScheduler::new();
    let commands = Arc::new(StdMutex::new(Vec::new()));
    let client = TestDaemonClient {
        commands: commands.clone(),
    };

    let mut app = Application::new(state, scheduler, client);

    let (ui_tx, ui_rx) = mpsc::channel(10);
    let ui_source = TestUiSource { rx: ui_rx };

    let cancellation = app.cancellation().clone();
    let handle = tokio::spawn(async move {
        app.run(ui_source).await
    });

    // Send a SidebarEvent::Rename via TUI channel
    let test_id = SessionId::new();
    ui_tx.send(UiEvent::Sidebar(SidebarEvent::Rename(test_id, Some("New Title".to_string())))).await.unwrap();

    // Give it a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify command was sent to client
    let cmds = commands.lock().unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], BackendCommand::RenameSession { session_id: test_id, title: Some("New Title".to_string()) });

    cancellation.cancel();
    let _ = handle.await.unwrap();
}

#[tokio::test]
async fn test_sidebar_optimistic_state_updates() {
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use brain_tui::ui::interaction::{
        Editor, ScrollState, ChatState, UiEvent
    };
    use brain_tui::ui::focus::{FocusManager, FocusProfile};
    use brain_tui::ui::widgets::view_models::{FocusTarget, ChatScreenView, ConnectionState};
    use brain_tui::ui::widgets::ChatScreen;
    use brain_tui::ui::router::{ScreenRouter, ActiveScreen};
    use brain_tui::ui::state::AppState;
    use brain_tui::ui::protocol::BackendCommand;
    use brain_tui::ui::scheduler::MockRenderScheduler;
    use brain_tui::ui::application::{Application, DaemonClient, ApplicationError};
    use brain_tui::ui::interaction::sidebar::{SidebarInteraction, SidebarEvent};
    use brain_tui::client::SessionSummary;

    const CHAT_VIEW: ChatScreenView<'static> = ChatScreenView {
        session_title: "test",
        connection: ConnectionState::Connected,
        is_working: false,
        message_count: 0,
        input_buffer: "",
        focus: FocusTarget::Prompt,
    };

    struct TestDaemonClient {
        commands: Arc<StdMutex<Vec<BackendCommand>>>,
    }
    #[async_trait::async_trait]
    impl DaemonClient for TestDaemonClient {
        async fn send(&self, command: BackendCommand) -> Result<(), ApplicationError> {
            self.commands.lock().unwrap().push(command);
            Ok(())
        }
        async fn next_event(&self) -> Option<brain_tui::ui::protocol::BackendEvent> {
            None
        }
    }

    let chat = ChatState::new();
    let editor = Editor::new();
    let scroll = ScrollState::new();
    let focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let chat_screen = ChatScreen { view: &CHAT_VIEW };
    let router = ScreenRouter::new(ActiveScreen::Chat(chat_screen));
    let sidebar = SidebarInteraction::new();
    let mut state = AppState::new(chat, editor, scroll, focus, sidebar, router);

    // Seed state with mock sessions
    let test_id1 = SessionId::new();
    let test_id2 = SessionId::new();
    state.set_sessions(vec![
        SessionSummary {
            id: test_id1,
            title: "Original Title".to_string(),
            updated_at: std::time::SystemTime::now(),
            pinned: false,
            archived: false,
        },
        SessionSummary {
            id: test_id2,
            title: "Second Session".to_string(),
            updated_at: std::time::SystemTime::now(),
            pinned: true,
            archived: false,
        },
    ]);

    let scheduler = MockRenderScheduler::new();
    let commands = Arc::new(StdMutex::new(Vec::new()));
    let client = TestDaemonClient {
        commands: commands.clone(),
    };

    let mut app = Application::new(state, scheduler, client);

    // 1. Test optimistic rename
    app.handle_ui_event(UiEvent::Sidebar(SidebarEvent::Rename(test_id1, Some("New Title".to_string())))).await.unwrap();

    // 2. Test optimistic toggle pin
    app.handle_ui_event(UiEvent::Sidebar(SidebarEvent::TogglePin(test_id2))).await.unwrap();

    // 3. Test optimistic archive
    app.handle_ui_event(UiEvent::Sidebar(SidebarEvent::Archive(test_id1))).await.unwrap();

    // 4. Test optimistic delete
    app.handle_ui_event(UiEvent::Sidebar(SidebarEvent::Delete(test_id2))).await.unwrap();

    // Assert rename and archive was applied optimistically
    let s1 = app.state().sessions().iter().find(|x| x.id == test_id1).unwrap();
    assert_eq!(s1.title, "New Title");
    assert!(s1.archived);

    // Assert delete was applied (s2 should be missing)
    assert!(app.state().sessions().iter().find(|x| x.id == test_id2).is_none());

    // Verify commands were sent to client
    let cmds = commands.lock().unwrap();
    assert_eq!(cmds.len(), 4);
}

