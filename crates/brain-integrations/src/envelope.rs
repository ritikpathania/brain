//! Ingestion envelopes wrapping identity and payload.

use crate::events::IngestionEvent;
use crate::identity::EventIdentity;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Transport-agnostic envelope containing the identity metadata and event payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub struct IngestionEnvelope {
    /// Typed event payload.
    pub event: IngestionEvent,

    /// Canonical Event Model version (e.g. "1.0").
    pub event_model_version: String,

    /// Full identity chain for attributing the event.
    pub identity: EventIdentity,
}
