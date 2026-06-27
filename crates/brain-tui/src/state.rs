use brain_domain::SessionId;

/// Active rendering connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Connected to UDS daemon.
    Daemon,
    /// Direct in-process engine.
    Embedded,
    /// Disconnected state.
    Disconnected,
    /// Attempting to establish socket hook.
    Connecting,
}

/// Viewport and scroll offset parameters.
pub struct ViewportState {
    /// Monotonic scroll offset position.
    pub scroll_offset: usize,
    /// Lock scroll to follow incoming responses tail.
    pub follow_tail: bool,
    /// Flag indicating whether command selection modal overlay is open.
    pub is_command_palette_open: bool,
}

/// Interactive input prompt editing buffer.
pub struct EditorState {
    chars: Vec<char>,
    cursor: usize,
}

impl EditorState {
    /// Creates a new `EditorState`.
    pub fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
        }
    }

    /// Returns the active text buffer as a String.
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Returns the active cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Inserts a character at the current cursor position.
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Removes the character immediately preceding the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    /// Removes the character at the current cursor position.
    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    /// Moves the cursor position leftwards by one character cell.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Moves the cursor position rightwards by one character cell.
    pub fn move_right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Central application layout and editor context.
pub struct UiState {
    /// Active engine connection mode.
    pub connection_mode: ConnectionMode,
    /// Current conversation session identifier.
    pub session_id: SessionId,
    /// Title description of the active session.
    pub session_title: String,
    /// Interactive prompt input buffer.
    pub editor: EditorState,
    /// Viewport state variables.
    pub viewport: ViewportState,
}

/// Structured user action triggering pure state transitions.
pub enum Action {
    /// Append a text character to input.
    InsertChar(char),
    /// Move prompt editor cursor left.
    MoveCursorLeft,
    /// Move prompt editor cursor right.
    MoveCursorRight,
    /// Trigger backspace text deletion.
    Backspace,
    /// Trigger delete text deletion.
    Delete,
    /// Force screen dimension recalculation.
    Resize(u16, u16),
    /// Request TUI termination.
    Quit,
    /// Modify connection mode.
    SetConnectionMode(ConnectionMode),
}

/// Pure status indicator returning from state updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// Redraw not needed.
    NoChange,
    /// Trigger layout update and redraw.
    Changed,
    /// Exit main interactive loop.
    Exit,
}

impl UiState {
    /// Creates a default `UiState` with random Session ID.
    pub fn new() -> Self {
        Self {
            connection_mode: ConnectionMode::Disconnected,
            session_id: SessionId::new(),
            session_title: "New Conversation".to_string(),
            editor: EditorState::new(),
            viewport: ViewportState {
                scroll_offset: 0,
                follow_tail: true,
                is_command_palette_open: false,
            },
        }
    }

    /// Pure reducer transitioning state based on Action.
    pub fn update(&mut self, action: Action) -> UpdateResult {
        match action {
            Action::InsertChar(c) => {
                self.editor.insert(c);
                UpdateResult::Changed
            }
            Action::MoveCursorLeft => {
                self.editor.move_left();
                UpdateResult::Changed
            }
            Action::MoveCursorRight => {
                self.editor.move_right();
                UpdateResult::Changed
            }
            Action::Backspace => {
                self.editor.backspace();
                UpdateResult::Changed
            }
            Action::Delete => {
                self.editor.delete();
                UpdateResult::Changed
            }
            Action::Resize(_, _) => {
                UpdateResult::Changed
            }
            Action::Quit => {
                UpdateResult::Exit
            }
            Action::SetConnectionMode(mode) => {
                if self.connection_mode != mode {
                    self.connection_mode = mode;
                    UpdateResult::Changed
                } else {
                    UpdateResult::NoChange
                }
            }
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_input_append() {
        let mut state = UiState::new();
        assert_eq!(state.editor.text(), "");
        assert_eq!(state.editor.cursor(), 0);

        let res = state.update(Action::InsertChar('a'));
        assert_eq!(res, UpdateResult::Changed);
        assert_eq!(state.editor.text(), "a");
        assert_eq!(state.editor.cursor(), 1);

        state.update(Action::InsertChar('b'));
        assert_eq!(state.editor.text(), "ab");
        assert_eq!(state.editor.cursor(), 2);

        state.update(Action::MoveCursorLeft);
        assert_eq!(state.editor.cursor(), 1);

        state.update(Action::InsertChar('c'));
        assert_eq!(state.editor.text(), "acb");
        assert_eq!(state.editor.cursor(), 2);

        state.update(Action::Backspace);
        assert_eq!(state.editor.text(), "ab");
        assert_eq!(state.editor.cursor(), 1);

        state.update(Action::Delete);
        assert_eq!(state.editor.text(), "a");
        assert_eq!(state.editor.cursor(), 1);
    }
}
