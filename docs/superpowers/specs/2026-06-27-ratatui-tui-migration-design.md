# Design Specification: Native Ratatui TUI Client Migration

This document outlines the architectural specification to migrate the Brain terminal user interface client from the legacy React + Ink + Yoga frontend to a native Rust implementation using Ratatui and Crossterm.

---

## 1. Crate Boundaries & Dependencies

The migrated client architecture is split into two primary components to preserve strict separation of concerns:

1. **`crates/brain-tui`**: A pure library presentation crate. It is responsible for terminal rendering, event handling, viewports, keyboard editing, and local user controls.
   - **Allowed Dependencies**: `ratatui`, `crossterm`, `tokio`, `brain-core`, `brain-domain` (canonical domain models), `brain-services` (stable service APIs/DTOs only), `brain-observability`.
   - **Forbidden Dependencies**: Direct database storage layers (`brain-storage`), raw Python interpreter runtimes (`brain-python`), or runtime daemon execution internals.
2. **`apps/brain`**: The primary executable composition root. It parses CLI flags, manages the background daemon lifecycle, and instantiates/executes `brain-tui`.

---

## 2. Event Stream Protocol & Client Abstraction

### A. Canonical Shared Events (`brain-core`)
`StreamEvent` is moved from services to `crates/brain-core/src/events.rs` to serve as the unified IPC and in-process contract.

```rust
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub execution_id: Uuid,
    pub sequence: u64,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub metadata: EventMetadata,
    pub kind: StreamEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEventKind {
    Token(String),
    Progress { message: String, percentage: Option<f32> },
    Stage { name: String, active: bool },
    Finished { response: String },
    Cancelled,
    Error { message: String },
}
```

### B. Stable Execution Client
The TUI queries business operations exclusively through the `ExecutionClient` trait:

```rust
use async_trait::async_trait;
use brain_core::errors::BrainError;
use brain_domain::{SessionId, Message};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    pub model: Option<String>,
    pub run_goal_mode: bool,
    pub custom_parameters: std::collections::HashMap<String, String>,
}

pub struct ExecutionRequest {
    pub session_id: SessionId,
    pub prompt: String,
    pub options: ExecutionOptions,
    pub cancellation_token: CancellationToken,
}

pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub updated_at: SystemTime,
}

pub struct EventReceiver {
    rx: tokio::sync::mpsc::UnboundedReceiver<Result<StreamEvent, BrainError>>,
    cancellation_token: CancellationToken,
}

impl EventReceiver {
    pub async fn recv(&mut self) -> Option<Result<StreamEvent, BrainError>> {
        self.rx.recv().await
    }
    
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

#[async_trait]
pub trait ExecutionClient: Send + Sync {
    /// Submits a query request and returns a cancellable event stream receiver.
    async fn execute(&self, req: ExecutionRequest) -> Result<EventReceiver, BrainError>;
    
    /// Lists all historical sessions.
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError>;
    
    /// Loads a historical session thread.
    async fn load_session(&self, id: SessionId) -> Result<Vec<Message>, BrainError>;
    
    /// Deletes a historical session.
    async fn delete_session(&self, id: SessionId) -> Result<(), BrainError>;
}
```

We provide two implementations:
1. **`UdsClient`**: Communicates with the daemon over `~/.brain/daemon.sock` using newline-delimited JSON.
2. **`EmbeddedClient`**: Routes calls directly to the in-process `ApplicationRuntime`.

---

## 3. Dual-Queue Event Loop

To ensure OS terminal interactions are isolated from application-level streams, we multiplex inputs:

```rust
pub enum TerminalEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),
}

pub enum AppEvent {
    Stream(StreamEvent),
    Error(String),
    Shutdown,
}

pub enum Event {
    Terminal(TerminalEvent),
    App(AppEvent),
    Tick,
}
```
- **Terminal Inputs**: background Crossterm poll task mapped to `Event::Terminal`.
- **Application Ingestion**: active streams mapping events to `Event::App`.
- **Clock Tick**: 100ms interval for spinners, cursor blink, and typewriter pacing.

---

