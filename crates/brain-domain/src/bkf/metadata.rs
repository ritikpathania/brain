use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata properties of the document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metadata {
    /// Document title.
    pub title: Option<String>,
    /// Document author.
    pub author: Option<String>,
    /// Sha256 checksum of raw source content.
    pub checksum: Option<String>,
    /// Unique content fingerprint.
    pub fingerprint: Option<String>,
    /// Document language code.
    pub language: Option<String>,
    /// MIME content type.
    pub mime: Option<String>,
    /// Size of the raw source in bytes.
    pub size: Option<u64>,
    /// Unix timestamp when created.
    pub created: Option<u64>,
    /// Unix timestamp when modified.
    pub modified: Option<u64>,
    /// License information.
    pub license: Option<String>,
    /// Extensible extra attributes.
    pub extra: HashMap<String, serde_json::Value>,
}
