use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl CellSpec {
    pub fn empty() -> Self {
        Self {
            symbol: " ".to_string(),
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underlined: false,
            dim: false,
            reversed: false,
        }
    }

    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underlined: false,
            dim: false,
            reversed: false,
        }
    }
}

pub fn inspect_cell_buf(buf: &Buffer, x: u16, y: u16) -> CellSpec {
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

pub fn inspect_cell(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> CellSpec {
    let buf = terminal.backend().buffer();
    inspect_cell_buf(buf, x, y)
}

pub fn assert_cell_grid_eq(
    actual: &Terminal<TestBackend>,
    expected: &[Vec<CellSpec>],
    w: u16,
    h: u16,
) {
    let buf = actual.backend().buffer();
    assert_cell_buf_grid_eq(buf, expected, w, h);
}

pub fn assert_cell_buf_grid_eq(
    actual: &Buffer,
    expected: &[Vec<CellSpec>],
    w: u16,
    h: u16,
) {
    let mut mismatches = Vec::new();
    for y in 0..h {
        let row_idx = y as usize;
        for x in 0..w {
            let col_idx = x as usize;
            let act = inspect_cell_buf(actual, x, y);
            let exp = if row_idx < expected.len() && col_idx < expected[row_idx].len() {
                &expected[row_idx][col_idx]
            } else {
                panic!(
                    "Expected grid matrix does not match viewport dimensions {}x{} (row {}, col {})",
                    w, h, row_idx, col_idx
                );
            };
            if &act != exp {
                mismatches.push((x, y, exp.clone(), act));
            }
        }
    }
    if !mismatches.is_empty() {
        let mut msg = format!(
            "\nVisual Oracle Failure at Viewport {}x{} [Mismatch Count: {}]\n",
            w,
            h,
            mismatches.len()
        );
        for (x, y, exp, act) in mismatches.iter().take(20) {
            msg.push_str(&format!(
                "Coordinate ({}, {}):\n  Expected: {:?}\n  Actual:   {:?}\n",
                x, y, exp, act
            ));
        }
        panic!("{}", msg);
    }
}

pub fn load_reference_fixture(relative_path: &str) -> Vec<Vec<CellSpec>> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = std::path::Path::new(&manifest_dir).join("tests").join(relative_path);
    let content = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("Failed to read reference fixture at {:?}: {}", path, err);
    });
    serde_json::from_str(&content).unwrap_or_else(|err| {
        panic!("Failed to deserialize reference fixture JSON at {:?}: {}", path, err);
    })
}
