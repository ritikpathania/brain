# Bit-Exact Claude Reference Reconstruction & Cell-Oracle Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan phase-by-phase. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct Brain TUI's rendering, component hierarchy, geometry, styling, and state interactivity to achieve bit-exact cell-level parity with the local Claude reference implementation (`/Users/ritikpathania/Developer/src`) and screenshots across viewports (`80×24`, `127×24`, `80×34`, `182×53`).

**Architecture:** Rebuild the presentation layer in `crates/brain-tui/src/ui/` around a **Bit-Exact Cell Oracle** (`CellSpec` matrix comparison of 8 properties: symbol, foreground RGB, background RGB, bold, italic, underlined, dim, reversed):
- `CellOracle`: Full-grid cell specification test harness comparing all 8 properties per cell with structured diff diagnostics on failure showing `(x, y)`, `Expected`, and `Actual`.
- `Allowed Scope`: `crates/brain-tui/src/ui/`, `crates/brain-tui/src/state.rs`, `crates/brain-tui/src/ui/interaction/dispatcher.rs`, `crates/brain-tui/tests/`.
- `ThemeSystem`: Canonical RGB palette (`claude` orange `#D77757`, `suggestion` blue-purple `#AFB9F9`, `selectionBg` `#264F78`, `subtle` `#505050`, `promptBorder` `#888888`).
- `HomeWelcomeWidget`: Terracotta surface at `y = 2`, integrated title, 2-column split at col 47, right rail, ambient status `● xhigh · /effort`.
- `WorkspaceDashboardWidget`: Full-width task table, multi-agent counts header, background banner, interactive `Up`/`Down` session selection, `Enter` to open session, `Esc` to return to `Screen::Home`/`Screen::Conversation`.
- `PromptWidget` & `QuietFooter`: Chevron prefix `❯ `, contextual placeholders, quiet status footer `▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents`.
- `CommandPaletteWidget`: 3-column dropdown (Name, Type/Category in `#AFB9F9`, Description), key navigation (`Up`/`Down`/`Enter`/`Esc`), backed by real Brain commands.

**Tech Stack:** Rust, Ratatui (TUI rendering framework), Crossterm.

## Global Constraints
- **Zero Synthetic Approximations**: Do NOT substitute loose text matching for cell geometry. Cell-by-cell equality across all 8 properties is mandatory.
- **Zero Backend Mutation**: Do NOT modify `brain-domain`, `brain-core`, `brain-storage`, `brain-services`, or `brain-events`.
- **Allowed Scope Boundary**: Changes permitted strictly within `crates/brain-tui/src/ui/`, `crates/brain-tui/src/state.rs`, `crates/brain-tui/src/ui/interaction/dispatcher.rs`, and `crates/brain-tui/tests/`.

---

### Phase 1: Reference Lock & Cell-Level Oracle Test Infrastructure

**Files:**
- Create: `crates/brain-tui/tests/common/cell_oracle.rs`
- Create: `crates/brain-tui/tests/claude_visual_oracle_tests.rs`

- [ ] **Step 1: Implement 8-property `CellSpec` and `inspect_cell_grid` helper with structured diff diagnostics**

```rust
// crates/brain-tui/tests/common/cell_oracle.rs
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSpec {
    pub symbol: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub dim: bool,
    pub reversed: bool,
}

pub fn inspect_cell(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> CellSpec {
    let buf = terminal.backend().buffer();
    let cell = buf.get(x, y);
    CellSpec {
        symbol: cell.symbol().to_string(),
        fg: cell.style().fg,
        bg: cell.style().bg,
        bold: cell.style().add_modifier.contains(Modifier::BOLD),
        italic: cell.style().add_modifier.contains(Modifier::ITALIC),
        underlined: cell.style().add_modifier.contains(Modifier::UNDERLINED),
        dim: cell.style().add_modifier.contains(Modifier::DIM),
        reversed: cell.style().add_modifier.contains(Modifier::REVERSED),
    }
}
```

- [ ] **Step 2: Write initial failing oracle test in `claude_visual_oracle_tests.rs`** asserting cell `(1, 2)` foreground is `Color::Rgb(215, 119, 87)`.
- [ ] **Step 3: Run test to verify it fails** (`cargo test -p brain-tui --test claude_visual_oracle_tests`).
- [ ] **Step 4: Commit**: `git commit -m "test(ui): add cell-level visual oracle test infrastructure with structured diff diagnostics"`.

---

### Phase 2: Theme System & Exact RGB Palette Alignment

**Files:**
- Modify: `crates/brain-tui/src/ui/theme/palette.rs`
- Modify: `crates/brain-tui/src/ui/theme/theme.rs`
- Test: `crates/brain-tui/tests/theme_tests.rs`

