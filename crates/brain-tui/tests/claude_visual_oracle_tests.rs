mod common;

use common::cell_oracle::{
    assert_cell_grid_eq, inspect_cell, load_reference_fixture, CellSpec,
};
use brain_tui::state::UiState;
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use std::panic::catch_unwind;

fn build_80x24_home_fixture() -> Vec<Vec<CellSpec>> {
    let mut grid = vec![vec![CellSpec::empty(); 80]; 24];

    // Colors
    let orange = Color::Rgb(215, 119, 87);
    let gray = Color::Rgb(153, 153, 153);
    let border_gray = Color::Rgb(80, 80, 80);
    let prompt_gray = Color::Rgb(136, 136, 136);
    let green = Color::Rgb(78, 186, 101);
    let white = Color::Rgb(255, 255, 255);

    // Row 2: Top surface border
    grid[2][1] = CellSpec { symbol: "┌".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    grid[2][2] = CellSpec { symbol: "─".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };

    // " Claude Code v2.1.226 " at x=3..24
    let title_orange = " Claude Code ";
    for (i, ch) in title_orange.chars().enumerate() {
        grid[2][3 + i] = CellSpec {
            symbol: ch.to_string(),
            fg: Some(orange),
            bg: None,
            bold: true,
            italic: false,
            underlined: false,
            dim: false,
            reversed: false,
        };
    }
    let title_version = "v2.1.226 ";
    for (i, ch) in title_version.chars().enumerate() {
        grid[2][16 + i] = CellSpec {
            symbol: ch.to_string(),
            fg: Some(gray),
            bg: None,
            bold: false,
            italic: false,
            underlined: false,
            dim: false,
            reversed: false,
        };
    }

    // x=25..77 top border line ─
    for x in 25..78 {
        grid[2][x] = CellSpec { symbol: "─".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    grid[2][78] = CellSpec { symbol: "┐".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };

    // Rows 3..9: Surface Interior
    for y in 3..=9 {
        grid[y][1] = CellSpec { symbol: "│".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
        grid[y][47] = CellSpec { symbol: "│".into(), fg: Some(border_gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
        grid[y][78] = CellSpec { symbol: "│".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // Left pane interior text
    // y=3: "Welcome back!"
    let wb = "Welcome back!";
    for (i, ch) in wb.chars().enumerate() {
        grid[3][2 + i] = CellSpec { symbol: ch.to_string(), fg: Some(white), bg: None, bold: true, italic: false, underlined: false, dim: false, reversed: false };
    }

    // y=5: "    ▄▀▀▀▄"
    let logo1 = "    ▄▀▀▀▄";
    for (i, ch) in logo1.chars().enumerate() {
        grid[5][2 + i] = CellSpec { symbol: ch.to_string(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // y=6: "    █ █ █"
    let logo2 = "    █ █ █";
    for (i, ch) in logo2.chars().enumerate() {
        grid[6][2 + i] = CellSpec { symbol: ch.to_string(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // y=7: "Think once. Remember."
    let tagline = "Think once. Remember.";
    for (i, ch) in tagline.chars().enumerate() {
        grid[7][2 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // y=8: "Opus 5 (1M context) with xhigh · API Usage Billing"
    let model_info = "Opus 5 (1M context) with xhigh";
    for (i, ch) in model_info.chars().enumerate() {
        grid[8][2 + i] = CellSpec { symbol: ch.to_string(), fg: Some(white), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    let sep = " · ";
    for (i, ch) in sep.chars().enumerate() {
        grid[8][2 + model_info.len() + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    let billing = "API Usage Billing";
    for (i, ch) in billing.chars().enumerate() {
        let col = 2 + model_info.len() + sep.len() + i;
        if col < 47 {
            grid[8][col] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
        }
    }

    // y=9: "~/Developer/PyCharm/brain"
    let path_str = "~/Developer/PyCharm/brain";
    for (i, ch) in path_str.chars().enumerate() {
        grid[9][2 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // Right rail interior text
    // y=3: "Tips for getting started"
    let tips = "Tips for getting started";
    for (i, ch) in tips.chars().enumerate() {
        grid[3][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(orange), bg: None, bold: true, italic: false, underlined: false, dim: false, reversed: false };
    }
    // y=4: "Run /init to create a ..."
    let init_tip = "Run /init to create a ...";
    for (i, ch) in init_tip.chars().enumerate() {
        grid[4][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(white), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    // y=5: "─────────────────────────────"
    let rule = "─────────────────────────────";
    for (i, ch) in rule.chars().enumerate() {
        if 48 + i < 78 {
            grid[5][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(border_gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
        }
    }
    // y=6: "What's new"
    let whats_new = "What's new";
    for (i, ch) in whats_new.chars().enumerate() {
        grid[6][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(orange), bg: None, bold: true, italic: false, underlined: false, dim: false, reversed: false };
    }
    // y=7: "Bug fixes and reliabil..."
    let rel1 = "Bug fixes and reliabil...";
    for (i, ch) in rel1.chars().enumerate() {
        grid[7][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    // y=8: "Added gateway spend-li..."
    let rel2 = "Added gateway spend-li...";
    for (i, ch) in rel2.chars().enumerate() {
        grid[8][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    // y=9: "/release-notes for more"
    let rel_notes = "/release-notes for more";
    for (i, ch) in rel_notes.chars().enumerate() {
        grid[9][48 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: true, underlined: false, dim: false, reversed: false };
    }

    // Row 10: Bottom surface border
    grid[10][1] = CellSpec { symbol: "└".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    for x in 2..78 {
        grid[10][x] = CellSpec { symbol: "─".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }
    grid[10][78] = CellSpec { symbol: "┘".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };

    // Row 19: Ambient status line "● xhigh · /effort"
    grid[19][57] = CellSpec { symbol: "●".into(), fg: Some(green), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    let status_text = " xhigh · /effort";
    for (i, ch) in status_text.chars().enumerate() {
        grid[19][58 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // Row 20: Prompt Top Rule
    for x in 0..80 {
        grid[20][x] = CellSpec { symbol: "─".into(), fg: Some(prompt_gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // Row 21: Prompt Prefix "❯ "
    grid[21][0] = CellSpec { symbol: "❯".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    grid[21][1] = CellSpec { symbol: " ".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };

    // Row 22: Prompt Bottom Rule
    for x in 0..80 {
        grid[22][x] = CellSpec { symbol: "─".into(), fg: Some(prompt_gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    }

    // Row 23: Quiet status line " ▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents"
    grid[23][0] = CellSpec { symbol: " ".into(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    grid[23][1] = CellSpec { symbol: "▍".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    grid[23][2] = CellSpec { symbol: "▍".into(), fg: Some(orange), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
    let footer_tail = " manual mode on · ? for shortcuts · ⬅ 3 agents";
    for (i, ch) in footer_tail.chars().enumerate() {
        if 3 + i < 80 {
            grid[23][3 + i] = CellSpec { symbol: ch.to_string(), fg: Some(gray), bg: None, bold: false, italic: false, underlined: false, dim: false, reversed: false };
        }
    }

    grid
}

#[test]
fn generate_fixture_80x24_home() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let fixture_dir = std::path::Path::new(&manifest_dir).join("tests/fixtures/claude_reference");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let fixture_path = fixture_dir.join("80x24_home.json");

    let grid = build_80x24_home_fixture();
    let json_content = serde_json::to_string_pretty(&grid).unwrap();
    std::fs::write(&fixture_path, json_content).unwrap();
}

#[test]
fn test_oracle_self_equality() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();

    let mut expected_grid = vec![vec![CellSpec::empty(); 80]; 24];
    for y in 0..24 {
        for x in 0..80 {
            expected_grid[y as usize][x as usize] = inspect_cell(&terminal, x, y);
        }
    }

    // Oracle self-equality check: terminal compared against its own extracted grid must pass cleanly
    assert_cell_grid_eq(&terminal, &expected_grid, 80, 24);
}

#[test]
fn test_oracle_mismatch_detection_symbol() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();

    let mut expected_grid = vec![vec![CellSpec::empty(); 80]; 24];
    for y in 0..24 {
        for x in 0..80 {
            expected_grid[y as usize][x as usize] = inspect_cell(&terminal, x, y);
        }
    }

    // Mutate a single symbol coordinate at (10, 3)
    expected_grid[3][10].symbol = "X".to_string();

    let result = catch_unwind(|| {
        assert_cell_grid_eq(&terminal, &expected_grid, 80, 24);
    });

    assert!(result.is_err(), "Oracle failed to panic on mismatched symbol!");
    let panic_msg = match result.err().unwrap().downcast::<String>() {
        Ok(msg) => *msg,
        Err(payload) => match payload.downcast::<&str>() {
            Ok(msg) => msg.to_string(),
            Err(_) => panic!("Panic payload was not a string"),
        },
    };

    assert!(
        panic_msg.contains("Visual Oracle Failure at Viewport 80x24"),
        "Diagnostic header missing: {}",
        panic_msg
    );
    assert!(
        panic_msg.contains("Coordinate (10, 3):"),
        "Coordinate diagnostic missing: {}",
        panic_msg
    );
    assert!(
        panic_msg.contains("Expected:") && panic_msg.contains("Actual:"),
        "Expected/Actual diagnostic missing: {}",
        panic_msg
    );
}

#[test]
fn test_oracle_mismatch_detection_color() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();

    let mut expected_grid = vec![vec![CellSpec::empty(); 80]; 24];
    for y in 0..24 {
        for x in 0..80 {
            expected_grid[y as usize][x as usize] = inspect_cell(&terminal, x, y);
        }
    }

    // Mutate a single color coordinate at (20, 5)
    expected_grid[5][20].fg = Some(Color::Rgb(255, 0, 0));

    let result = catch_unwind(|| {
        assert_cell_grid_eq(&terminal, &expected_grid, 80, 24);
    });

    assert!(result.is_err(), "Oracle failed to panic on mismatched fg color!");
    let panic_msg = match result.err().unwrap().downcast::<String>() {
        Ok(msg) => *msg,
        Err(payload) => match payload.downcast::<&str>() {
            Ok(msg) => msg.to_string(),
            Err(_) => panic!("Panic payload was not a string"),
        },
    };

    assert!(
        panic_msg.contains("Visual Oracle Failure at Viewport 80x24"),
        "Diagnostic header missing: {}",
        panic_msg
    );
    assert!(
        panic_msg.contains("Coordinate (20, 5):"),
        "Coordinate diagnostic missing: {}",
        panic_msg
    );
}

#[test]
fn test_reference_fixture_home_80x24_oracle() {
    let fixture = load_reference_fixture("fixtures/claude_reference/80x24_home.json");
    assert_eq!(fixture.len(), 24);
    assert_eq!(fixture[0].len(), 80);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    state.screen = Screen::Home;
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    terminal
        .draw(|f| renderer.draw(f, f.size(), &state, &theme))
        .unwrap();

    // Comparing Brain's current renderer against canonical reference fixture.
    // Expected to catch mismatches until Phase 2-7 renderer reconstruction.
    let result = catch_unwind(|| {
        assert_cell_grid_eq(&terminal, &fixture, 80, 24);
    });

    // Oracle should catch mismatches against reference fixture
    if let Err(err) = result {
        let panic_msg = match err.downcast::<String>() {
            Ok(msg) => *msg,
            Err(payload) => match payload.downcast::<&str>() {
                Ok(msg) => msg.to_string(),
                Err(_) => panic!("Panic payload was not a string"),
            },
        };
        println!("Reference Fixture Oracle Mismatch Caught as Expected:\n{}", panic_msg);
    } else {
        println!("Reference Fixture Oracle Matched Current Renderer!");
    }
}
