use brain_tui::ui::application::{Application, ApplicationError, DaemonClient, UiEventSource};
use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::interaction::{ChatState, Editor, MessageId, ScrollState, UiEvent};
use brain_tui::ui::protocol::{BackendCommand, BackendEvent, RequestId};
use brain_tui::ui::router::{ActiveScreen, ScreenRouter};
use brain_tui::ui::scheduler::{MockRenderScheduler, RenderInvalidation, RenderReason};
use brain_tui::ui::state::AppState;
use brain_tui::ui::widgets::view_models::{ChatScreenView, ConnectionState, FocusTarget};
use brain_tui::ui::widgets::ChatScreen;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;

const CHAT_VIEW: ChatScreenView<'static> = ChatScreenView {
    session_title: "test",
    connection: ConnectionState::Connected,
    is_working: false,
    message_count: 0,
    input_buffer: "",
    focus: FocusTarget::Prompt,
};

fn make_test_app_state<'a>() -> AppState<'a> {
    let chat = ChatState::new();
    let editor = Editor::new();
    let scroll = ScrollState::new();
    let focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    let chat_screen = ChatScreen { view: &CHAT_VIEW };
    let router = ScreenRouter::new(ActiveScreen::Chat(chat_screen));
    let sidebar = brain_tui::ui::interaction::sidebar::SidebarInteraction::new();
    AppState::new(chat, editor, scroll, focus, sidebar, router)
}

struct ChannelUiEventSource {
    rx: mpsc::Receiver<UiEvent>,
}

#[async_trait::async_trait]
impl UiEventSource for ChannelUiEventSource {
    async fn next_event(&mut self) -> Option<UiEvent> {
        self.rx.recv().await
    }
}

struct QueueUiEventSource {
    events: Vec<UiEvent>,
}

#[async_trait::async_trait]
impl UiEventSource for QueueUiEventSource {
    async fn next_event(&mut self) -> Option<UiEvent> {
        if self.events.is_empty() {
            None
        } else {
            Some(self.events.remove(0))
        }
    }
}

struct MockDaemonClient {
    commands: Arc<StdMutex<Vec<BackendCommand>>>,
    event_rx: Arc<TokioMutex<mpsc::Receiver<BackendEvent>>>,
}

#[async_trait::async_trait]
impl DaemonClient for MockDaemonClient {
    async fn send(&self, command: BackendCommand) -> Result<(), ApplicationError> {
        self.commands.lock().unwrap().push(command);
        Ok(())
    }

    async fn next_event(&self) -> Option<BackendEvent> {
        self.event_rx.lock().await.recv().await
    }
}

#[tokio::test]
async fn test_application_loop_orchestration() {
    let state = make_test_app_state();
    let scheduler = MockRenderScheduler::new();
    let commands = Arc::new(StdMutex::new(Vec::new()));
    let (event_tx, event_rx) = mpsc::channel(10);
    let client = MockDaemonClient {
        commands: commands.clone(),
        event_rx: Arc::new(TokioMutex::new(event_rx)),
    };

    let mut app = Application::new(state, scheduler, client);

    let (ui_tx, ui_rx) = mpsc::channel(10);
    let ui_source = ChannelUiEventSource { rx: ui_rx };

    // Spin up the app run loop in a background task
    let cancellation = app.cancellation().clone();
    let handle = tokio::spawn(async move { app.run(ui_source).await });

    // 1. Submit prompt text
    ui_tx
        .send(UiEvent::SubmitPrompt("ping".to_string()))
        .await
        .unwrap();

    // Verify command sent
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    {
        let cmds = commands.lock().unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            cmds[0],
            BackendCommand::SubmitPrompt {
                request: RequestId::new(1),
                message: MessageId(2),
                text: "ping".to_string(),
            }
        );
    }

    // 2. Stream back event token
    event_tx
        .send(BackendEvent::Token {
            message: MessageId(2),
            sequence: 1,
            text: "pong".to_string(),
        })
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Shut down gracefully
    cancellation.cancel();
    let res = handle.await.unwrap();
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_transport_independence() {
    // Verifies that a Queue event source produces identical state output to a Channel event source.
    let commands = Arc::new(StdMutex::new(Vec::new()));
    let (_event_tx, event_rx) = mpsc::channel(10);
    let client = MockDaemonClient {
        commands: commands.clone(),
        event_rx: Arc::new(TokioMutex::new(event_rx)),
    };

    // Run using QueueUiEventSource
    let mut app_queue = Application::new(make_test_app_state(), MockRenderScheduler::new(), client);
    let queue_source = QueueUiEventSource {
        events: vec![UiEvent::SubmitPrompt("independence".to_string())],
    };
    app_queue.run(queue_source).await.unwrap();

    let queue_text = app_queue.state().chat().messages()[0]
        .text
        .raw()
        .to_string();

    // Run using ChannelUiEventSource
    let (ui_tx, ui_rx) = mpsc::channel(10);
    let channel_source = ChannelUiEventSource { rx: ui_rx };
    let commands2 = Arc::new(StdMutex::new(Vec::new()));
    let (_event_tx2, event_rx2) = mpsc::channel(10);
    let client2 = MockDaemonClient {
        commands: commands2.clone(),
        event_rx: Arc::new(TokioMutex::new(event_rx2)),
    };
    let mut app_channel =
        Application::new(make_test_app_state(), MockRenderScheduler::new(), client2);

    ui_tx
        .send(UiEvent::SubmitPrompt("independence".to_string()))
        .await
        .unwrap();
    drop(ui_tx); // Closes UI source to terminate run loop gracefully

    app_channel.run(channel_source).await.unwrap();
    let channel_text = app_channel.state().chat().messages()[0]
        .text
        .raw()
        .to_string();

    assert_eq!(queue_text, channel_text);
    assert_eq!(queue_text, "independence");
}

