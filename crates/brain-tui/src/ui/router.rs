//! Router managing active screens and validation paths.

use crate::ui::widgets::ChatScreen;

/// Screens available for layout viewport rendering.
pub enum ActiveScreen<'a> {
    /// Active chat communication panel.
    Chat(ChatScreen<'a>),
}

/// Navigation error conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationError {
    /// Target transition was rejected.
    TransitionDenied,
}

/// Controls and validates screen navigation paths.
pub struct ScreenRouter<'a> {
    active: ActiveScreen<'a>,
}

impl<'a> ScreenRouter<'a> {
    /// Instantiates a new ScreenRouter.
    pub fn new(initial: ActiveScreen<'a>) -> Self {
        Self { active: initial }
    }

    /// Access the currently active screen.
    pub fn current(&self) -> &ActiveScreen<'a> {
        &self.active
    }

    /// Attempts to navigate to the target screen.
    pub fn transition_to(&mut self, next: ActiveScreen<'a>) -> Result<(), NavigationError> {
        if self.can_transition(&next) {
            self.active = next;
            Ok(())
        } else {
            Err(NavigationError::TransitionDenied)
        }
    }

    /// Resolves whether transition to the target screen is allowed.
    pub fn can_transition(&self, _next: &ActiveScreen<'a>) -> bool {
        // By default, allow all screen transitions (can expand policy logic later)
        true
    }
}
