//! ArtifactDescriptor payload produced by StepExecutors to declare artifact outputs and metadata labels.

use crate::artifact::EvidenceArtifactKind;
use crate::value::StructuredValue;
use std::collections::BTreeMap;

/// Payload descriptor emitted by step executors containing value, representation kind, and metadata labels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArtifactDescriptor {
    /// Semantic representation classification.
    pub kind: EvidenceArtifactKind,
    /// Canonical structured domain value.
    pub value: StructuredValue,
    /// Extensible metadata key-value annotations.
    pub labels: BTreeMap<String, String>,
}

impl ArtifactDescriptor {
    /// Instantiates a new `ArtifactDescriptor`.
    pub fn new(kind: EvidenceArtifactKind, value: StructuredValue) -> Self {
        Self {
            kind,
            value,
            labels: BTreeMap::new(),
        }
    }

    /// Appends a metadata key-value label annotation.
    pub fn with_label(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.labels.insert(key.into(), val.into());
        self
    }
}