## 4. Layout, UI State, & Renderer Boundary

### A. Stateless Drawing Widgets & Layout Invariant
- The presentation layer is separated into layout owners and drawing widgets under `crates/brain-tui/src/ui/`:
  - **`renderer.rs`**: Owns main grid constraints assembly.
  - **`widgets/chat.rs`**, **`widgets/input.rs`**, **`widgets/status.rs`**: Stateless drawing functions.
- **Layout Invariant**: Widgets never compute layout. The renderer owns all layout decisions and passes `Rect` scopes into stateless widgets.
- Colors conform to `DESIGN.md` palette tags (`claude` orange, `permission` blue-purple) dynamically resolved from `Theme` configs.

### B. Unified UI State
All interactive state is tracked in `UiState`:

```rust
pub struct UiState {
    pub connection_mode: ConnectionMode,
    pub session_id: SessionId,
    pub session_title: String,
    pub messages: Vec<Message>,
    pub editor: EditorState,
    pub viewport: ViewportState,
    pub typewriter: TypewriterRenderer,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Daemon,
    Embedded,
    Disconnected,
    Connecting,
}

pub struct ViewportState {
    pub scroll_offset: usize,
    pub follow_tail: bool,
    pub is_command_palette_open: bool,
}

pub struct EditorState {
    pub text: String,
    pub cursor: usize,
    pub history: HistoryStore,
}
```

---

## 5. Typewriter & Tokenizer Engine

Incoming stream events pass through a clean processing pipeline to tokenize and queue chunks:

```
StreamEvent (Chunk/Token) ──► Tokenizer ──► RenderToken ──► TypewriterRenderer ──► Renderer
```

```rust
pub enum RenderToken {
    Text(String),
    Whitespace(String),
    Newline,
}

pub struct TypewriterRenderer {
    token_queue: std::collections::VecDeque<RenderToken>,
    network_finished: bool,
    is_streaming: bool,
    elapsed_budget: std::time::Duration,
}

impl TypewriterRenderer {
    /// Steps the typewriter by measuring elapsed time, popping and rendering tokens to fit speed budgets.
    pub fn advance(&mut self, delta: std::time::Duration) -> Vec<RenderToken> {
        // Pop tokens matching typing speed budgets
        todo!()
    }
    
    /// Flushes all remaining queued tokens immediately to skip animation.
    pub fn flush(&mut self) -> Vec<RenderToken> {
        todo!()
    }
}
```

*Note: Stream sequence verification warnings are logged directly to `brain-observability` / local diagnostics log, never outputting technical transport diagnostics inside the user-facing chat panel.*

---

## 6. History, Slash Commands, & Command Palette

1. **`HistoryStore`**: Pressing Up/Down recalls older commands while preserving the user's active prompt draft.
2. **`SlashCommandRegistry`**: Encapsulates slash command routing:
   - **Local UI Commands** (`/theme`, `/clear`, `/help`, `/quit`) are resolved instantly within the TUI state machine.
   - **Execution Commands** (`/goal`, `/model`, `/agent`) alter `ExecutionRequest` options.
3. **Command Palette (`Ctrl+P` / `Ctrl+K`)**: Opens a modal selection overlay for quick navigation (session list, theme picking, setting parameters).

---

## 7. Migration & Verification Phases

We execute Approach 1 in a progressive cycle:
1. **Milestone 1 (Scaffold)**: Define event loops, crossterm bindings, and layout grids.
2. **Milestone 2 (Event Loop)**: Standardize Terminal/App queue multiplexers.
3. **Milestone 3 (Rendering & Layout)**: Implement layout grids and stateless widgets.
4. **Milestone 4 (Input & Editor)**: Build text editor buffers with Command/History recall.
5. **Milestone 5 (Streaming)**: Connect Tokenizer, TypewriterRenderer, and cancellation.
6. **Milestone 6 (Sessions & History)**: Connect storage DTOs and command palettes.
7. **Milestone 7 (Parity Validation)**: Verify visual layout and action parity.
8. **Milestone 8 (Purge)**: Remove all npm/bun configurations and package scripts.