#[tokio::test]
async fn test_request_allocator_monotonicity() {
    let state = make_test_app_state();
    let scheduler = MockRenderScheduler::new();
    let commands = Arc::new(StdMutex::new(Vec::new()));
    let (_event_tx, event_rx) = mpsc::channel(10);
    let client = MockDaemonClient {
        commands: commands.clone(),
        event_rx: Arc::new(TokioMutex::new(event_rx)),
    };

    let mut app = Application::new(state, scheduler, client);
    let source = QueueUiEventSource {
        events: vec![
            UiEvent::SubmitPrompt("first".to_string()),
            UiEvent::SubmitPrompt("second".to_string()),
        ],
    };

    app.run(source).await.unwrap();

    let cmds = commands.lock().unwrap();
    assert_eq!(cmds.len(), 2);
    // Sequence IDs must be strictly increasing: 1, then 2
    assert_eq!(
        cmds[0],
        BackendCommand::SubmitPrompt {
            request: RequestId::new(1),
            message: MessageId(2),
            text: "first".to_string()
        }
    );
    assert_eq!(
        cmds[1],
        BackendCommand::SubmitPrompt {
            request: RequestId::new(2),
            message: MessageId(4),
            text: "second".to_string()
        }
    );
}

#[tokio::test]
async fn test_coalesced_render_scheduling() {
    let state = make_test_app_state();
    let scheduler = MockRenderScheduler::new();
    let commands = Arc::new(StdMutex::new(Vec::new()));
    let (_event_tx, event_rx) = mpsc::channel(10);
    let client = MockDaemonClient {
        commands: commands.clone(),
        event_rx: Arc::new(TokioMutex::new(event_rx)),
    };

    let mut app = Application::new(state, scheduler, client);

    // Prompt submission must produce render requests
    let req1 = app
        .handle_ui_event(UiEvent::SubmitPrompt("hello".to_string()))
        .await
        .unwrap();
    assert!(req1.is_some());
    let req = req1.unwrap();
    assert_eq!(req.reason, RenderReason::Input);
    assert_eq!(req.invalidation, RenderInvalidation::EverythingStale);

    // Stream token must produce render requests
    let req2 = app
        .handle_backend_event(BackendEvent::Token {
            message: MessageId(2),
            sequence: 1,
            text: "hi".to_string(),
        })
        .await
        .unwrap();
    assert!(req2.is_some());
    let req_b = req2.unwrap();
    assert_eq!(req_b.reason, RenderReason::StreamToken);
    assert_eq!(req_b.invalidation, RenderInvalidation::ConversationStale);

    // Coalesce request checks
    let coalesced = req.coalesce(req_b);
    assert_eq!(coalesced.reason, RenderReason::Input);
    assert_eq!(coalesced.invalidation, RenderInvalidation::EverythingStale);
}
