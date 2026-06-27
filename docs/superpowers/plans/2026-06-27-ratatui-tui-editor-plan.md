# Ratatui TUI Client Migration (Milestone 4: Input & Editor) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement interactive prompt input buffers, command history recall storage, and form submit execution within the `brain-tui` presentation crate.

**Architecture:** Extend `EditorState` to own a prompt `HistoryStore` tracking previous submissions. Hitting Enter submits the active draft to the client, clears the editor, and commits it to history. Pressing Up/Down recalls history entries while preserving active typing drafts.

**Tech Stack:** Rust, Ratatui, Crossterm.

## Global Constraints
- `brain-tui` remains a pure presentation layer crate.
- No direct database or runtime execution internals.
- Widgets never compute layout; `renderer.rs` coordinates area allocation.
- Redrawing occurs selectively when `UiState::update` reports `UpdateResult::Changed`.
- **Validation-Aware Submissions**: Prompts must be non-empty and contain non-whitespace characters to be committed to history and trigger submissions.
- **Submit vs Send Decoupling**: Hitting Enter submits the prompt to the state manager. Hitting this state transitions `UiState::update` to return `UpdateResult::PromptSubmitted(String)`. The asynchronous run loop intercepts this result and schedules the client query execution, leaving the reducer side-effect free.
- **Encapsulated History Store**: Expose private navigation helper methods (`push`, `previous`, `next`, `reset_navigation`) on `HistoryStore` instead of exposing indices or internal vector stores.
- **Self-Contained Navigation Resets**: `HistoryStore::push` automatically resets its own internal navigation index pointer and clears cached typing drafts, ensuring navigation is self-correcting.
- **Semantic PromptSubmitted**: Treat `PromptSubmitted` as representing the semantic user outcome rather than being coupled to raw Enter keyboard events, preparing the dispatcher for command palette or mouse buttons submittals in future milestones.
- **Bounded History Capacity**: Configures `HistoryStore` with a hard capacity limit (e.g. 500 entries) to prevent unbounded growth, discarding the oldest entries when capacity is exceeded.
- **Duplicate Preservation**: Retain consecutive and non-consecutive duplicates to preserve the chronological shell-like editing history.
- **Draft Rotation and Restoration Invariant**: When navigating back down from history, discard edits made to recalled historical commands and restore the original uncommitted typing draft.
- **Single Cached Draft Slot**: The original unfinished draft is saved exactly once at the beginning of a history navigation session (when moving up from the newest slot) and is never overwritten during subsequent movements through history, resetting only when returning to the newest entry or submitting.

---

### Task 1: Prompt History Store & State Operations

**Files:**
- Modify: `crates/brain-tui/src/state.rs`

**Interfaces:**
- Consumes: `brain_domain::SessionId`.
- Produces: `HistoryStore`, expanded `EditorState` (owning `HistoryStore`), and new `Action` variants (`SubmitPrompt`, `RecallPrevious`, `RecallNext`).
- Extends: `UpdateResult` to include `PromptSubmitted(String)`.

- [ ] **Step 1: Write a unit test for history recall**
  Add a test verifying that pressing Up/Down scrolls prompt history correctly, caches active drafts exactly once at the start of navigation, handles capacity eviction limits, retains duplicate commands, navigation state resets automatically on push, and navigating back down restores the original draft (discarding historical command edits).
- [ ] **Step 2: Implement HistoryStore**
  Create `HistoryStore` exposing `push(String)`, `previous() -> Option<&str>`, `next() -> Option<&str>`, and `reset_navigation()` methods, enforcing a 500-entry capacity limit.
- [ ] **Step 3: Extend EditorState with History Actions**
  Expose `submit(&mut self) -> Option<String>` to capture prompt text, clear the active buffer, reset history navigation, and push to history if valid. Expose `recall_up(&mut self)` and `recall_down(&mut self)` to rotate prompt text drafts, discarding edits to history items and restoring original unsubmitted drafts.
- [ ] **Step 4: Update Action Enum and Reducer**
  Add `Action::SubmitPrompt`, `Action::RecallPrevious`, and `Action::RecallNext`. Update `UiState::update` to yield `UpdateResult::PromptSubmitted(prompt)` on valid submissions.
- [ ] **Step 5: Run compiler check and unit tests**
  Ensure TUI crate tests compile and pass successfully: `cargo test -p brain-tui`.
- [ ] **Step 6: Commit**
  Commit Task 1: `git add . && git commit -m "feat(tui): implement prompt history storage and recall actions"`

---

### Task 2: Keyboard Form Submission & Run Loop Wiring

**Files:**
- Modify: `crates/brain-tui/src/lib.rs`

**Interfaces:**
- Consumes: `Event` and `UiState`.
- Produces: Command dispatching inside `run()`.

- [ ] **Step 1: Write a loop integration test for history key triggers**
  Add an integration test verifying that feeding Up/Down keystrokes inside the run loop rotates prompt states correctly, and Enter yields `PromptSubmitted`.
- [ ] **Step 2: Map arrow keys and Enter triggers**
  Map incoming terminal keys to `Action` variants:
  - Enter: `Action::SubmitPrompt`.
  - Up Arrow: `Action::RecallPrevious`.
  - Down Arrow: `Action::RecallNext`.
  Pass mapped actions into `state.update`.
- [ ] **Step 3: Run workspace test suite and clippy checks**
  Verify all 111 tests pass cleanly with zero compiler warnings.
- [ ] **Step 4: Commit**
  Commit Task 2: `git add . && git commit -m "feat(tui): map arrow keys and Enter submissions inside run loop"`
