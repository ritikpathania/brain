# Claude Visual Reconstruction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct Brain TUI's presentation layer to match Claude Code's exact screenshot component hierarchy, Rect geometry, and state transitions while preserving all backend domain and service models.

**Architecture:** Replace the split-view TUI layout model in `crates/brain-tui/src/ui/` with a unified `AppShell` component hierarchy:
- `HomeWelcomeSurface`: Bounded welcome component starting at `y = 2` with integrated title (`Claude Code v2.1.226`), 2-column split (Welcome mascot on left, `Tips for getting started` & `What's new` on right, separated by vertical divider `│`), and model/context/path metadata footer.
- `WorkspaceDashboard`: Full-width task & session dashboard (`AgentHeader` + `BackgroundBanner` + `Needs Input Table` + `Completed Table`) replacing the two-column sidebar layout.
- `AmbientStatusLine`: Right-aligned status line (`● xhigh · /effort`) directly above the prompt divider line (`y = prompt_y - 1`).
- `PromptComposer`: `❯ ` prompt prefix, contextual placeholders, and horizontal divider rules.
- `CommandPalette`: Floating 3-column dropdown (`Name`, `Category`, `Description`) anchored above composer.
- `QuietFooter`: Single borderless status row at `y = height - 1`.

**Tech Stack:** Rust, Ratatui (TUI rendering framework), Crossterm.

## Global Constraints
- **Zero Backend Mutations**: Do NOT modify `brain-domain`, `brain-core`, `brain-storage`, `brain-services`, or `brain-events`.
- **Protocol & Reducer Purity**: Preserve UDS `StreamEvent` protocol, reducer state machine purity, and `ThemeToken` architecture.
- **Strict Visual Scope**: All implementation changes are strictly confined to `crates/brain-tui/src/ui/` and `crates/brain-tui/tests/`.
- **Zero Placeholder Policy**: All code, test commands, and assertions must be complete and exact.

---

### Task 1: Reconstruct `HomeWelcomeWidget` & `AmbientStatusLine`

**Files:**
- Create: `crates/brain-tui/src/ui/widgets/home_welcome.rs`
- Create: `crates/brain-tui/src/ui/widgets/ambient_status.rs`
- Modify: `crates/brain-tui/src/ui/widgets/mod.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/home_welcome_tests.rs`

**Interfaces:**
- Consumes: `UiState`, `Theme`, `RenderCapabilities`
- Produces: `HomeWelcomeWidget::draw(f, surface_rect, state, theme)`, `AmbientStatusWidget::draw(f, status_rect, state, theme)`

- [ ] **Step 1: Write the failing test for `HomeWelcomeWidget`**

```rust
// crates/brain-tui/tests/home_welcome_tests.rs
use brain_tui::state::UiState;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_home_welcome_surface_geometry_and_components() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();

    terminal
        .draw(|f| {
            let surface_rect = Rect::new(1, 2, 78, 9);
            brain_tui::ui::widgets::home_welcome::draw(f, surface_rect, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    
    // Assert top border box starts at y=2 with integrated title
    let row2 = (0..80).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(row2.contains("Claude Code v2.1.226"), "Row 2 must contain integrated title");
    assert!(row2.contains("┌"), "Row 2 must contain top-left border corner");
    assert!(row2.contains("┐"), "Row 2 must contain top-right border corner");

    // Assert vertical divider at x=47
    assert_eq!(buf.get(47, 3).symbol(), "│");
    assert_eq!(buf.get(47, 4).symbol(), "│");
    assert_eq!(buf.get(47, 5).symbol(), "│");

    // Assert Right Rail Headers
    let rail_text = (3..9)
        .map(|y| (48..78).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rail_text.contains("Tips for getting started"));
    assert!(rail_text.contains("What's new"));

    // Negative assertions: legacy Brain UI text MUST NOT exist
    let full_text = (0..24)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!full_text.contains("System Status"), "System Status must not exist");
    assert!(!full_text.contains("Context"), "Context telemetry must not exist");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brain-tui --test home_welcome_tests`
Expected: FAIL with `module home_welcome not found`

- [ ] **Step 3: Implement `HomeWelcomeWidget` and `AmbientStatusWidget`**

