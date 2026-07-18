//! Clipboard integration abstractions.

use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

/// Errors that can occur during clipboard operations.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    /// The clipboard is not available on this platform.
    #[error("Clipboard unavailable")]
    Unavailable,

    /// The operation failed due to an underlying system error.
    #[error("Clipboard operation failed")]
    OperationFailed(#[source] Box<dyn Error + Send + Sync>),
}

/// Trait abstracting platform clipboard actions.
pub trait Clipboard: Send + Sync {
    /// Fetches the current text content from the clipboard.
    fn get(&self) -> Result<String, ClipboardError>;

    /// Sets the text content in the clipboard.
    fn set(&mut self, text: &str) -> Result<(), ClipboardError>;
}

/// Clipboard utilizing standard macOS pbcopy/pbpaste tools.
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn get(&self) -> Result<String, ClipboardError> {
        let output = Command::new("pbpaste")
            .output()
            .map_err(|e| ClipboardError::OperationFailed(Box::new(e)))?;

        if !output.status.success() {
            return Err(ClipboardError::Unavailable);
        }

        let s = String::from_utf8(output.stdout)
            .map_err(|e| ClipboardError::OperationFailed(Box::new(e)))?;
        Ok(s)
    }

    fn set(&mut self, text: &str) -> Result<(), ClipboardError> {
        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| ClipboardError::OperationFailed(Box::new(e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| ClipboardError::OperationFailed(Box::new(e)))?;
        } else {
            return Err(ClipboardError::OperationFailed(Box::new(
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to open pbcopy stdin"),
            )));
        }

        let status = child
            .wait()
            .map_err(|e| ClipboardError::OperationFailed(Box::new(e)))?;

        if !status.success() {
            return Err(ClipboardError::Unavailable);
        }

        Ok(())
    }
}

/// In-memory mock clipboard for testing.
pub struct MockClipboard {
    content: String,
}

impl MockClipboard {
    /// Creates a new empty `MockClipboard`.
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }
}

impl Default for MockClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard for MockClipboard {
    fn get(&self) -> Result<String, ClipboardError> {
        Ok(self.content.clone())
    }

    fn set(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.content = text.to_string();
        Ok(())
    }
}
