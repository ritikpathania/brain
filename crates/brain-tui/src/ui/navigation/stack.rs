//! History-backed Navigation Stack.

use super::screen::Screen;

/// Tracks screen history, enabling deterministic back navigation on Esc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationStack {
    history: Vec<Screen>,
}

impl Default for NavigationStack {
    fn default() -> Self {
        Self {
            history: vec![Screen::Home],
        }
    }
}

impl NavigationStack {
    /// Creates a new NavigationStack initialized with the specified screen.
    pub fn new(initial: Screen) -> Self {
        Self {
            history: vec![initial],
        }
    }

    /// Access the active top-of-stack screen.
    pub fn current(&self) -> Screen {
        self.history.last().copied().unwrap_or(Screen::Home)
    }

    /// Pushes a new screen onto the history stack.
    pub fn push(&mut self, screen: Screen) {
        if self.current() != screen {
            self.history.push(screen);
        }
    }

    /// Pops the top screen off the stack, returning to the previous screen.
    pub fn pop(&mut self) -> Option<Screen> {
        if self.history.len() > 1 {
            self.history.pop()
        } else {
            None
        }
    }

    /// Replaces the entire history stack with a single target screen.
    pub fn reset(&mut self, screen: Screen) {
        self.history.clear();
        self.history.push(screen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_stack_push_pop() {
        let mut nav = NavigationStack::new(Screen::Home);
        assert_eq!(nav.current(), Screen::Home);

        nav.push(Screen::Workspace);
        assert_eq!(nav.current(), Screen::Workspace);

        nav.push(Screen::GraphExplorer);
        assert_eq!(nav.current(), Screen::GraphExplorer);

        assert_eq!(nav.pop(), Some(Screen::GraphExplorer));
        assert_eq!(nav.current(), Screen::Workspace);

        assert_eq!(nav.pop(), Some(Screen::Workspace));
        assert_eq!(nav.current(), Screen::Home);

        // Cannot pop root screen
        assert_eq!(nav.pop(), None);
        assert_eq!(nav.current(), Screen::Home);
    }
}
