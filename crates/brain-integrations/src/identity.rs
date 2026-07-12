//! Identity model for attributing events to workspaces, sessions, and clients.

use brain_domain::{AdapterId, ClientId, ConversationId, EventId, SessionId, WorkspaceId};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Full identity chain for attributing an ingestion event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct EventIdentity {
    /// Identifies the adapter type that produced this event.
    #[specta(type = String)]
    pub adapter_id: AdapterId,

    /// Identifies the client application instance (e.g. "cursor-2.1.3-abc").
    #[specta(type = String)]
    pub client_id: ClientId,

    /// Groups conversational events into a single thread.
    #[specta(type = Option<String>)]
    pub conversation_id: Option<ConversationId>,

    /// Unique identifier for this specific event. Used for deduplication.
    #[specta(type = String)]
    pub event_id: EventId,

    /// Optional parent event (e.g. tool call parent is the prompting message).
    #[specta(type = Option<String>)]
    pub parent_event_id: Option<EventId>,

    /// Groups events into a session boundary.
    #[specta(type = String)]
    pub session_id: SessionId,

    /// Wall-clock timestamp when the event was generated at the client.
    #[specta(type = String)]
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Identifies the physical workspace/project directory.
    #[specta(type = String)]
    pub workspace_id: WorkspaceId,
}
