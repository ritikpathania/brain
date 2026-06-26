use brain_domain::{PluginId, PluginState, SessionId};
use thiserror::Error;

/// The central system error type representing all recoverable and unrecoverable failures inside Brain.
#[derive(Error, Debug)]
pub enum BrainError {
    /// Failure during storage or database operations.
    #[error("Storage Error: {message}")]
    Storage {
        /// Descriptive message of the failure.
        message: String,
        /// Optional underlying database or connection source error.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Failure in plugin loading, verification, or state transition.
    #[error("Plugin Error [State: {state:?}]: {message}")]
    Plugin {
        /// Crate-unique identifier of the plugin.
        plugin_id: PluginId,
        /// Current lifecycle state when the error occurred.
        state: PluginState,
        /// Detailed error message.
        message: String,
    },

    /// Failure executing embedded Python interpreter agent code.
    #[error("Python Execution Error: {message}")]
    Python {
        /// Error message.
        message: String,
        /// Optional Python interpreter execution stack traceback.
        traceback: Option<String>,
    },

    /// Failure performing network requests.
    #[error("Network I/O Error: {message}")]
    Network {
        /// Error message.
        message: String,
        /// Target URL which failed.
        url: Option<String>,
    },

    /// Failure executing a tool.
    #[error("Tool Execution Error [Tool: {tool_name}]: {message}")]
    Tool {
        /// Name of the target tool.
        tool_name: String,
        /// Error message.
        message: String,
    },

    /// Failure in session management lifecycle.
    #[error("Session Lifecycle Error [Session: {session_id}]: {message}")]
    Session {
        /// Identifier of the session.
        session_id: SessionId,
        /// Error message.
        message: String,
    },

    /// Configuration parsing or load hierarchy failure.
    #[error("Configuration Error: {message}")]
    Configuration {
        /// Error message.
        message: String,
    },

    /// Invalid startup state machine transition.
    #[error("Lifecycle State Error: {message}")]
    InvalidTransition {
        /// Error message.
        message: String,
    },

    /// Validation failure of inputs or schema definitions.
    #[error("Validation Error: {message}")]
    Validation {
        /// Error message.
        message: String,
    },

    /// Authorization failure verifying tool/plugin permission scopes.
    #[error("Authorization Error: {message}")]
    Authorization {
        /// Error message.
        message: String,
    },

    /// Timeout occurred during task execution.
    #[error("Timeout Error: Action timed out after {elapsed_ms}ms: {message}")]
    Timeout {
        /// Time elapsed in milliseconds.
        elapsed_ms: u64,
        /// Error message.
        message: String,
    },

    /// Recoverable or unrecoverable internal system error.
    #[error("Internal System Error: {message}")]
    Internal {
        /// Error message.
        message: String,
    },
}