- [ ] **Step 1: Write failing test asserting theme RGB colors match `CLAUDE_VISUAL_CONTRACT.md`**.
- [ ] **Step 2: Update `palette.rs` and `theme.rs` with exact RGB values**.
- [ ] **Step 3: Run test to verify it passes** (`cargo test -p brain-tui --test theme_tests`).
- [ ] **Step 4: Commit**: `git commit -m "feat(ui): align theme system to canonical Claude RGB palette"`.

---

### Phase 3: Home Welcome Surface Reconstruction & Geometry Lock

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/home_welcome.rs`
- Modify: `crates/brain-tui/src/ui/widgets/ambient_status.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/home_welcome_tests.rs`

- [ ] **Step 1: Write cell-oracle tests verifying border box at `y = 2`, integrated title color `#D77757`, vertical divider `│` at `x = 47` in `#505050`, ambient status at `y = 19`**.
- [ ] **Step 2: Update `home_welcome.rs` and `renderer.rs`**.
- [ ] **Step 3: Run test to verify it passes** (`cargo test -p brain-tui --test home_welcome_tests`).
- [ ] **Step 4: Commit**: `git commit -m "feat(ui): reconstruct HomeWelcomeWidget with exact terracotta border and cell geometry"`.

---

### Phase 4: Conversation Canvas & Streaming Response Reconstruction

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/chat.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/streaming_rendering_tests.rs`

- [ ] **Step 1: Write failing test asserting `HomeWelcomeSurface` stays at head of canvas scroll buffer when messages are added**.
- [ ] **Step 2: Update `chat.rs` and `renderer.rs`**.
- [ ] **Step 3: Run test to verify it passes** (`cargo test -p brain-tui --test streaming_rendering_tests`).
- [ ] **Step 4: Commit**: `git commit -m "feat(ui): preserve HomeWelcomeSurface in transcript canvas scroll buffer"`.

---

### Phase 5: Workspace Dashboard & Reducer State Machine Interactivity

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/workspace_dashboard.rs`
- Modify: `crates/brain-tui/src/ui/interaction/dispatcher.rs`
- Modify: `crates/brain-tui/src/state.rs`
- Test: `crates/brain-tui/tests/workspace_dashboard_tests.rs`

- [ ] **Step 1: Write failing interactive reducer state machine test**:
  - Assert `Screen::Workspace` displays full-width task table.
  - Dispatch `Action::NavigateDown` -> assert selected row index changes to 1, row background is `selectionBg` (`#264F78`).
  - Dispatch `Action::NavigateUp` -> assert selected row index returns to 0.
  - Dispatch `Action::SelectSession` -> assert `screen` transitions to `Screen::Conversation`.
  - Dispatch `Action::Escape` -> assert `screen` transitions back to `Screen::Home` / `Screen::Conversation`.
- [ ] **Step 2: Run test to verify it fails** (`cargo test -p brain-tui --test workspace_dashboard_tests`).
- [ ] **Step 3: Implement keyboard routing and reducer transitions in `dispatcher.rs` and `state.rs`**.
- [ ] **Step 4: Run test to verify it passes** (`cargo test -p brain-tui --test workspace_dashboard_tests`).
- [ ] **Step 5: Commit**: `git commit -m "feat(ui): implement Workspace interactive reducer state machine and row selection"`.

---

### Phase 6: Command Palette 3-Column Layout & Key Navigation

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/palette.rs`
- Modify: `crates/brain-tui/src/ui/command/palette.rs`
- Test: `crates/brain-tui/tests/command_palette_reconstruction_tests.rs`

- [ ] **Step 1: Write failing test for 3-column layout formatting, category color `#AFB9F9`, selection background `#264F78`, `Up`/`Down`/`Enter`/`Esc` key routing**.
- [ ] **Step 2: Update `palette.rs`**.
- [ ] **Step 3: Run test to verify it passes** (`cargo test -p brain-tui --test command_palette_reconstruction_tests`).
- [ ] **Step 4: Commit**: `git commit -m "feat(ui): reconstruct CommandPaletteWidget 3-column layout and key navigation"`.

---

### Phase 7: Certified Viewport Matrix (`80×24`, `127×24`, `80×34`, `182×53`)

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/claude_visual_oracle_tests.rs`

- [ ] **Step 1: Implement full cell-grid oracle test suite** across all 4 certified viewports.
- [ ] **Step 2: Run test to verify it passes** (`cargo test -p brain-tui --test claude_visual_oracle_tests`).
- [ ] **Step 3: Commit**: `git commit -m "feat(ui): validate cell-level visual oracle across certified viewports"`.

---

### Phase 8: Full Workspace Regression Certification Gate

- [ ] **Step 1: Run full TUI test suite**: `cargo test -p brain-tui`.
- [ ] **Step 2: Run full workspace test suite**: `cargo test --workspace`.
- [ ] **Step 3: Commit**: `git commit -m "feat(ui): pass master workspace regression gate for Claude reference reconstruction"`.
