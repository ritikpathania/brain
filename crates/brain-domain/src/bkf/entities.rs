use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::bkf::ids::BkfEntityId;

/// An entity extracted from text or structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    /// Entity ID.
    pub id: BkfEntityId,
    /// Entity classification type (e.g. Person, Org, API).
    pub entity_type: String,
    /// Canonical name of the entity.
    pub name: String,
    /// Alternate names/aliases.
    pub aliases: Vec<String>,
    /// Extensible key-value properties.
    pub attributes: HashMap<String, serde_json::Value>,
    /// Extraction confidence (0.0 to 1.0).
    pub confidence: f32,
}
