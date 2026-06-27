use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Layout grid organizer dividing the screen cells.
pub struct AppRenderer;

impl AppRenderer {
    /// Creates a new `AppRenderer`.
    pub fn new() -> Self {
        Self
    }

    /// Computes constraints and returns partitioned area Rects for widgets.
    pub fn compute_layout(&self, area: Rect) -> (Rect, Rect, Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Logo / Header
                Constraint::Min(10),  // Chat window viewport
                Constraint::Length(3), // Prompt input editor
                Constraint::Length(1), // Footer status bar
            ])
            .split(area);
        
        (chunks[0], chunks[1], chunks[2], chunks[3])
    }
}

impl Default for AppRenderer {
    fn default() -> Self {
        Self::new()
    }
}