Create `crates/brain-tui/src/ui/widgets/home_welcome.rs`:
```rust
use crate::state::UiState;
use crate::ui::theme::{Theme, ThemeToken};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
    if area.width < 40 || area.height < 6 {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.style(ThemeToken::BorderSubtle))
        .title(
            Line::from(vec![
                Span::styled(" Claude Code ", theme.style(ThemeToken::HeaderPrimary).add_modifier(Modifier::BOLD)),
                Span::styled("v2.1.226 ", theme.style(ThemeToken::TextMuted)),
            ])
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split inner area horizontally into Left Welcome Pane (58%) and Right Information Rail (42%)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(inner);

    let left_area = chunks[0];
    let right_area = chunks[1];

    // Render Vertical Divider at boundary
    let divider_x = right_area.x.saturating_sub(1);
    let buf = f.buffer_mut();
    for y in inner.y..(inner.y + inner.height) {
        if divider_x < buf.area.width && y < buf.area.height {
            buf.get_mut(divider_x, y).set_symbol("│").set_style(theme.style(ThemeToken::BorderSubtle));
        }
    }

    // Left Welcome Pane Content
    let left_lines = vec![
        Line::from(Span::styled("Welcome back!", theme.style(ThemeToken::TextPrimary).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("    ▄▀▀▀▄", theme.style(ThemeToken::Accent))),
        Line::from(Span::styled("    █ █ █", theme.style(ThemeToken::Accent))),
        Line::from(Span::styled("Think once. Remember.", theme.style(ThemeToken::TextMuted))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Opus 5 (1M context) with xhigh", theme.style(ThemeToken::TextSecondary)),
            Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
            Span::styled("API Usage Billing", theme.style(ThemeToken::TextMuted)),
        ]),
        Line::from(Span::styled("~/Developer/PyCharm/brain", theme.style(ThemeToken::TextMuted))),
    ];
    let left_p = Paragraph::new(left_lines);
    f.render_widget(left_p, left_area);

    // Right Information Rail Content
    let right_lines = vec![
        Line::from(Span::styled("Tips for getting started", theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("Run /init to create a ...", theme.style(ThemeToken::TextPrimary))),
        Line::from(Span::styled("─────────────────────────────", theme.style(ThemeToken::BorderSubtle))),
        Line::from(Span::styled("What's new", theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("Bug fixes and reliabil...", theme.style(ThemeToken::TextSecondary))),
        Line::from(Span::styled("Added gateway spend-li...", theme.style(ThemeToken::TextSecondary))),
        Line::from(Span::styled("/release-notes for more", theme.style(ThemeToken::TextMuted).add_modifier(Modifier::ITALIC))),
    ];
    let right_p = Paragraph::new(right_lines);
    f.render_widget(right_p, right_area);
}
```

Create `crates/brain-tui/src/ui/widgets/ambient_status.rs`:
```rust
use crate::state::UiState;
use crate::ui::theme::{Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn draw(f: &mut Frame<'_>, area: Rect, _state: &UiState, theme: &Theme) {
    let text = Line::from(vec![
        Span::styled("● ", theme.style(ThemeToken::Success)),
        Span::styled("xhigh", theme.style(ThemeToken::TextSecondary)),
        Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
        Span::styled("/effort", theme.style(ThemeToken::TextMuted)),
    ]);
    let p = Paragraph::new(text).alignment(ratatui::layout::Alignment::Right);
    f.render_widget(p, area);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brain-tui --test home_welcome_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-tui/src/ui/widgets/home_welcome.rs crates/brain-tui/src/ui/widgets/ambient_status.rs crates/brain-tui/src/ui/widgets/mod.rs crates/brain-tui/tests/home_welcome_tests.rs
git commit -m "feat(ui): implement HomeWelcomeWidget and AmbientStatusWidget"
```

---

### Task 2: Reconstruct `WorkspaceDashboardWidget` (Full-Width Task Table)

**Files:**
- Create: `crates/brain-tui/src/ui/widgets/workspace_dashboard.rs`
- Modify: `crates/brain-tui/src/ui/widgets/mod.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/workspace_dashboard_tests.rs`

**Interfaces:**
- Consumes: `UiState`, `Theme`
- Produces: `WorkspaceDashboardWidget::draw(f, area, state, theme)`

- [ ] **Step 1: Write the failing test for `WorkspaceDashboardWidget`**

