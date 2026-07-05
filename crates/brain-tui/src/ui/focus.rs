//! Focus management and graph cyclic traversal.

use crate::ui::widgets::view_models::FocusTarget;

/// Categorizes visibility layer focal boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusScope {
    /// Primary view panel layers.
    Screen,
    /// Dialog/Confirm prompts.
    Modal,
    /// Transient overlays/dropdowns.
    Overlay,
}

/// Traversal configurations mapping static graph arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusProfile {
    /// Chat screen order.
    Chat,
    /// Dialog modal order.
    Dialog,
}

impl FocusProfile {
    /// Returns the static slice ordering of traversal targets.
    pub fn targets(self) -> &'static [FocusTarget] {
        match self {
            FocusProfile::Chat => &[
                FocusTarget::Sidebar,
                FocusTarget::Conversation,
                FocusTarget::Prompt,
            ],
            FocusProfile::Dialog => &[
                FocusTarget::Prompt,
            ],
        }
    }
}

/// Tracks focused inputs and manages cyclic graph traversal.
pub struct FocusManager {
    focused: FocusTarget,
    scope: FocusScope,
    profile: FocusProfile,
    /// Saved focus target for transient overlays.
    saved_focus: Option<FocusTarget>,
}

impl FocusManager {
    /// Instantiates a new FocusManager with focus targets.
    pub fn new(initial: FocusTarget, profile: FocusProfile) -> Self {
        Self {
            focused: initial,
            scope: FocusScope::Screen,
            profile,
            saved_focus: None,
        }
    }

    /// Access the currently focused panel.
    pub fn current(&self) -> FocusTarget {
        self.focused
    }

    /// Access the currently active focus scope.
    pub fn scope(&self) -> FocusScope {
        self.scope
    }

    /// Update focus scope.
    pub fn set_scope(&mut self, scope: FocusScope) {
        self.scope = scope;
    }

    /// Moves focus forward along the configured profile list.
    pub fn next(&mut self) {
        let targets = self.profile.targets();
        if targets.is_empty() {
            return;
        }
        if let Some(idx) = targets.iter().position(|&t| t == self.focused) {
            let next_idx = (idx + 1) % targets.len();
            self.focused = targets[next_idx];
        } else {
            self.focused = targets[0];
        }
    }

    /// Moves focus backward along the configured profile list.
    pub fn prev(&mut self) {
        let targets = self.profile.targets();
        if targets.is_empty() {
            return;
        }
        if let Some(idx) = targets.iter().position(|&t| t == self.focused) {
            let prev_idx = if idx == 0 { targets.len() - 1 } else { idx - 1 };
            self.focused = targets[prev_idx];
        } else {
            self.focused = targets[0];
        }
    }

    /// Set focused target directly.
    pub fn set_focus(&mut self, target: FocusTarget) {
        self.focused = target;
    }

    /// Saves the current focus target.
    pub fn save_focus(&mut self, target: FocusTarget) {
        self.saved_focus = Some(target);
    }

    /// Pops and returns the saved focus target, resetting it to None.
    pub fn pop_saved_focus(&mut self) -> Option<FocusTarget> {
        self.saved_focus.take()
    }
}

