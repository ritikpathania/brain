use crate::bkf::blocks::Block;
use crate::bkf::capabilities::BKFCapabilities;
use crate::bkf::document::{BKFDocument, Section};
use crate::bkf::entities::Entity;
use crate::bkf::errors::BkfError;
use crate::bkf::facts::{BKFTargetRef, Fact, FactObject};
use crate::bkf::ids::*;
use crate::bkf::metadata::Metadata;
use crate::bkf::provenance::Provenance;
use crate::bkf::references::{Attachment, ChunkRef, Citation, EmbeddingRef};
use crate::bkf::relationships::Relationship;
use std::collections::{HashMap, HashSet};

/// Incremental builder for stream-constructing a canonical, immutable `BKFDocument`.
pub struct BKFDocumentBuilder {
    id: BkfDocumentId,
    schema_name: String,
    schema_version: String,
    metadata: Option<Metadata>,
    sections: Vec<Section>,
    blocks: Vec<Block>,
    entities: Vec<Entity>,
    relationships: Vec<Relationship>,
    facts: Vec<Fact>,
    citations: Vec<Citation>,
    attachments: Vec<Attachment>,
    provenance: Vec<Provenance>,
    embeddings: Vec<EmbeddingRef>,
    chunk_refs: Vec<ChunkRef>,
    tags: Vec<String>,
    custom_metadata: HashMap<String, serde_json::Value>,
}

impl BKFDocumentBuilder {
    /// Creates a new builder instance.
    pub fn new(id: BkfDocumentId, schema_name: String, schema_version: String) -> Self {
        Self {
            id,
            schema_name,
            schema_version,
            metadata: None,
            sections: Vec::new(),
            blocks: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
            facts: Vec::new(),
            citations: Vec::new(),
            attachments: Vec::new(),
            provenance: Vec::new(),
            embeddings: Vec::new(),
            chunk_refs: Vec::new(),
            tags: Vec::new(),
            custom_metadata: HashMap::new(),
        }
    }

    /// Sets the document metadata.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Appends a Section to the document structure.
    pub fn add_section(&mut self, section: Section) {
        self.sections.push(section);
    }