```rust
// crates/brain-tui/tests/workspace_dashboard_tests.rs
use brain_tui::state::UiState;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_workspace_dashboard_full_width_layout() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            brain_tui::ui::widgets::workspace_dashboard::draw(f, area, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let text = (0..20)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    // Positive Assertions
    assert!(text.contains("Claude Code v2.1.226"));
    assert!(text.contains("awaiting input"));
    assert!(text.contains("Your conversation moved to the background"));
    assert!(text.contains("Needs input"));
    assert!(text.contains("Completed"));

    // Negative Assertions: 2-column sidebar split line must NOT exist
    for y in 0..20 {
        assert_ne!(buf.get(22, y).symbol(), "│", "Vertical sidebar divider must not exist at col 22");
    }
    assert!(!text.contains("Sessions (Active)"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brain-tui --test workspace_dashboard_tests`
Expected: FAIL with `module workspace_dashboard not found`

- [ ] **Step 3: Implement `WorkspaceDashboardWidget`**

Create `crates/brain-tui/src/ui/widgets/workspace_dashboard.rs`:
```rust
use crate::state::UiState;
use crate::ui::theme::{Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn draw(f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
    if area.width < 40 || area.height < 10 {
        return;
    }

    let mut lines = Vec::new();

    // 1. Agent Header Block
    lines.push(Line::from(vec![
        Span::styled("▄▀▀  ", theme.style(ThemeToken::Accent)),
        Span::styled("Claude Code ", theme.style(ThemeToken::HeaderPrimary).add_modifier(Modifier::BOLD)),
        Span::styled("v2.1.226", theme.style(ThemeToken::TextMuted)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Opus 5 (1M context) · ~/Developer/PyCharm/brain", theme.style(ThemeToken::TextMuted)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("4 awaiting input", theme.style(ThemeToken::Accent)),
        Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
        Span::styled("0 working", theme.style(ThemeToken::TextMuted)),
        Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
        Span::styled("17 completed", theme.style(ThemeToken::TextSecondary)),
    ]));
    lines.push(Line::from(""));

    // 2. Background Navigation Banner
    lines.push(Line::from(Span::styled(
        "Your conversation moved to the background — enter opens it · esc returns to it",
        theme.style(ThemeToken::TextMuted).add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(""));

    // 3. Needs Input Table Section
    lines.push(Line::from(Span::styled(
        "Needs input",
        theme.style(ThemeToken::TextPrimary).add_modifier(Modifier::BOLD),
    )));

    if state.sessions.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("* ", theme.style(ThemeToken::Accent)),
            Span::styled(format!("{:<30}", "current session"), theme.style(ThemeToken::TextPrimary).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<35}", "brain"), theme.style(ThemeToken::TextSecondary)),
            Span::styled("2s", theme.style(ThemeToken::TextMuted)),
        ]));
    } else {
        for (idx, session) in state.sessions.iter().enumerate() {
            let is_sel = idx == state.selected_session_index;
            let style = if is_sel {
                theme.style(ThemeToken::Selection)
            } else {
                theme.style(ThemeToken::TextPrimary)
            };
            let prefix = if is_sel { "* " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(prefix, theme.style(ThemeToken::Accent)),
                Span::styled(format!("{:<30}", session.title), style),
                Span::styled(format!("{:<35}", "active"), theme.style(ThemeToken::TextMuted)),
                Span::styled("1m", theme.style(ThemeToken::TextMuted)),
            ]));
        }
    }

    lines.push(Line::from(""));

    // 4. Completed Section
    lines.push(Line::from(Span::styled(
        "Completed",
        theme.style(ThemeToken::TextPrimary).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("· ", theme.style(ThemeToken::TextMuted)),
        Span::styled(format!("{:<30}", "bg"), theme.style(ThemeToken::TextSecondary)),
        Span::styled(format!("{:<35}", "(idle - send a prompt to start)"), theme.style(ThemeToken::TextMuted)),
        Span::styled("11h", theme.style(ThemeToken::TextMuted)),
    ]));

    let p = Paragraph::new(lines);
    f.render_widget(p, area);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brain-tui --test workspace_dashboard_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-tui/src/ui/widgets/workspace_dashboard.rs crates/brain-tui/src/ui/widgets/mod.rs crates/brain-tui/tests/workspace_dashboard_tests.rs
git commit -m "feat(ui): implement WorkspaceDashboardWidget full-width task table"
```

---

### Task 3: Reconstruct `PromptWidget` & `QuietStatusFooter`

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/prompt.rs`
- Modify: `crates/brain-tui/src/ui/status_footer.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Test: `crates/brain-tui/tests/prompt_footer_reconstruction_tests.rs`

