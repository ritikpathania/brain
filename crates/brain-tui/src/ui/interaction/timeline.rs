//! Chronological event timeline models and ordinals.

use brain_core::events::ToolCallId;
use brain_domain::bkf::retrieval::RetrievalId;
use crate::ui::interaction::chat::MessageId;

/// Strongly-typed monotonic ordinal for ordering events in a session's timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventOrdinal(pub u64);

/// Chronological event items mapped inside a session's timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineItem {
    /// Reference to a chat message.
    Message(MessageId),
    /// Reference to a tool execution call.
    ToolExecution(ToolCallId),
    /// Reference to a context retrieval block.
    Retrieval(RetrievalId),
}
