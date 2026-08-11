//! Formal state machine for conversation viewport scroll anchoring during streaming responses.

/// State of viewport anchoring relative to streaming content bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollAnchor {
    /// Viewport automatically follows and locks to bottom during active streaming.
    #[default]
    Pinned,
    /// Viewport is unlocked at a user-specified manual scroll offset.
    Unpinned,
}

impl ScrollAnchor {
    /// Creates a new `ScrollAnchor` initialized to `Pinned`.
    pub fn new() -> Self {
        Self::Pinned
    }

    /// Handles a manual scroll up action from the user.
    /// Explicitly transitions from `Pinned` to `Unpinned`.
    pub fn on_scroll_up(&mut self) {
        *self = Self::Unpinned;
    }

    /// Updates anchor state based on current viewport offset relative to max valid offset.
    /// If `current_offset >= max_offset`, re-enables `Pinned` state.
    pub fn update_position(&mut self, current_offset: usize, max_offset: usize) {
        if current_offset >= max_offset {
            *self = Self::Pinned;
        }
    }

    /// Evaluates whether the viewport should automatically snap to bottom when a new token arrives.
    pub fn should_follow_bottom(&self) -> bool {
        matches!(self, Self::Pinned)
    }

    /// Returns `true` if currently in `Pinned` state.
    pub fn is_pinned(&self) -> bool {
        matches!(self, Self::Pinned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_anchor_state_transitions() {
        let mut anchor = ScrollAnchor::new();
        assert_eq!(anchor, ScrollAnchor::Pinned);
        assert!(anchor.should_follow_bottom());

        // ScrollUp transitions to Unpinned
        anchor.on_scroll_up();
        assert_eq!(anchor, ScrollAnchor::Unpinned);
        assert!(!anchor.should_follow_bottom());

        // Token arrival while Unpinned keeps it Unpinned
        anchor.update_position(5, 20);
        assert_eq!(anchor, ScrollAnchor::Unpinned);
        assert!(!anchor.should_follow_bottom());

        // Reaching max_offset transitions back to Pinned
        anchor.update_position(20, 20);
        assert_eq!(anchor, ScrollAnchor::Pinned);
        assert!(anchor.should_follow_bottom());
    }
}
