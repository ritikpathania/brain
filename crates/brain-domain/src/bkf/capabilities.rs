use serde::{Deserialize, Serialize};

/// Advertising what structural and semantic items are present inside the document.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BKFCapabilities {
    /// Indicates if sections are present.
    pub has_sections: bool,
    /// Indicates if blocks are present.
    pub has_blocks: bool,
    /// Indicates if entities are present.
    pub has_entities: bool,
    /// Indicates if relationships are present.
    pub has_relationships: bool,
    /// Indicates if facts are present.
    pub has_facts: bool,
    /// Indicates if citations are present.
    pub has_citations: bool,
    /// Indicates if attachments are present.
    pub has_attachments: bool,
    /// Indicates if vector embedding index links are present.
    pub has_embeddings: bool,
}
