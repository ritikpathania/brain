//! Modal overlay enum targets.

/// Overlay modal dialog targets mounted on top of active screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modal {
    /// Searchable Command Palette.
    CommandPalette,
    /// Interactive Help & Documentation overlay.
    Help,
    /// Theme Selector with live preview capability.
    ThemePicker,
    /// Destructive confirmation dialog.
    ConfirmDelete,
    /// Fast-path reply composition dialog.
    ReplyComposer,
    /// Generic Document Inspector modal overlay.
    DocumentInspector,
}

impl Modal {
    /// Returns human-readable modal drawer title.
    pub fn title(self) -> &'static str {
        match self {
            Modal::CommandPalette => "Command Palette",
            Modal::Help => "Help & Shortcuts",
            Modal::ThemePicker => "Select Theme",
            Modal::ConfirmDelete => "Confirm Action",
            Modal::ReplyComposer => "Reply Composer",
            Modal::DocumentInspector => "Document Inspector",
        }
    }
}
