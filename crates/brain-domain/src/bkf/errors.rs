use crate::bkf::facts::BKFTargetRef;
use crate::bkf::ids::*;
use thiserror::Error;
use ulid::Ulid;

/// Validation and structural errors for the BKF module.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BkfError {
    /// Detected duplicate identifier across BKF elements.
    #[error("Duplicate ID detected: {0}")]
    DuplicateId(Ulid),

    /// Referential integrity violation.
    #[error("Referential integrity error: element {id} references missing target: {target:?}")]
    MissingReference {
        /// Referencing element description/ID.
        id: String,
        /// Missing referenced target.
        target: BKFTargetRef,
    },

    /// Loop detected in hierarchy.
    #[error("Section cyclic hierarchy detected: Section cycle around {id}")]
    CycleDetected {
        /// Section ID that caused the cycle.
        id: BkfSectionId,
    },

    /// Invalid schema version.
    #[error("Invalid schema version semver format: {0}")]
    InvalidSchema(String),

    /// Self-referencing link error.
    #[error("Self-referencing relationship not allowed on ID: {id}")]
    SelfReferencing {
        /// Relationship ID.
        id: BkfRelationshipId,
    },

    /// Missing metadata container.
    #[error("Metadata is required to build a BKFDocument")]
    MissingMetadata,
}
