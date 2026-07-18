use brain_core::errors::BrainError;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use std::io::stdout;

/// RAII guard managing raw mode and alternate screen setup and teardown.
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Enters raw mode and the alternate screen buffer.
    pub fn new() -> Result<Self, BrainError> {
        enable_raw_mode().map_err(|e| BrainError::Validation {
            message: format!("Failed to enable raw mode: {}", e),
        })?;

        let mut out = stdout();
        out.execute(EnterAlternateScreen)
            .map_err(|e| BrainError::Validation {
                message: format!("Failed to enter alternate screen: {}", e),
            })?;
        out.execute(crossterm::cursor::Hide)
            .map_err(|e| BrainError::Validation {
                message: format!("Failed to hide cursor: {}", e),
            })?;

        Ok(Self { active: true })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let mut out = stdout();
            let _ = out.execute(crossterm::cursor::Show);
            let _ = out.execute(LeaveAlternateScreen);
            let _ = disable_raw_mode();
            self.active = false;
        }
    }
}
