use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::bkf::ids::*;
use crate::bkf::metadata::Metadata;
use crate::bkf::capabilities::BKFCapabilities;
use crate::bkf::blocks::Block;
use crate::bkf::entities::Entity;
use crate::bkf::relationships::Relationship;
use crate::bkf::facts::Fact;
use crate::bkf::references::{EmbeddingRef, ChunkRef, Citation, Attachment};
use crate::bkf::provenance::Provenance;

/// A section within a document, establishing structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Section {
    /// Section ID.
    pub id: BkfSectionId,
    /// Section heading title.
    pub title: String,
    /// Heading hierarchy level (e.g. 1 for H1).
    pub level: u32,
    /// ID of the parent section for nested structures.
    pub parent_id: Option<BkfSectionId>,
    /// List of block IDs contained within this section.
    pub block_ids: Vec<BkfBlockId>,
}

/// Canonical document representation containing all structural, semantic, and factual knowledge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BKFDocument {
    pub(crate) id: BkfDocumentId,
    pub(crate) schema_name: String,
    pub(crate) schema_version: String,
    pub(crate) capabilities: BKFCapabilities,
    pub(crate) metadata: Metadata,
    pub(crate) sections: Vec<Section>,
    pub(crate) blocks: Vec<Block>,
    pub(crate) entities: Vec<Entity>,
    pub(crate) relationships: Vec<Relationship>,
    pub(crate) facts: Vec<Fact>,
    pub(crate) citations: Vec<Citation>,
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) provenance: Vec<Provenance>,
    pub(crate) embeddings: Vec<EmbeddingRef>,
    pub(crate) chunk_refs: Vec<ChunkRef>,
    pub(crate) tags: Vec<String>,
    pub(crate) custom_metadata: HashMap<String, serde_json::Value>,
}

impl BKFDocument {
    /// Returns the Document unique ID.
    pub fn id(&self) -> BkfDocumentId {
        self.id
    }

    /// Returns the Schema Name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the Schema Version.
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Returns the computed structural and semantic capabilities.
    pub fn capabilities(&self) -> BKFCapabilities {
        self.capabilities
    }

    /// Returns the Document Metadata.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Returns list of sections.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Returns list of blocks.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Returns list of entities.
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Returns list of relationships.
    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    /// Returns list of facts.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// Returns list of citations.
    pub fn citations(&self) -> &[Citation] {
        &self.citations
    }

    /// Returns list of attachments.
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// Returns provenance history chain.
    pub fn provenance(&self) -> &[Provenance] {
        &self.provenance
    }

    /// Returns list of embedding references.
    pub fn embeddings(&self) -> &[EmbeddingRef] {
        &self.embeddings
    }

    /// Returns list of chunk references.
    pub fn chunk_refs(&self) -> &[ChunkRef] {
        &self.chunk_refs
    }

    /// Returns document classification tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns custom extensible metadata.
    pub fn custom_metadata(&self) -> &HashMap<String, serde_json::Value> {
        &self.custom_metadata
    }

    /// Validates internal schema version structure.
    pub fn validate_schema(&self) -> Result<(), semver::Error> {
        let _ = semver::Version::parse(&self.schema_version)?;
        Ok(())
    }
}