    /// Appends a Block to the document structure.
    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }

    /// Appends an Entity.
    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    /// Appends a Relationship.
    pub fn add_relationship(&mut self, relationship: Relationship) {
        self.relationships.push(relationship);
    }

    /// Appends a Fact.
    pub fn add_fact(&mut self, fact: Fact) {
        self.facts.push(fact);
    }

    /// Appends a Citation.
    pub fn add_citation(&mut self, citation: Citation) {
        self.citations.push(citation);
    }

    /// Appends an Attachment.
    pub fn add_attachment(&mut self, attachment: Attachment) {
        self.attachments.push(attachment);
    }

    /// Appends a Provenance source record.
    pub fn add_provenance(&mut self, provenance: Provenance) {
        self.provenance.push(provenance);
    }

    /// Appends an embedding index reference.
    pub fn add_embedding(&mut self, embedding: EmbeddingRef) {
        self.embeddings.push(embedding);
    }

    /// Appends a chunk offset reference.
    pub fn add_chunk_ref(&mut self, chunk_ref: ChunkRef) {
        self.chunk_refs.push(chunk_ref);
    }

    /// Adds a tag to the document.
    pub fn add_tag(&mut self, tag: String) {
        self.tags.push(tag);
    }

    /// Inserts a custom metadata key-value.
    pub fn insert_custom_metadata(&mut self, key: String, value: serde_json::Value) {
        self.custom_metadata.insert(key, value);
    }

    /// Validates, freezes, and constructs the final immutable `BKFDocument`.
    pub fn build(self) -> Result<BKFDocument, BkfError> {
        // 1. Assert that metadata has been populated
        let metadata = self.metadata.ok_or(BkfError::MissingMetadata)?;

        // 2. Validate version structure
        let _ = semver::Version::parse(&self.schema_version)
            .map_err(|e| BkfError::InvalidSchema(e.to_string()))?;

        // 3. Construct set of all defined identifiers to check referential integrity
        let mut defined_ids = HashSet::new();
        defined_ids.insert(BKFTargetRef::Document(self.id));

        let mut duplicate_checker = HashSet::new();

        for sec in &self.sections {
            if !duplicate_checker.insert(sec.id.0) {
                return Err(BkfError::DuplicateId(sec.id.0));
            }
            defined_ids.insert(BKFTargetRef::Section(sec.id));
        }
        for blk in &self.blocks {
            if !duplicate_checker.insert(blk.id.0) {
                return Err(BkfError::DuplicateId(blk.id.0));
            }
            defined_ids.insert(BKFTargetRef::Block(blk.id));
        }
        for ent in &self.entities {
            if !duplicate_checker.insert(ent.id.0) {
                return Err(BkfError::DuplicateId(ent.id.0));
            }
            defined_ids.insert(BKFTargetRef::Entity(ent.id));
        }
        for rel in &self.relationships {
            if !duplicate_checker.insert(rel.id.0) {
                return Err(BkfError::DuplicateId(rel.id.0));
            }
            defined_ids.insert(BKFTargetRef::Relationship(rel.id));
        }
        for fct in &self.facts {
            if !duplicate_checker.insert(fct.id.0) {
                return Err(BkfError::DuplicateId(fct.id.0));
            }
            defined_ids.insert(BKFTargetRef::Fact(fct.id));
        }
        for cit in &self.citations {
            if !duplicate_checker.insert(cit.id.0) {
                return Err(BkfError::DuplicateId(cit.id.0));
            }
            defined_ids.insert(BKFTargetRef::Citation(cit.id));
        }
        for att in &self.attachments {
            if !duplicate_checker.insert(att.id.0) {
                return Err(BkfError::DuplicateId(att.id.0));
            }
            defined_ids.insert(BKFTargetRef::Attachment(att.id));
        }

        // 4. Validate Section structure contiguous block list and acyclic hierarchy
        let mut section_by_id = HashMap::new();
        for sec in &self.sections {
            section_by_id.insert(sec.id, sec);
            for blk_id in &sec.block_ids {
                if !defined_ids.contains(&BKFTargetRef::Block(*blk_id)) {
                    return Err(BkfError::MissingReference {
                        id: format!("Section:{}", sec.id),
                        target: BKFTargetRef::Block(*blk_id),
                    });
                }
            }
        }

        // Detect cycles in sections hierarchy
        for sec in &self.sections {
            let mut current = sec;
            let mut visited = HashSet::new();
            visited.insert(current.id);
            while let Some(parent_id) = current.parent_id {
                if !visited.insert(parent_id) {
                    return Err(BkfError::CycleDetected { id: parent_id });
                }
                if let Some(parent) = section_by_id.get(&parent_id) {
                    current = parent;
                } else {
                    return Err(BkfError::MissingReference {
                        id: format!("Section:{}", current.id),
                        target: BKFTargetRef::Section(parent_id),
                    });
                }
            }
        }

        // 5. Validate Relationships (referential integrity, self-referencing check)
        for rel in &self.relationships {
            if rel.source == rel.target {
                return Err(BkfError::SelfReferencing { id: rel.id });
            }
            if !defined_ids.contains(&rel.source) {
                return Err(BkfError::MissingReference {
                    id: format!("Relationship:{}", rel.id),
                    target: rel.source,
                });
            }
            if !defined_ids.contains(&rel.target) {
                return Err(BkfError::MissingReference {
                    id: format!("Relationship:{}", rel.id),
                    target: rel.target,
                });
            }
        }

        // 6. Validate Facts
        for fct in &self.facts {
            if !defined_ids.contains(&fct.subject) {
                return Err(BkfError::MissingReference {
                    id: format!("Fact:{}", fct.id),
                    target: fct.subject,
                });
            }
            if let FactObject::Entity(ent_id) = fct.object {
                if !defined_ids.contains(&BKFTargetRef::Entity(ent_id)) {
                    return Err(BkfError::MissingReference {
                        id: format!("Fact:{}", fct.id),
                        target: BKFTargetRef::Entity(ent_id),
                    });
                }
            }
        }

        // 7. Compute capabilities dynamically
        let capabilities = BKFCapabilities {
            has_sections: !self.sections.is_empty(),
            has_blocks: !self.blocks.is_empty(),
            has_entities: !self.entities.is_empty(),
            has_relationships: !self.relationships.is_empty(),
            has_facts: !self.facts.is_empty(),
            has_citations: !self.citations.is_empty(),
            has_attachments: !self.attachments.is_empty(),
            has_embeddings: !self.embeddings.is_empty(),
        };

        Ok(BKFDocument {
            id: self.id,
            schema_name: self.schema_name,
            schema_version: self.schema_version,
            capabilities,
            metadata,
            sections: self.sections,
            blocks: self.blocks,
            entities: self.entities,
            relationships: self.relationships,
            facts: self.facts,
            citations: self.citations,
            attachments: self.attachments,
            provenance: self.provenance,
            embeddings: self.embeddings,
            chunk_refs: self.chunk_refs,
            tags: self.tags,
            custom_metadata: self.custom_metadata,
        })
    }
}
