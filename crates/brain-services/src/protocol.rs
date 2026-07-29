//! Stable Application Interface DTO contracts, versioned protocol schemas, and Handshake Negotiator (Phase H).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current frozen protocol version for Brain Relational Memory Engine IPC & SDK.
pub const CURRENT_PROTOCOL_VERSION: u32 = 1;

/// Minimum supported backward-compatible protocol version.
pub const MIN_SUPPORTED_PROTOCOL_VERSION: u32 = 1;

/// Protocol version identifier wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub u32);

impl ProtocolVersion {
    /// Initial frozen V1 protocol version.
    pub fn v1() -> Self {
        Self(1)
    }
}

/// Supported protocol version range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedRange {
    /// Minimum supported protocol version.
    pub min_version: ProtocolVersion,
    /// Maximum supported protocol version.
    pub max_version: ProtocolVersion,
}

impl SupportedRange {
    /// Returns default supported version range.
    pub fn default_range() -> Self {
        Self {
            min_version: ProtocolVersion(MIN_SUPPORTED_PROTOCOL_VERSION),
            max_version: ProtocolVersion(CURRENT_PROTOCOL_VERSION),
        }
    }

    /// Checks if a client protocol version falls within the supported range.
    pub fn contains(&self, version: ProtocolVersion) -> bool {
        version >= self.min_version && version <= self.max_version
    }
}

/// Strongly typed protocol errors.
#[derive(Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolError {
    /// Client requested an unsupported protocol version.
    #[error("Unsupported protocol version {requested:?}. Supported range is {min:?} to {max:?}")]
    UnsupportedVersion {
        /// Requested version index.
        requested: u32,
        /// Minimum supported version index.
        min: u32,
        /// Maximum supported version index.
        max: u32,
    },
    /// Malformed protocol handshake message payload.
    #[error("Malformed handshake payload: {0}")]
    MalformedPayload(String),
}

/// Centralized protocol negotiator handling handshake negotiation prior to business execution.
pub struct ProtocolNegotiator;

impl ProtocolNegotiator {
    /// Negotiates protocol version between client request and server supported range.
    pub fn negotiate(
        client_version: ProtocolVersion,
        server_range: SupportedRange,
    ) -> Result<ProtocolVersion, ProtocolError> {
        if server_range.contains(client_version) {
            Ok(client_version)
        } else {
            Err(ProtocolError::UnsupportedVersion {
                requested: client_version.0,
                min: server_range.min_version.0,
                max: server_range.max_version.0,
            })
        }
    }
}

use std::collections::HashSet;

/// Supported Protocol Capabilities that can be dynamically negotiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolCapability {
    /// Replay capability.
    Replay,
    /// Live event streaming capability.
    Streaming,
    /// Binary state snapshot capability.
    Snapshots,
    /// Payload compression capability.
    Compression,
}

/// Client handshake HELLO request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeHelloDTO {
    /// Client requested protocol version.
    pub client_version: ProtocolVersion,
    /// Unique client instance identifier string.
    pub client_id: String,
    /// Requested protocol capabilities.
    pub requested_capabilities: HashSet<ProtocolCapability>,
}

/// Server handshake ACK response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeAckDTO {
    /// Agreed negotiated protocol version.
    pub negotiated_version: ProtocolVersion,
    /// Server supported version range.
    pub server_range: SupportedRange,
    /// Accepted protocol capabilities.
    pub accepted_capabilities: HashSet<ProtocolCapability>,
}

/// Frozen Command DTO: Goal execution request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteGoalCommandDTO {
    /// Goal instruction prompt text.
    pub goal_prompt: String,
    /// Target workspace path string.
    pub workspace_id: String,
    /// Maximum execution timeout in seconds.
    pub timeout_seconds: u64,
}

/// Frozen Query DTO: Knowledge graph query request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryKnowledgeGraphDTO {
    /// Target node or query string.
    pub query_text: String,
    /// Maximum entity result count limit.
    pub limit: usize,
}

/// Frozen Event Stream Subscription DTO: Replay & live event stream request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamEventsSubscriptionDTO {
    /// Target execution plan ID to filter events.
    pub plan_id: String,
    /// Optional starting sequence number for replay streams.
    pub start_sequence: Option<u64>,
}
