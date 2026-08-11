//! Visual snapshot integration tests — renders each scenario at every requested
//! terminal size and writes human-readable text grids to the artifacts directory.
//!
//! Run with:
//!   cargo test -p brain-tui --test visual_snapshots -- --nocapture
//!
//! Output files are written to: /tmp/brain_snapshots/

use brain_tui::state::{Action, ConnectionMode, FocusRegion, UiState};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::fs;
use std::path::PathBuf;

const OUT_DIR: &str = "target/snapshots";

/// Extracts the buffer contents as a grid of lines (plain symbols, no ANSI).
fn buffer_to_lines(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buf = terminal.backend().buffer();
    let w = buf.area().width as usize;
    let h = buf.area().height as usize;
    let cells = buf.content();

    (0..h)
        .map(|row| {
            let line: String = cells[row * w..(row + 1) * w]
                .iter()
                .map(|c| {
                    let s = c.symbol();
                    if s.trim().is_empty() {
                        " "
                    } else {
                        s
                    }
                })
                .collect();
            // right-trim trailing spaces for readability
            line.trim_end().to_string()
        })
        .collect()
}

/// Formats lines into a ruler-decorated text block for easy human inspection.
fn format_snapshot(label: &str, w: u16, h: u16, lines: &[String]) -> String {
    let ruler_top = format!(
        "╔═══ {} @ {}×{} {}═╗",
        label,
        w,
        h,
        "═".repeat((w as usize).saturating_sub(label.len() + 12 + format!("{}×{}", w, h).len()))
    );
    let ruler_bot = format!("╚{}╝", "═".repeat(w as usize));
    let body: String = lines
        .iter()
        .map(|l| {
            // pad to width so grid is rectangular
            format!("│{:<width$}│", l, width = w as usize)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n{}\n{}\n", ruler_top, body, ruler_bot)
}

/// Renders a single scenario to the test backend and writes the text grid.
fn snapshot(label: &str, state: &UiState, w: u16, h: u16) {
    let theme = dark_theme();
    let renderer = AppRenderer::new();

    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            renderer.draw(f, Rect::new(0, 0, w, h), state, theme);
        })
        .unwrap();

    let lines = buffer_to_lines(&terminal);
    let output = format_snapshot(label, w, h, &lines);

    let dir = PathBuf::from(OUT_DIR);
    fs::create_dir_all(&dir).unwrap();

    let artifact_dir = PathBuf::from("/Users/ritikpathania/.gemini/antigravity/brain/54e6a8e0-61d1-4873-a558-ce34a6f18907/snapshots");
    fs::create_dir_all(&artifact_dir).unwrap();

    // sanitize label for filename
    let fname = label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let filename = format!("{}__{w}x{h}.txt", fname);
    let path = dir.join(&filename);
    fs::write(&path, &output).unwrap();
    let artifact_path = artifact_dir.join(&filename);
    fs::write(&artifact_path, &output).unwrap();
    println!("{}", output);
}

/// Builds a freshly-connected Home state (no messages).
fn home_state() -> UiState {
    let mut s = UiState::default();
    s.screen = brain_tui::ui::navigation::Screen::Home;
    s.update(Action::SetConnectionMode(ConnectionMode::Daemon));
    s
}

/// Builds a Workspace state with task dashboard.
fn workspace_state() -> UiState {
    use brain_domain::ulid::Ulid;
    use brain_domain::{Message, MessageId, MessageRole};

    let mut s = UiState::default();
    s.screen = brain_tui::ui::navigation::Screen::Workspace;
    s.update(Action::SetConnectionMode(ConnectionMode::Daemon));

    let user_msg = Message::new(
        MessageId(Ulid::new()),
        MessageRole::User,
        "What is the Rust ownership model and how does it prevent memory leaks?".to_string(),
    );
    let assistant_msg = Message::new(
        MessageId(Ulid::new()),
        MessageRole::Assistant,
        "Rust's ownership model is a set of compile-time rules that govern how memory is \
         managed without a garbage collector. Every value has exactly one owner. When the \
         owner goes out of scope, the value is dropped and memory is freed automatically. \
         Borrowing rules prevent data races: at most one mutable reference OR any number of \
         immutable references can exist at a time.\n\nThis means memory leaks, use-after-free, \
         and double-free bugs are caught at compile time rather than at runtime."
            .to_string(),
    );

    s.active_messages.push(user_msg);
    s.active_messages.push(assistant_msg);
    s
}

