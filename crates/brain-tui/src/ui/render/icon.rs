//! Icon mapping helpers for Unicode and ASCII terminals.

/// A set of icons for consistent display across terminals.
pub struct IconSet {
    unicode: bool,
}

impl IconSet {
    /// Creates a new IconSet instance configured for the given Unicode support setting.
    pub fn new(unicode: bool) -> Self {
        Self { unicode }
    }

    /// Returns a checkmark icon.
    pub fn check(&self) -> &'static str {
        if self.unicode {
            "✓"
        } else {
            "[OK]"
        }
    }

    /// Returns a cross/error icon.
    pub fn cross(&self) -> &'static str {
        if self.unicode {
            "✗"
        } else {
            "[ERR]"
        }
    }

    /// Returns a folder icon.
    pub fn folder(&self) -> &'static str {
        if self.unicode {
            "📁"
        } else {
            "[DIR]"
        }
    }
}
