# Ratatui TUI Client Migration (Milestone 1: Scaffold) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the foundation of the native Rust Ratatui TUI client by configuring crate dependencies, setting up canonical shared events, defining client abstractions, building the async event multiplexer, and constructing the layout grid and CLI runner hook.

**Architecture:** We configure `crates/brain-tui` to depend on `ratatui` and `crossterm`. The TUI interacts with the engine via a transport-agnostic `ExecutionClient` trait. Operating system inputs (Crossterm raw mode keys) and application events are multiplexed via an async channel loop.

**Tech Stack:** Rust, Ratatui, Crossterm, Tokio.

## Global Constraints
- `brain-tui` remains a pure presentation layer crate.
- No direct database or interpreter dependencies in `crates/brain-tui`.
- Widgets never compute layout; the renderer owns all layout constraints and rect scopes.
- Canonical event models are defined in `brain-core` to maintain separation of concerns.
- **High-Throughput Note**: Unbounded receivers are used for the scaffold. If profiling shows memory bloat under sustained high-throughput streaming, evaluate switching to bounded channels with backpressure.

---

### Task 1: Crate Configurations & Domain Client Traits

**Files:**
- Create: `crates/brain-core/src/events.rs`
- Modify: `crates/brain-core/src/lib.rs`
- Modify: `crates/brain-tui/Cargo.toml`
- Create: `crates/brain-tui/src/client.rs`

**Interfaces:**
- Consumes: `brain_domain::SessionId` and `brain_domain::Message`.
- Produces: `StreamEvent` protocol envelope, `ExecutionRequest`, `ExecutionOptions`, `EventReceiver` wrapper, and the `ExecutionClient` trait.

- [ ] **Step 1: Write a unit test for canonical event serialization**
  Create `crates/brain-core/src/events.rs` and verify we can serialize and deserialize `StreamEvent` frames.
- [ ] **Step 2: Define StreamEvent structures in brain-core**
  Implement `EventMetadata`, `StreamEvent`, and `StreamEventKind` enums deriving `Serialize` and `Deserialize`.
- [ ] **Step 3: Register events in brain-core lib.rs**
  Expose `pub mod events;` in `crates/brain-core/src/lib.rs`.
- [ ] **Step 4: Configure TUI dependencies**
  Update `crates/brain-tui/Cargo.toml` to depend on `ratatui` (with crossterm backend), `crossterm`, `tokio`, `serde`, `brain-core`, and `brain-domain`.
- [ ] **Step 5: Write a test for Client Abstraction**
  Create `crates/brain-tui/src/client.rs` and add a test verifying a mock implementation of `ExecutionClient` yields structured messages.
- [ ] **Step 6: Implement ExecutionClient traits**
  Declare `ExecutionRequest`, `ExecutionOptions`, `EventReceiver` (encapsulating an unbounded channel receiver), and the `ExecutionClient` trait.
- [ ] **Step 7: Run compilation and unit tests**
  Execute `cargo test -p brain-core` and verify everything builds cleanly.
- [ ] **Step 8: Commit**
  Commit the changes for Task 1: `git add . && git commit -m "feat(tui): configure crate dependencies and client traits"`

---

### Task 2: Core Event Loop & Crossterm Input Reader

**Files:**
- Create: `crates/brain-tui/src/event.rs`

**Interfaces:**
- Consumes: `crossterm::event::KeyEvent` and `crossterm::event::MouseEvent`.
- Produces: `TerminalEvent`, `AppEvent`, and `Event` multiplexer stream.

- [ ] **Step 1: Write a unit test for event multiplexing**
  Create `crates/brain-tui/src/event.rs` and implement a test verifying that sending crossterm events or clock ticks correctly yields them in chronological order.
- [ ] **Step 2: Define TuiEvent structure**
  Implement `TerminalEvent`, `AppEvent`, and `Event` enums.
- [ ] **Step 3: Implement Async EventHandler**
  Create `pub struct EventHandler` containing a `tokio::sync::mpsc::UnboundedReceiver<Event>`. Spawn a background Crossterm polling loop and a tick interval timer to forward events onto the channel.
- [ ] **Step 4: Run compiler check and event tests**
  Execute `cargo test -p brain-tui` and ensure tests pass.
- [ ] **Step 5: Commit**
  Commit Task 2: `git add . && git commit -m "feat(tui): implement async terminal and application event multiplexer"`

---

### Task 3: Layout Grid & App Integration

**Files:**
- Create: `crates/brain-tui/src/ui/renderer.rs`
- Create: `crates/brain-tui/src/ui/mod.rs`
- Modify: `crates/brain-tui/src/lib.rs`
- Modify: `apps/brain-v2/Cargo.toml`
- Modify: `apps/brain-v2/src/main.rs`

**Interfaces:**
- Consumes: `ExecutionClient`.
- Produces: `brain_tui::run(client: Box<dyn ExecutionClient>) -> Result<(), BrainError>`.

- [ ] **Step 1: Write an event-loop lifecycle integration test**
  Add a test inside `crates/brain-tui/src/lib.rs` that programmatically starts the TUI event loop, feeds it a Shutdown event, and asserts that the raw mode setup is restored cleanly on exit. Avoid asserting directly on global OS terminal state; instead, encapsulate setup/teardown behind an RAII guard abstraction and verify initialization succeeds, event loop exits cleanly, and cleanup is executed deterministically.
- [ ] **Step 2: Implement Layout Grid**
  Create `crates/brain-tui/src/ui/renderer.rs` with `pub struct AppRenderer` partitioning the terminal window into Logo, Chat Area, Prompt, and Status cells. Ensure widgets never compute their own layout.
- [ ] **Step 3: Implement TUI entry run function**
  Implement `pub fn run(client: Box<dyn ExecutionClient>) -> Result<(), BrainError>` in `crates/brain-tui/src/lib.rs`. It should initialize raw mode, build the terminal object, enter the event loop, and restore raw mode on exit.
- [ ] **Step 4: Link brain-tui to apps/brain-v2**
  Add `brain-tui = { path = "../../crates/brain-tui" }` to `apps/brain-v2/Cargo.toml`.
- [ ] **Step 5: Bind CLI subcommand in main.rs**
  Modify `apps/brain-v2/src/main.rs` to parse CLI subcommands (defaulting to starting the interactive TUI) and execute `brain_tui::run` under the appropriate configuration.
- [ ] **Step 6: Run final integration build and verify cargo test**
  Verify the full workspace builds and executes tests successfully.
- [ ] **Step 7: Commit**
  Commit Task 3: `git add . && git commit -m "feat(tui): implement ratatui layout grid and daemon CLI execution hook"`