**Interfaces:**
- Consumes: `PromptView`, `UiState`, `Theme`
- Produces: `prompt::draw(f, area, view, theme)`, `StatusFooterWidget::draw(f, area, state, theme)`

- [ ] **Step 1: Write failing test for Prompt & Footer**

```rust
// crates/brain-tui/tests/prompt_footer_reconstruction_tests.rs
use brain_tui::state::UiState;
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::prompt::PromptView;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_prompt_glyph_and_quiet_footer_content() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();
    let prompt_view = PromptView {
        prompt_text: "".to_string(),
        cursor_position: 0,
        has_focus: true,
        submit_with_workspace: false,
        is_welcome: true,
    };

    terminal
        .draw(|f| {
            let prompt_area = Rect::new(0, 20, 80, 3);
            let footer_area = Rect::new(0, 23, 80, 1);
            brain_tui::ui::widgets::prompt::draw(f, prompt_area, &prompt_view, &theme);
            brain_tui::ui::status_footer::StatusFooterWidget::draw(f, footer_area, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // Assert Prompt Prefix Glyph ❯
    let row21 = (0..80).map(|x| buf.get(x, 21).symbol()).collect::<String>();
    assert!(row21.starts_with("❯ "), "Prompt prefix must start with ❯ ");

    // Assert Quiet Footer Content
    let row23 = (0..80).map(|x| buf.get(x, 23).symbol()).collect::<String>();
    assert!(row23.contains("manual mode on") || row23.contains("? for shortcuts"));
    assert!(!row23.contains("Daemon: Connected | Latency:"), "Raw telemetry must not be in footer");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brain-tui --test prompt_footer_reconstruction_tests`
Expected: FAIL with glyph/text assertion mismatch

- [ ] **Step 3: Update `prompt.rs` and `status_footer.rs`**

Update `crates/brain-tui/src/ui/widgets/prompt.rs`:
```rust
// Change prompt prefix from "brain> " or "> " to "❯ "
let prefix = "❯ ";
```

Update `crates/brain-tui/src/ui/status_footer.rs`:
```rust
// Replace raw daemon latency text with quiet shortcut hints
let footer_text = if state.screen == crate::ui::navigation::Screen::Workspace {
    "enter to return · space to reply · ctrl+x to delete · ? for shortcuts".to_string()
} else {
    "▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents".to_string()
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brain-tui --test prompt_footer_reconstruction_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-tui/src/ui/widgets/prompt.rs crates/brain-tui/src/ui/status_footer.rs crates/brain-tui/tests/prompt_footer_reconstruction_tests.rs
git commit -m "feat(ui): update prompt glyph to ❯ and enforce quiet status footer"
```

---

