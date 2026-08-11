# Claude Reference Reconstruction Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct Brain TUI's rendering and interactivity to match the local Claude Code reference implementation (`/Users/ritikpathania/Developer/src`) and screenshots with 100% fidelity across geometry, colors, borders, typography, state transitions, and responsive viewports (`80×24`, `127×24`, `80×34`, `182×53`).

**Architecture:** Rebuild the presentation layer in `crates/brain-tui/src/ui/` around the official `CLAUDE_VISUAL_CONTRACT.md`:
- `ThemeSystem`: Canonical RGB palette (`claude` orange `#D77757`, `suggestion` blue-purple `#AFB9F9`, `selectionBg` `#264F78`, `subtle` `#505050`, `promptBorder` `#888888`).
- `HomeWelcomeWidget`: Terracotta rounded/box surface at `y = 2`, integrated title, 2-column split at col 47, right rail, ambient status `● xhigh · /effort`.
- `WorkspaceDashboardWidget`: Full-width task table, multi-agent counts header, background banner, interactive `Up`/`Down` session selection, `Enter` to open session.
- `PromptWidget` & `QuietFooter`: Chevron prefix `❯ `, contextual placeholders, quiet status footer `▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents`.
- `CommandPaletteWidget`: 3-column dropdown (Name, Type/Category in `#AFB9F9`, Description), key navigation (`Up`/`Down`/`Enter`/`Esc`), backed by real Brain commands.

**Tech Stack:** Rust, Ratatui (TUI rendering framework), Crossterm.

## Global Constraints
- **Reference Authority**: Claude Code (`/Users/ritikpathania/Developer/src`) is the external reference. Do NOT alter the reference or synthesize fake approximations.
- **Zero Backend Mutation**: Do NOT modify `brain-domain`, `brain-core`, `brain-storage`, `brain-services`, or `brain-events`.
- **Strict Visual Scope**: All implementation changes are strictly confined to `crates/brain-tui/src/ui/` and `crates/brain-tui/tests/`.

---

### Task 1: Semantic Theme Palette Reconstruction

**Files:**
- Modify: `crates/brain-tui/src/ui/theme/palette.rs`
- Modify: `crates/brain-tui/src/ui/theme/theme.rs`
- Test: `crates/brain-tui/tests/theme_tests.rs`

- [ ] **Step 1: Write failing test for exact RGB color tokens**

```rust
// crates/brain-tui/tests/theme_tests.rs
use brain_tui::ui::theme::{dark_theme, ThemeToken};
use ratatui::style::Color;

#[test]
fn test_claude_canonical_rgb_tokens() {
    let theme = dark_theme();
    assert_eq!(theme.token_color(ThemeToken::HeaderPrimary), Color::Rgb(215, 119, 87)); // #D77757
    assert_eq!(theme.token_color(ThemeToken::Accent), Color::Rgb(215, 119, 87));
    assert_eq!(theme.token_color(ThemeToken::Selection), Color::Rgb(38, 79, 120)); // #264F78
    assert_eq!(theme.token_color(ThemeToken::BorderSubtle), Color::Rgb(215, 119, 87)); // #D77757 home border
}
```

- [ ] **Step 2: Run test to verify it fails** (`cargo test -p brain-tui --test theme_tests`).
- [ ] **Step 3: Update `palette.rs` and `theme.rs` with exact RGB values**.
- [ ] **Step 4: Run test to verify it passes** (`cargo test -p brain-tui --test theme_tests`).
- [ ] **Step 5: Commit**: `git commit -m "feat(ui): update theme palette to exact Claude RGB token definitions"`.

---

### Task 2: Reconstruct `HomeWelcomeWidget` & `AmbientStatusLine`

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/home_welcome.rs`
- Modify: `crates/brain-tui/src/ui/widgets/ambient_status.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/home_welcome_tests.rs`

- [ ] **Step 1: Write failing cell-buffer geometry & color test**

```rust
// crates/brain-tui/tests/home_welcome_tests.rs
use brain_tui::state::UiState;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;

#[test]
fn test_home_welcome_claude_color_and_geometry() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = dark_theme();

    terminal
        .draw(|f| {
            let surface_rect = Rect::new(1, 2, 78, 9);
            brain_tui::ui::widgets::home_welcome::draw(f, surface_rect, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    assert_eq!(buf.get(1, 2).style().fg, Some(Color::Rgb(215, 119, 87))); // #D77757
    assert_eq!(buf.get(47, 4).symbol(), "│");
}
```

- [ ] **Step 2: Run test to verify it fails** (`cargo test -p brain-tui --test home_welcome_tests`).
- [ ] **Step 3: Update `home_welcome.rs` and `renderer.rs`**.
- [ ] **Step 4: Run test to verify it passes** (`cargo test -p brain-tui --test home_welcome_tests`).
- [ ] **Step 5: Commit**: `git commit -m "feat(ui): reconstruct HomeWelcomeWidget with exact terracotta borders and geometry"`.

---

### Task 3: Reconstruct `WorkspaceDashboardWidget` Interactivity & Layout

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/workspace_dashboard.rs`
- Modify: `crates/brain-tui/src/ui/interaction/dispatcher.rs`
- Modify: `crates/brain-tui/src/state.rs`
- Test: `crates/brain-tui/tests/workspace_dashboard_tests.rs`

- [ ] **Step 1: Write failing test for full-width layout and keyboard navigation**.
- [ ] **Step 2: Run test to verify it fails** (`cargo test -p brain-tui --test workspace_dashboard_tests`).
- [ ] **Step 3: Update `workspace_dashboard.rs` and `dispatcher.rs`**.
- [ ] **Step 4: Run test to verify it passes** (`cargo test -p brain-tui --test workspace_dashboard_tests`).
- [ ] **Step 5: Commit**: `git commit -m "feat(ui): implement WorkspaceDashboardWidget selection highlight and keyboard navigation"`.

---

### Task 4: Reconstruct `CommandPaletteWidget` 3-Column Category Layout

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/palette.rs`
- Modify: `crates/brain-tui/src/ui/command/palette.rs`
- Test: `crates/brain-tui/tests/command_palette_reconstruction_tests.rs`

- [ ] **Step 1: Write failing test for 3-column category formatting and selection**.
- [ ] **Step 2: Run test to verify it fails** (`cargo test -p brain-tui --test command_palette_reconstruction_tests`).
- [ ] **Step 3: Update `palette.rs`**.
- [ ] **Step 4: Run test to verify it passes** (`cargo test -p brain-tui --test command_palette_reconstruction_tests`).
- [ ] **Step 5: Commit**: `git commit -m "feat(ui): reconstruct CommandPaletteWidget 3-column layout with blue-purple category tags"`.

---

### Task 5: Master Visual Parity Matrix & Viewport Validation

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/claude_visual_reconstruction_tests.rs`

- [ ] **Step 1: Write comprehensive viewport tests** for `80×24`, `127×24`, `80×34`, `182×53`.
- [ ] **Step 2: Run test to verify pass** (`cargo test -p brain-tui --test claude_visual_reconstruction_tests`).
- [ ] **Step 3: Run full TUI & workspace test suite** (`cargo test -p brain-tui && cargo test --workspace`).
- [ ] **Step 4: Commit**: `git commit -m "feat(ui): complete reference-backed Claude visual reconstruction"`.
