use brain_events::SequenceNumber;
use tokio::sync::broadcast;

/// Strongly-typed identifier to identify distinct projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionId {
    /// Jobs read-model projection.
    Jobs,
    /// Sessions list read-model projection.
    Sessions,
    /// Search index FTS5 projection.
    Search,
    /// Retrieval context projection.
    Retrieval,
    /// Test A projection.
    TestA,
    /// Test B projection.
    TestB,
    /// Test C projection.
    TestC,
}

impl std::str::FromStr for ProjectionId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jobs" | "Jobs" => Ok(Self::Jobs),
            "sessions" | "Sessions" => Ok(Self::Sessions),
            "search" | "Search" => Ok(Self::Search),
            "retrieval" | "Retrieval" => Ok(Self::Retrieval),
            "test_a" | "TestA" => Ok(Self::TestA),
            "test_b" | "TestB" => Ok(Self::TestB),
            "test_c" | "TestC" => Ok(Self::TestC),
            _ => Err(format!("Unknown projection ID: {}", s)),
        }
    }
}

/// A lightweight, best-effort notification indicating that a projection has advanced.
///
/// **Invariants & Safety Rules**:
/// - **Minimal Payload**: A `ProjectionNotification` communicates only that a projection has advanced
///   to at least a given sequence. It must never include domain events, projection rows, DTOs,
///   or any derived state.
/// - **Best Effort**: Notifications are hints only; correctness is guaranteed by persistent database
///   checkpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionNotification {
    /// The unique typed ID of the advanced projection.
    pub projection_id: ProjectionId,
    /// The latest sequence checkpoint it advanced to.
    pub sequence: SequenceNumber,
}

/// Decoupled, in-memory best-effort notification bus for broadcasting projection advances.
pub struct ProjectionNotificationBus {
    sender: broadcast::Sender<ProjectionNotification>,
}

impl ProjectionNotificationBus {
    /// Creates a new `ProjectionNotificationBus`.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    /// Publishes a projection advance notification onto the bus.
    pub fn publish(&self, notification: ProjectionNotification) {
        let _ = self.sender.send(notification);
    }

    /// Subscribes to the projection advance notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<ProjectionNotification> {
        self.sender.subscribe()
    }
}

impl Default for ProjectionNotificationBus {
    fn default() -> Self {
        Self::new()
    }
}