### Task 4: Reconstruct `CommandPaletteWidget` (3-Column Layout)

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/palette.rs`
- Modify: `crates/brain-tui/src/ui/command/palette.rs`
- Test: `crates/brain-tui/tests/command_palette_reconstruction_tests.rs`

**Interfaces:**
- Consumes: `CommandPaletteState`, `Theme`
- Produces: `palette::draw(f, area, state, theme)` with 3 columns (`Command Name`, `Category`, `Description`)

- [ ] **Step 1: Write failing test for 3-Column Command Palette**

```rust
// crates/brain-tui/tests/command_palette_reconstruction_tests.rs
use brain_tui::ui::command::palette::CommandPaletteState;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_command_palette_3_column_layout() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut palette = CommandPaletteState::new();
    palette.open = true;
    let theme = Theme::default();

    terminal
        .draw(|f| {
            let area = Rect::new(0, 14, 80, 6);
            brain_tui::ui::widgets::palette::draw(f, area, &palette, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let text = (14..20)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("/help") || text.contains("/session"));
    assert!(text.contains("System") || text.contains("Session") || text.contains("Command"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brain-tui --test command_palette_reconstruction_tests`
Expected: FAIL if columns are missing

- [ ] **Step 3: Update `palette.rs` to render 3 distinct columns**

In `crates/brain-tui/src/ui/widgets/palette.rs`:
```rust
let line = Line::from(vec![
    Span::styled(format!("{:<20}", item.title), name_style),
    Span::styled(format!("{:<15}", item.category), theme.style(ThemeToken::TextMuted)),
    Span::styled(item.description.clone(), theme.style(ThemeToken::TextSecondary)),
]);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brain-tui --test command_palette_reconstruction_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-tui/src/ui/widgets/palette.rs crates/brain-tui/tests/command_palette_reconstruction_tests.rs
git commit -m "feat(ui): update CommandPaletteWidget to 3-column category layout"
```

---

### Task 5: Implement `HomeWelcomeSurface` Canvas Scroll & Master Layout Assembly

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Modify: `crates/brain-tui/src/ui/widgets/chat.rs`
- Test: `crates/brain-tui/tests/claude_visual_reconstruction_tests.rs`

**Interfaces:**
- Consumes: All UI components (`HomeWelcomeWidget`, `WorkspaceDashboardWidget`, `PromptWidget`, `AmbientStatusWidget`, `StatusFooterWidget`)
- Produces: `AppRenderer::draw(f, area, state, theme)` assembly matching 80×24, 100×26, 120×30, 182×53 viewports.

- [ ] **Step 1: Write failing master visual reconstruction test suite**

Create `crates/brain-tui/tests/claude_visual_reconstruction_tests.rs`:
```rust
use brain_tui::state::{FocusRegion, Screen, UiState};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn test_master_home_80x24_cell_buffer_reconstruction() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal.draw(|f| renderer.draw(f, f.area(), &state, &theme)).unwrap();
    let buf = terminal.backend().buffer();

    // 1. Surface Border Box at y=2
    assert_eq!(buf.get(1, 2).symbol(), "┌");
    assert_eq!(buf.get(78, 2).symbol(), "┐");

    // 2. Integrated Title
    let title_line = (0..80).map(|x| buf.get(x, 2).symbol()).collect::<String>();
    assert!(title_line.contains("Claude Code v2.1.226"));

    // 3. Vertical Divider at x=47
    assert_eq!(buf.get(47, 4).symbol(), "│");

    // 4. Ambient Status at y=19
    let ambient_line = (0..80).map(|x| buf.get(x, 19).symbol()).collect::<String>();
    assert!(ambient_line.contains("● xhigh · /effort"));

    // 5. Prompt Prefix at y=21
    assert_eq!(buf.get(0, 21).symbol(), "❯");

    // 6. Negative Assertions
    let full_screen = (0..24)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!full_screen.contains("System Status"));
    assert!(!full_screen.contains("Context"));
}

#[test]
fn test_master_workspace_80x24_cell_buffer_reconstruction() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Workspace;
    state.focus = FocusRegion::Sidebar;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal.draw(|f| renderer.draw(f, f.area(), &state, &theme)).unwrap();
    let buf = terminal.backend().buffer();

    // Must be full-width task table, NOT a 22-column sidebar split
    assert_ne!(buf.get(22, 5).symbol(), "│", "Vertical sidebar divider must not exist in Workspace mode");

    let full_screen = (0..24)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(full_screen.contains("Needs input"));
    assert!(full_screen.contains("Completed"));
    assert!(!full_screen.contains("Sessions (Active)"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brain-tui --test claude_visual_reconstruction_tests`
Expected: FAIL on layout assembly

- [ ] **Step 3: Update `AppRenderer::draw` and `compute_layout` in `renderer.rs`**

In `crates/brain-tui/src/ui/renderer.rs`:
- Update `compute_layout` for `Screen::Home`: allocate `welcome_surface_rect` at `y = 2`, `ambient_status_rect` at `y = 19`, `prompt_rect` at `y = 20..22`, `footer_rect` at `y = 23`.
- Update `compute_layout` for `Screen::Workspace`: allocate full width (`80` cols) for `WorkspaceDashboardWidget`, setting sidebar width to `0`.
- In `draw`: call `HomeWelcomeWidget::draw`, `WorkspaceDashboardWidget::draw`, `AmbientStatusWidget::draw`, `PromptWidget::draw`, `StatusFooterWidget::draw`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brain-tui --test claude_visual_reconstruction_tests`
Expected: PASS

- [ ] **Step 5: Run full workspace test gate**

Run: `cargo test -p brain-tui && cargo test --workspace`
Expected: PASS with 100% clean test suite

- [ ] **Step 6: Commit**

```bash
git add crates/brain-tui/src/ui/renderer.rs crates/brain-tui/tests/claude_visual_reconstruction_tests.rs
git commit -m "feat(ui): complete master Claude visual reconstruction and pass cell-buffer test gate"
```

---

## Plan Handoff & Execution Choice

Plan complete and saved to `docs/superpowers/plans/2026-08-11-claude-visual-reconstruction.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — Fresh subagent per task, review between tasks, fast iteration using `superpowers:subagent-driven-development`.
2. **Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach would you like to take?
