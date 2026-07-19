# Ratatui TUI Client Migration (Milestone 5: Streaming & Typewriter) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the asynchronous streaming transport pipe, the generation state machine, and the typewriter-based token streaming renderer inside the `brain-tui` presentation crate.

**Architecture:**
- Introduce a data-bearing `GenerationState` enum tracking stream lifecycles.
- Buffer incoming raw text chunks into a typewriter queue, drawing them out using a tick-pacing timer for smooth visual flow.
- Block new user submittals and bind Esc to cancellation when generation is active.

**Tech Stack:** Rust, Ratatui, Crossterm.

## Global Constraints
- `brain-tui` remains a pure presentation layer crate.
- No direct database or runtime execution internals.
- Widgets never compute layout; `renderer.rs` coordinates area allocation.
- Redrawing occurs selectively when `UiState::update` reports `UpdateResult::Changed`.
- **Data-Bearing Stream Lifecycle Machine**: Introduce a data-bearing `GenerationState` enum (`Idle`, `Starting`, `Streaming { started_at: std::time::SystemTime }`, `Finished`, `Cancelled(Option<String>)`, `Error(String)`) to coordinate state updates and status line messages.
- **Extensible Semantic RenderToken Queue**:
  - Declare the extensible presentation contract `RenderToken` inside `crates/brain-tui/src/state.rs` (or a stable module) rather than coupling it to specific widgets.
  - `TypewriterQueue` buffers presentation-oriented `RenderToken` variants (`Text(String)`, `Code(String)`) rather than raw strings.
  - The `RenderToken` structure uses semantic variants (such as `Heading(usize)`, `Bullet`, `CodeBlock(String)`) rather than raw styling directives (like `Bold` or `BlueText`).
- **Tokenizer Ownership & Incremental Parsing**:
  - Only the Tokenizer is responsible for creating `RenderToken`s. The typewriter queue, reducer, and widgets only consume/pass them down, preventing divergent semantic logic in different components.
  - The Tokenizer processes text chunks incrementally, accumulating partial inputs until complete semantic `RenderToken`s can be emitted, defending against transport fragments splitting words/markdown formatting tags across boundary packets.
- **RenderToken Value Object Immutability & Stable Equality**: All layers in the presentation pipeline treat `RenderToken` as an immutable value object. Derive `Clone`, `Debug`, `PartialEq`, `Eq`, `Serialize`, `Deserialize` on `RenderToken` for stable equality.
- **Loose Coupling of State and Queue**: Maintain `GenerationState` and `TypewriterQueue` as independent fields within `UiState` and `EditorState` respectively. Do not nest the queue inside the state enum, preserving distinct concerns.
- **Decoupled Pacing Logic**: `TypewriterQueue` is completely decoupled from Ratatui (owning no concepts of spans, styles, frames, or widgets). Its sole concern is `RenderToken` ingress, pacing, and egress.
- **Encapsulated TypewriterQueue APIs**: Hide all internal collection collections (like `VecDeque`) completely. Expose only semantic APIs (`push`, `drain_for_tick(Instant)`, `clear`, `is_empty`, `is_finished`).
- **Semantic DrainResult**: Expose `DrainResult` deriving stable equality traits (`Clone`, `Debug`, `PartialEq`, `Eq`). `TypewriterQueue::drain_for_tick(Instant)` returns a `DrainResult` carrying the newly emitted tokens list and a `finished: bool` flag, isolating transition logic details from the reducer.
- **Independent State Machine Transitions**:
  - `Action::ReceiveToken` executes only `TypewriterQueue::push()`.
  - `Action::TypewriterTick(Instant)` executes only `TypewriterQueue::drain_for_tick(Instant)`.
  - Neither action implicitly triggers the other, maintaining a clean boundary between network progression and visual progress.
- **Time-Driven Typewriter Pacing**: `TypewriterQueue` uses elapsed time calculations (`now: Instant` or `SystemTime` comparison) to determine the number of tokens to emit per tick, making it resilient to missed event loop ticks and maintaining consistent typewriter speed.
- **Injectable Timing for Testing**: Expose `drain_for_tick(now: Instant)` to accept an external timestamp, enabling tests to advance time deterministically without thread sleeps.
- **Immediate Cancellation**: When `Esc` is pressed, cancel the client channel immediately, discard all remaining tokens from the typewriter queue, and transition the UI state to `Cancelled`.
- **Backend vs Animation Completion Separation**:
  - Distinguish between the backend stream finishing (`Action::FinishStream`) vs the visible animation finishing.
  - Expose `is_empty()` and `is_finished()` on `TypewriterQueue`.
  - Invariant: `queue.is_finished() == true` implies `backend_finished == true` and `queue.is_empty() == true`. The queue can be empty while waiting for new tokens, but is only finished when both conditions are satisfied.
  - Maintain the active `GenerationState` as `Streaming` while the queue is still draining, transitioning to `Finished` only when the last token is actually drained and displayed.
