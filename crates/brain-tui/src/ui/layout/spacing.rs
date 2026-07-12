//! Semantic spacing tokens resolved to cell sizes.

/// Spacing categories mapped to layout margins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
    /// No spacing (0 cells).
    None,
    /// Tight spacing (1 cell).
    Compact,
    /// Standard spacing (2 cells).
    Normal,
    /// Relaxed spacing (4 cells).
    Wide,
}

impl Spacing {
    /// Resolves spacing categories to cell offsets.
    pub fn cells(self) -> u16 {
        match self {
            Spacing::None => 0,
            Spacing::Compact => 1,
            Spacing::Normal => 2,
            Spacing::Wide => 4,
        }
    }
}
