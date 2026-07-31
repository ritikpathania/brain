# Ratatui TUI Client Migration (Milestone 6: Sessions & History) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the session browser sidebar, sqlite-backed session listing, selection, and deletion controls inside the native Ratatui TUI client.

**Architecture:**
- Opaque session identifiers (Session ID value objects passed to widgets instead of raw strings).
- Lazy-loaded message history: Session lists load metadata summaries first, hydra-opening full messages only when a session is activated.
- Three isolated Focus/Cursor States:
  - **Focused widget**: Tab cycles focus between the Editor and the Sidebar.
  - **Selected row**: Up/Down arrows move the sidebar cursor row without loading data.
  - **Active session**: Pressing Enter activates the selected session, initiating background message load queries.
- Track Pending Activation atomically: Introduce `PendingLoad { session_id: SessionId, request_id: LoadRequestId }` value object to point to the loading target, avoiding desynchronized state variables. Derive `Clone`, `Debug`, `PartialEq`, `Eq` on `PendingLoad` and treat it as a fully immutable value object. `active_session` continues pointing to the old session to render its messages during the load cycle.
- Run-Loop Owned Request Versioning: The async run loop manages the monotonic `LoadRequestId` counter (an opaque wrapper type around a bare `u64`), passing it into `Action::ActivateSession { session_id, request_id }`. The reducer stores this `request_id` deterministically, using it to discard completed payloads that don't match (preventing late-arriving queries from overwriting newer selections).
- Reducer Acceptance Assertion: Assert via `debug_assert_eq!` in the reducer that an accepted completion payload's `request_id` strictly matches the active `PendingLoad` context.
- Centralized Idempotent Reset logic: Expose an idempotent helper `clear_pending_load()` inside the reducer to safely reset `pending_load = None` during all completion paths (success/failure, cancellation, deletion, shutdown) to avoid orphaned pending references.
- Safe Deletion Lifecycle Invariants:
  - If the active session is deleted, clear `pending_load = None`, select the nearest remaining session in the sidebar list, and trigger a replacement load.
  - If a pending session is deleted mid-load, invalidate the active `pending_load` so that late-arriving completion payloads are discarded safely.
- Session Load State Machine: Introduce `SessionLoadState` (`NotLoaded`, `Loading`, `Loaded(Vec<Message>)`, `Error(String)`) to correctly show loading spinners, messages lists, or error/retry panels.
- Flicker-Free History Transition: When switching sessions, preserve the display of the previously active conversation with an overlaid loading indicator until the new session's messages are successfully resolved. If loading fails, roll back to rendering the previous conversation.
- Optional Preview Summary: Expose `preview: Option<String>` on `SessionViewModel` to allow cheap metadata loads.
- Session List metadata carries: `id`, `title`, `updated_at`, `active`.

**Tech Stack:** Rust, Ratatui, SQLite.

## Global Constraints
- `brain-tui` remains a pure presentation layer crate.
- No direct database or runtime execution internals.
- Widgets never compute layout; `renderer.rs` coordinates area allocation.
- Redrawing occurs selectively when `UiState::update` reports `UpdateResult::Changed`.
- **Opaque Session Value Objects**: Ensure TUI views pass `SessionId` as strongly-typed opaque value objects.
- **Lazy-Load History**: Load session summaries on startup; load full conversation messages only when a specific session is explicitly selected.
- **Metadata Summary Record**: The sidebar list ViewModel holds a minimal summary representation containing `id`, `title`, `updated_at`, `active` status, and an optional `preview`.

---

### Task 1: Session Metadata ViewModel & State Reducer

**Files:**
- Modify: `crates/brain-tui/src/state.rs`

**Interfaces:**
- Consumes: `SessionSummary` and selection inputs.
- Produces: `SessionListState` tracker and state transitions.

- [ ] **Step 1: Define SessionSummary ViewModel & Load States**
  Create `SessionViewModel` carrying `id: SessionId`, `title: String`, `updated_at: SystemTime`, `active: bool`, and `preview: Option<String>`. Define `SessionLoadState`, `LoadRequestId` opaque wrapper, `PendingLoad` grouping struct, and `FocusRegion` enums.
- [ ] **Step 2: Add Session Actions and Reducer Logic**
  Add actions `Action::LoadSessions(Vec<SessionSummary>)`, `Action::ToggleFocus`, `Action::MoveSidebarCursorUp`, `Action::MoveSidebarCursorDown`, `Action::ActivateSession { session_id: SessionId, request_id: LoadRequestId }`, `Action::SessionLoaded { session_id: SessionId, request_id: LoadRequestId, messages: Vec<Message> }`, `Action::SessionLoadFailed { session_id: SessionId, request_id: LoadRequestId, error: String }`, and `Action::DeleteSession(SessionId)`.
  Update `UiState::update` to apply these transitions, preserving previous conversation history while the loader is active.
- [ ] **Step 3: Write unit tests for session switching**
  Add unit tests verifying that selecting a session triggers correct load outcomes, changes `active` flag in list, and handles evictions. Write tests asserting that every terminal path (success, failure, cancellation, deletion, shutdown) leaves the pending state cleared (`pending_load == None`).
- [ ] **Step 4: Run compiler check and verify**
  Verify compilation passes: `cargo check -p brain-tui`.
- [ ] **Step 5: Commit**
  Commit Task 1: `git add . && git commit -m "feat(tui): implement session metadata viewmodels and reducer transitions"`

---

### Task 2: Sidebar UI Widget & ExecutionClient Hooking

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/sidebar.rs` (new), `crates/brain-tui/src/ui/renderer.rs`, `crates/brain-tui/src/lib.rs`

**Interfaces:**
- Consumes: `SessionViewModel` list.
- Produces: Visual sidebar rendering and keybinding event triggers.

- [ ] **Step 1: Create stateless sidebar widget**
  Create a stateless `SidebarWidget` under `crates/brain-tui/src/ui/widgets/sidebar.rs` that renders session titles, highlight outlines, and last-updated metrics.
- [ ] **Step 2: Wire Sidebar into AppRenderer**
  Refactor `renderer.rs` to compute sidebar partitions and delegate drawing.
- [ ] **Step 3: Connect Startup Session Listing in Main Loop**
  On loop startup, query `client.list_sessions()` and dispatch `Action::LoadSessions`.
- [ ] **Step 4: Wire Session Switching and Deletion Keybindings**
  Map Tab to switch focus between editor and sidebar. Map Enter (on sidebar) to trigger `client.load_session(id)` and dispatch `Action::SessionLoaded`. Map Backspace/Delete (on sidebar) to trigger session deletions.
- [ ] **Step 5: Write integration tests for session switching**
  Write an integration test checking list loading, active session switches, and message panel updates. Assert that Conversation A remains fully rendered during the loading phase of Conversation B, and is only replaced on successful completion. Write the double-load deletion race condition integration test.
- [ ] **Step 6: Run workspace test suite and clippy checks**
  Verify all workspace tests pass cleanly with zero compiler warnings.
- [ ] **Step 7: Commit**
  Commit Task 2: `git add . && git commit -m "feat(tui): integrate session sidebar widget and client database hooks"`
