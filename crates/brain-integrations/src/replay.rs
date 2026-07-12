//! Replay tracking positions for guaranteeing monotonic in-order streams.

use brain_domain::{AdapterId, EventId};
use serde::{Deserialize, Serialize};

/// Position tracking for adapter replays on daemon reconnection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayPosition {
    /// The adapter that owns this replay position.
    pub adapter_id: AdapterId,

    /// The event ID of the last successfully processed event.
    pub last_acknowledged: Option<EventId>,

    /// Monotonic sequence number assigned by the Brain server.
    pub sequence: u64,
}
