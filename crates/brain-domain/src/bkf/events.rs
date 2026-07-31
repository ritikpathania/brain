//! Append-only domain events emitted by knowledge mutations and reflection rewrites.

use crate::bkf::fact_version::*;
use crate::bkf::value_objects::*;
use serde::{Deserialize, Serialize};

/// Immutable domain event capturing facts recorded, superseded, or archived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum FactEvent {
    /// A new fact version was recorded.
    FactRecorded {
        /// The complete recorded fact version.
        fact: FactVersion,
        /// Optional semantic assertion context.
        #[serde(default)]
        assertion: Option<SemanticAssertion>,
    },

    /// An existing fact version was superseded by a newer version.
    FactSuperseded {
        /// Identifier of the superseded fact version.
        old_fact_id: FactVersionId,
        /// Identifier of the new successor fact version.
        new_fact_id: FactVersionId,
        /// Timestamp when supercedence occurred.
        superseded_at: Timestamp,
    },
    /// A fact version was archived to cold storage.
    FactArchived {
        /// Identifier of the archived fact version.
        fact_id: FactVersionId,
        /// Timestamp when archiving occurred.
        archived_at: Timestamp,
    },
}
