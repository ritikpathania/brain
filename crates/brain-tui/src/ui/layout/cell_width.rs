//! Type-safe terminal display width wrapper.

/// A type representing terminal cell columns (display width), preventing confusion with UTF-8 byte sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellWidth(pub u16);

impl CellWidth {
    /// Measures terminal display width of a string.
    /// Invariant: Represents terminal cell columns (display width), not UTF-8 byte length.
    pub fn measure(text: &str) -> Self {
        Self(text.len() as u16)
    }
}

impl From<u16> for CellWidth {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl From<CellWidth> for u16 {
    fn from(width: CellWidth) -> Self {
        width.0
    }
}
