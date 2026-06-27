# Ratatui TUI Client Migration (Milestone 3: Rendering & Layout) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the stateless UI widgets (Header, Chat Area, Prompt, and Status Line) and render them within the layout grids managed by `AppRenderer`, using design style palettes consistent with `DESIGN.md`.

**Architecture:** Create drawing widgets under `crates/brain-tui/src/ui/widgets/`. Keep widgets entirely stateless and side-effect free, drawing text and shapes into the preallocated cell zones (`Rect`) passed by `AppRenderer`.
We utilize a ViewModel assembly pipeline inside `AppRenderer`:
```
UiState ──► ViewModel Builder ──► HeaderView/ChatView/PromptView/StatusView ──► Widgets
```

**Tech Stack:** Rust, Ratatui, Crossterm.

## Global Constraints
- `brain-tui` remains a pure presentation layer crate.
- Widgets never compute layout; `renderer.rs` coordinates area allocation.
- Redrawing occurs selectively when `UiState::update` reports `UpdateResult::Changed`.
- **Semantic Theme Design**: Style maps in `theme.rs` must expose semantic roles (e.g., `primary`, `accent`, `success`, `warning`, `error`, `border`, `cursor`) rather than raw color names, mapping to `DESIGN.md` tokens internally.
- **View-Model Assembly Invariant**: `AppRenderer` acts as the ViewModel builder. Widgets only consume lightweight derived ViewModels (`HeaderView`, `ChatView`, `PromptView`, `StatusView`) built from `UiState`, isolating formatting decisions to `renderer.rs`.
- **Cursor Width Warning**: Avoid baking "one character equals one column" assumptions into the rendering loop. Account for display widths of CJK/emoji characters during layout cursor positioning.
- **Robust Smoke Tests**: The TUI layout tests must verify allocations and rendering at representative sizes (e.g. 80x24 and 120x40) to guard against area panic overflows.

---

### Task 1: Stateless Widgets, ViewModels, & Style Theme

**Files:**
- Create: `crates/brain-tui/src/ui/theme.rs`
- Create: `crates/brain-tui/src/ui/widgets/header.rs`
- Create: `crates/brain-tui/src/ui/widgets/chat.rs`
- Create: `crates/brain-tui/src/ui/widgets/prompt.rs`
- Create: `crates/brain-tui/src/ui/widgets/status.rs`
- Create: `crates/brain-tui/src/ui/widgets/mod.rs`

**Interfaces:**
- Consumes: Semantic ViewModels and `Theme` styling tokens.
- Produces: Stateless widget drawing hooks:
  - `header::draw(f: &mut Frame<'_>, area: Rect, view: &HeaderView)`
  - `chat::draw(f: &mut Frame<'_>, area: Rect, view: &ChatView)`
  - `prompt::draw(f: &mut Frame<'_>, area: Rect, view: &PromptView)`
  - `status::draw(f: &mut Frame<'_>, area: Rect, view: &StatusView)`

- [ ] **Step 1: Define Theme Palette**
  Create `crates/brain-tui/src/ui/theme.rs` exporting style maps resolving semantic roles (e.g., `Theme::border()`, `Theme::primary()`) from configured color configurations.
- [ ] **Step 2: Declare ViewModel structures**
  Define `HeaderView`, `ChatView`, `PromptView`, and `StatusView` structs carrying formatted string labels and data items.
- [ ] **Step 3: Implement Header Widget**
  Create `crates/brain-tui/src/ui/widgets/header.rs` to render the TUI logo and connection mode pill.
- [ ] **Step 4: Implement Chat Widget**
  Create `crates/brain-tui/src/ui/widgets/chat.rs` to render historical messages and scroll bars.
- [ ] **Step 5: Implement Prompt Widget**
  Create `crates/brain-tui/src/ui/widgets/prompt.rs` to draw the prompt border, text buffer, and active cursor cell.
- [ ] **Step 6: Implement Status Line Widget**
  Create `crates/brain-tui/src/ui/widgets/status.rs` to draw context bindings.
- [ ] **Step 7: Expose widgets module**
  Create `crates/brain-tui/src/ui/widgets/mod.rs` registering all widgets and expose `theme` and `widgets` modules in `crates/brain-tui/src/ui/mod.rs`.
- [ ] **Step 8: Run compiler check and verify**
  Ensure code compiles successfully: `cargo check -p brain-tui`.
- [ ] **Step 9: Commit**
  Commit Task 1: `git add . && git commit -m "feat(tui): implement stateless widgets, viewmodels, and theme palettes"`

---

### Task 2: Layout Assembly & Screen Redrawing

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Modify: `crates/brain-tui/src/lib.rs`

**Interfaces:**
- Consumes: `AppRenderer` layout bounds and ViewModels.
- Produces: Visual frame assembly inside `run()`.

- [ ] **Step 1: Write a robust size smoke test**
  Add a test verifying that drawing the full UI state into a mock terminal backend succeeds at 80x24 and 120x40.
- [ ] **Step 2: Assemble ViewModels & widgets in AppRenderer**
  Update `AppRenderer` in `crates/brain-tui/src/ui/renderer.rs` to derive ViewModels from `UiState` and draw all widgets in their designated partitions.
- [ ] **Step 3: Integrate AppRenderer inside run loop**
  Update `run()` in `crates/brain-tui/src/lib.rs` to pass `UiState` to `AppRenderer` for rendering frames.
- [ ] **Step 4: Run workspace test suite and clippy**
  Verify everything builds, passes tests, and has zero clippy warnings.
- [ ] **Step 5: Commit**
  Commit Task 2: `git add . && git commit -m "feat(tui): assemble widgets and integrate renderer inside TUI run loop"`
