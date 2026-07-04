//! TextBuffer and Editor state management.

/// Wraps raw string editing data, shielding the Editor from storage implementation details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBuffer {
    text: String,
}

impl TextBuffer {
    /// Instantiates a new empty TextBuffer.
    pub fn new() -> Self {
        Self { text: String::new() }
    }

    /// Access the underlying string content slice.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Returns the length in bytes of the text buffer.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether the buffer contains no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Inserts a character at the specified byte offset.
    pub(crate) fn insert(&mut self, idx: usize, c: char) {
        if idx <= self.text.len() {
            self.text.insert(idx, c);
        }
    }

    /// Removes a character at the specified byte offset.
    pub(crate) fn remove(&mut self, idx: usize) -> char {
        if idx < self.text.len() {
            self.text.remove(idx)
        } else {
            '\0'
        }
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Abstract coordinate mapping cursor bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// UTF-8 byte index in the buffer.
    pub byte_index: usize,
    /// Visual character column index.
    pub visual_col: u16,
}

/// A text editing controller managing a TextBuffer and Cursor coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    buffer: TextBuffer,
    cursor: Cursor,
}

impl Editor {
    /// Instantiates a new Editor.
    pub fn new() -> Self {
        Self {
            buffer: TextBuffer::new(),
            cursor: Cursor { byte_index: 0, visual_col: 0 },
        }
    }

    /// Clears the text buffer and resets the cursor.
    pub fn clear(&mut self) {
        self.buffer = TextBuffer::new();
        self.cursor = Cursor { byte_index: 0, visual_col: 0 };
    }

    /// Inserts a character at the active cursor position (aliasing insert).
    pub fn insert_char(&mut self, c: char) {
        self.insert(c);
    }

    /// Moves the cursor to the end of the text buffer.
    pub fn move_to_end(&mut self) {
        self.cursor.byte_index = self.buffer.len();
        self.cursor.visual_col = self.buffer.as_str().chars().count() as u16;
    }

    /// Access the current text content slice.
    pub fn text(&self) -> &str {
        self.buffer.as_str()
    }

    /// Returns the active Cursor indices.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Inserts a character at the active cursor position.
    pub fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor.byte_index, c);
        let char_len = c.len_utf8();
        self.cursor.byte_index += char_len;
        self.cursor.visual_col += 1;
    }

    /// Removes the character behind the active cursor position (backspace).
    pub fn backspace(&mut self) {
        if self.cursor.byte_index > 0 {
            let prefix = &self.buffer.as_str()[..self.cursor.byte_index];
            if let Some((idx, _c)) = prefix.char_indices().next_back() {
                let _ = self.buffer.remove(idx);
                self.cursor.byte_index = idx;
                self.cursor.visual_col = self.cursor.visual_col.saturating_sub(1);
            }
        }
    }

    /// Removes the character in front of the active cursor position (delete).
    pub fn delete(&mut self) {
        if let Some(tail) = self.buffer.as_str().get(self.cursor.byte_index..) {
            if tail.chars().next().is_some() {
                let _ = self.buffer.remove(self.cursor.byte_index);
            }
        }
    }

    /// Moves the cursor left by 1 column if bounds allow.
    pub fn move_cursor_left(&mut self) {
        if self.cursor.byte_index > 0 {
            let prefix = &self.buffer.as_str()[..self.cursor.byte_index];
            if let Some((idx, _c)) = prefix.char_indices().next_back() {
                self.cursor.byte_index = idx;
                self.cursor.visual_col = self.cursor.visual_col.saturating_sub(1);
            }
        }
    }

    /// Moves the cursor right by 1 column if bounds allow.
    pub fn move_cursor_right(&mut self) {
        if let Some(tail) = self.buffer.as_str().get(self.cursor.byte_index..) {
            if let Some(c) = tail.chars().next() {
                self.cursor.byte_index += c.len_utf8();
                self.cursor.visual_col += 1;
            }
        }
    }

    /// Access the raw buffer text.
    pub fn buffer(&self) -> &str {
        self.buffer.as_str()
    }

    /// Handles a key event by mutating the editor state. Returns true if handled.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char(c) => {
                self.insert(c);
                true
            }
            KeyCode::Backspace => {
                self.backspace();
                true
            }
            KeyCode::Delete => {
                self.delete();
                true
            }
            KeyCode::Left => {
                self.move_cursor_left();
                true
            }
            KeyCode::Right => {
                self.move_cursor_right();
                true
            }
            _ => false,
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