/// Builds a Workspace state mid-stream (no completed assistant message yet, typewriter active).
fn streaming_state() -> UiState {
    use brain_domain::ulid::Ulid;
    use brain_domain::{Message, MessageId, MessageRole};

    let mut s = UiState::default();
    s.screen = brain_tui::ui::navigation::Screen::Workspace;
    s.update(Action::SetConnectionMode(ConnectionMode::Daemon));

    let user_msg = Message::new(
        MessageId(Ulid::new()),
        MessageRole::User,
        "Explain how Brain stores relational memories.".to_string(),
    );
    s.active_messages.push(user_msg);
    s.update(Action::StartStream);
    for word in [
        "Brain", " uses", " a", " SQLite", " FTS5", " +", " vector", " index",
    ] {
        s.update(Action::ReceiveToken(brain_tui::state::RenderToken::Text(
            word.to_string(),
        )));
    }
    s
}

/// Builds a Workspace state with retrieval evidence blocks.
fn evidence_state() -> UiState {
    use brain_domain::ulid::Ulid;
    use brain_domain::{Message, MessageId, MessageRole};

    let mut s = UiState::default();
    s.screen = brain_tui::ui::navigation::Screen::Workspace;
    s.update(Action::SetConnectionMode(ConnectionMode::Daemon));

    let user_msg = Message::new(
        MessageId(Ulid::new()),
        MessageRole::User,
        "What do I know about async Rust?".to_string(),
    );
    s.active_messages.push(user_msg);
    s
}

// ─── Test entry points ────────────────────────────────────────────────────────

const SIZES: &[(u16, u16)] = &[(80, 24), (96, 24), (120, 30), (156, 52), (182, 53)];

#[test]
fn snapshot_01_home() {
    let state = home_state();
    for &(w, h) in SIZES {
        snapshot("01-home", &state, w, h);
    }
}

#[test]
fn snapshot_02_home_focused_prompt() {
    let mut state = home_state();
    state.focus = FocusRegion::Editor;
    for &(w, h) in SIZES {
        snapshot("02-home-focused-prompt", &state, w, h);
    }
}

#[test]
fn snapshot_03_slash_completion() {
    let mut state = home_state();
    state.focus = FocusRegion::Editor;
    state.update(Action::InsertChar('/'));
    for &(w, h) in SIZES {
        snapshot("03-slash-completion", &state, w, h);
    }
}

#[test]
fn snapshot_04_ctrl_k_palette() {
    let mut state = home_state();
    state.focus = FocusRegion::Editor;
    state.command_palette_mut().open_with_query(None);
    for &(w, h) in SIZES {
        snapshot("04-ctrl-k-palette", &state, w, h);
    }
}

#[test]
fn snapshot_05_slash_theme() {
    let mut state = home_state();
    state.focus = FocusRegion::Editor;
    state.update(Action::InsertChar('/'));
    state.update(Action::InsertChar('t'));
    state.update(Action::InsertChar('h'));
    state.update(Action::InsertChar('e'));
    state.update(Action::InsertChar('m'));
    state.update(Action::InsertChar('e'));
    for &(w, h) in SIZES {
        snapshot("05-slash-theme", &state, w, h);
    }
}

#[test]
fn snapshot_06_workspace_query() {
    let state = workspace_state();
    for &(w, h) in SIZES {
        snapshot("06-workspace-query", &state, w, h);
    }
}

#[test]
fn snapshot_07_workspace_streaming() {
    let state = streaming_state();
    for &(w, h) in SIZES {
        snapshot("07-workspace-streaming", &state, w, h);
    }
}

#[test]
fn snapshot_08_workspace_evidence() {
    let state = evidence_state();
    for &(w, h) in SIZES {
        snapshot("08-workspace-evidence", &state, w, h);
    }
}

#[test]
fn snapshot_09_slash_status() {
    let mut state = home_state();
    state.focus = FocusRegion::Editor;
    state.update(Action::InsertChar('/'));
    state.update(Action::InsertChar('s'));
    state.update(Action::InsertChar('t'));
    state.update(Action::InsertChar('a'));
    state.update(Action::InsertChar('t'));
    state.update(Action::InsertChar('u'));
    state.update(Action::InsertChar('s'));
    for &(w, h) in SIZES {
        snapshot("09-slash-status", &state, w, h);
    }
}
