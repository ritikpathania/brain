pub mod client;

pub use client::{
    BrainClient, BrainSdkError, ClientConfig, IngestAck, ReplayResponse, RuntimeState,
};

/// Backpressure policies when SDK queues are full
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressurePolicy {
    /// Block the sender until space becomes available
    Block,
    /// Return a QueueFull error immediately
    Fail,
    /// Drop the oldest unsent event to make space
    DropOldest,
}

/// Command sent from BrainClient handle to background ClientRuntime loop
pub enum ClientCommand {
    /// Ingest a single event
    Send {
        event: brain_integrations::IngestionEvent,
        tx: tokio::sync::oneshot::Sender<Result<IngestAck, BrainSdkError>>,
    },
    /// Request event replay from sequence
    Replay {
        after_sequence: u64,
        tx: tokio::sync::oneshot::Sender<
            Result<Vec<brain_integrations::IngestionEnvelope>, BrainSdkError>,
        >,
    },
    /// Gracefully shutdown the runtime
    Shutdown {
        tx: tokio::sync::oneshot::Sender<()>,
    },
}