- **Separate Network Arrival from Visual Progression**:
  - `Action::ReceiveToken` puts tokens into the `TypewriterQueue` without appending directly to the visible display buffer.
  - `Action::TypewriterTick` calls `drain_for_tick()` to append tokens incrementally to the visible text, maintaining smooth animation pacing regardless of network burstiness.
- **Multi-Tick Integration Test**: Write a test verifying that when A, B, C tokens arrive without ticks, they are not visible. A tick renders A, another tick renders B, and hitting Esc clears the queue and sets the status to `Cancelled`.
- **Reducer Invariant Test**: Write a test asserting that if the backend finishes (`Action::FinishStream`) but the typewriter queue still contains tokens, `GenerationState` remains `Streaming`. It must only transition to `Finished` on a subsequent `TypewriterTick` that drains the last token.

---

### Task 1: Generation State Machine & Typewriter Queue

**Files:**
- Modify: `crates/brain-tui/src/state.rs`

**Interfaces:**
- Consumes: `StreamEvent` tokens.
- Produces: `GenerationState` enum, `TypewriterQueue` state tracker, and updated reducer transitions.

- [ ] **Step 1: Define GenerationState Enum**
  Create `GenerationState` enum carrying state data (e.g. `Error(String)`, `Streaming { started_at: SystemTime }`). Add it to `UiState` representing active generation status.
- [ ] **Step 2: Declare RenderToken & Implement TypewriterQueue**
  Define `RenderToken` carrying `Text(String)` and `Code(String)`. Build `TypewriterQueue` which buffers `RenderToken`s, exposes `is_empty()`, and drains them based on elapsed time increments.
- [ ] **Step 3: Update Action and Reducer Transitions**
  Add actions `Action::StartStream`, `Action::ReceiveToken(RenderToken)`, `Action::TypewriterTick(std::time::Instant)`, `Action::FinishStream`, `Action::CancelStream`, and `Action::ReportError(String)`.
  Update `UiState::update` to reject `Action::SubmitPrompt` if generation is active.
- [ ] **Step 4: Write unit tests for state lifecycle transitions**
  Add unit tests verifying that state changes (`Idle` -> `Starting` -> `Streaming` -> `Finished`) transition correctly, reject submissions when active, and check multi-tick queue progress.
- [ ] **Step 5: Run compiler check and verify**
  Verify compilation passes: `cargo check -p brain-tui`.
- [ ] **Step 6: Commit**
  Commit Task 1: `git add . && git commit -m "feat(tui): implement generation state machine and typewriter queue"`

---

### Task 2: Typewriter Pacing & Stream Event Integration

**Files:**
- Modify: `crates/brain-tui/src/lib.rs`

**Interfaces:**
- Consumes: `EventReceiver` channel and `Event::Tick`.
- Produces: Smooth typewriter drawing updates inside run loop.

- [ ] **Step 1: Write integration tests for streaming ticks**
  Write an integration test simulating tick pacing, checking that cached text grows incrementally, and that Esc cancels active streams.
- [ ] **Step 2: Map Stream Events to Actions**
  Map incoming stream tokens to `Action::ReceiveToken` and final completions to `Action::FinishStream` inside the loop reader thread.
- [ ] **Step 3: Wire Typewriter Draining on Tick Events**
  On `Event::Tick`, dispatch `Action::TypewriterTick(std::time::Instant::now())` to drain buffered tokens into display message lists, triggering redrawing.
- [ ] **Step 4: Map Esc to Cancellation Trigger**
  If `Esc` is pressed while generation is active, send a cancellation request to the active `EventReceiver` and dispatch `Action::CancelStream`.
- [ ] **Step 5: Run workspace test suite and clippy checks**
  Verify all 113 tests pass cleanly with zero compiler warnings.
- [ ] **Step 6: Commit**
  Commit Task 2: `git add . && git commit -m "feat(tui): integrate typewriter ticks and stream cancellation hooks"`
