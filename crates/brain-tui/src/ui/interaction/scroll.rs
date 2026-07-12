//! Viewport scrolling state tracker.

/// Auto-scroll alignment policy configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoFollowPolicy {
    /// Viewport follows incoming stream blocks.
    Pinned,
    /// Manual control; do not shift viewport on new arrivals.
    Manual,
    /// Viewport follow suspended.
    Suspended,
}

/// Manages vertical window scroll viewport offsets.
pub struct ScrollState {
    offset: usize,
    content_height: usize,
    viewport_height: usize,
    /// Scrolling alignment auto-follow policy.
    pub policy: AutoFollowPolicy,
}

impl ScrollState {
    /// Instantiates a new ScrollState.
    pub fn new() -> Self {
        Self {
            offset: 0,
            content_height: 0,
            viewport_height: 0,
            policy: AutoFollowPolicy::Pinned,
        }
    }

    /// Access the active vertical line offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Computes the maximum scrollable line index offset.
    pub fn max_offset(&self) -> usize {
        self.content_height.saturating_sub(self.viewport_height)
    }

    /// Shifts the offset up by 1 line, clamping at 0 and transition to Manual.
    pub fn scroll_up(&mut self) {
        self.offset = self.offset.saturating_sub(1);
        self.policy = AutoFollowPolicy::Manual;
    }

    /// Shifts the offset down by 1 line, clamping and transition to Pinned if bottom is reached.
    pub fn scroll_down(&mut self) {
        let max = self.max_offset();
        self.offset = (self.offset + 1).min(max);
        if self.offset == max {
            self.policy = AutoFollowPolicy::Pinned;
        }
    }

    /// Modifies dynamic height constraints, clamping offset or pinning to bottom if enabled.
    pub fn update_bounds(&mut self, content_height: usize, viewport_height: usize) {
        self.content_height = content_height;
        self.viewport_height = viewport_height;
        if self.policy == AutoFollowPolicy::Pinned {
            self.offset = self.max_offset();
        } else {
            self.offset = self.offset.min(self.max_offset());
        }
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}
