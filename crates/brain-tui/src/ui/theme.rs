use ratatui::style::{Color, Modifier, Style};

/// Semantic theme definition carrying visual styles for the TUI elements.
pub struct Theme {
    /// Primary highlight color (e.g. Claude orange).
    pub primary: Style,
    /// Secondary accent color.
    pub accent: Style,
    /// Success indicator style.
    pub success: Style,
    /// Warning indicator style.
    pub warning: Style,
    /// Error indicator style.
    pub error: Style,
    /// Border style for windows and panels.
    pub border: Style,
    /// Border style when a window or panel is active/focused.
    pub border_active: Style,
    /// Inactive/disabled element style.
    pub inactive: Style,
    /// Standard body text style.
    pub text: Style,
    /// Text style inside headers.
    pub header: Style,
    /// Text style inside status/footer lines.
    pub status: Style,
    /// Cursor style.
    pub cursor: Style,
}

impl Theme {
    /// Creates a default semantic theme configuration mapped to the DESIGN.md palette.
    pub fn default_dark() -> Self {
        Self {
            primary: Style::default().fg(Color::Rgb(240, 100, 45)), // Claude orange
            accent: Style::default().fg(Color::Rgb(128, 90, 213)), // purple
            success: Style::default().fg(Color::Green),
            warning: Style::default().fg(Color::Yellow),
            error: Style::default().fg(Color::Red),
            border: Style::default().fg(Color::DarkGray),
            border_active: Style::default().fg(Color::Rgb(240, 100, 45)),
            inactive: Style::default().fg(Color::Gray),
            text: Style::default().fg(Color::White),
            header: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            status: Style::default().fg(Color::DarkGray),
            cursor: Style::default().bg(Color::White).fg(Color::Black),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_dark()
    }
}
