//! Dirty set tracking for KPP v1.2 Incremental Compilation.

use crate::compiler::ir::{EntityId, FactId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Set of invalidated or modified entity and fact identifiers triggering incremental compilation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtySet {
    /// Associated graph version epoch counter.
    pub graph_version: u64,
    /// Explicitly dirtied entity IDs.
    pub dirty_entities: BTreeSet<EntityId>,
    /// Explicitly dirtied fact IDs.
    pub dirty_facts: BTreeSet<FactId>,
    /// Explicitly dirtied relation pairs (source, target).
    pub dirty_relations: Vec<(EntityId, EntityId)>,
    /// Flag indicating fallback to full graph re-compilation.
    pub is_full_recompile: bool,
}

impl DirtySet {
    /// Creates a new empty `DirtySet` for a specific graph version.
    pub fn new(graph_version: u64) -> Self {
        Self {
            graph_version,
            dirty_entities: BTreeSet::new(),
            dirty_facts: BTreeSet::new(),
            dirty_relations: Vec::new(),
            is_full_recompile: false,
        }
    }

    /// Creates a `DirtySet` forcing full graph re-compilation.
    pub fn full_recompile(graph_version: u64) -> Self {
        let mut ds = Self::new(graph_version);
        ds.is_full_recompile = true;
        ds
    }

    /// Marks an entity ID as dirty.
    pub fn mark_entity(&mut self, entity_id: EntityId) {
        self.dirty_entities.insert(entity_id);
    }

    /// Marks a fact ID as dirty.
    pub fn mark_fact(&mut self, fact_id: FactId) {
        self.dirty_facts.insert(fact_id);
    }

    /// Marks a relation pair as dirty.
    pub fn mark_relation(&mut self, source_id: EntityId, target_id: EntityId) {
        self.dirty_relations.push((source_id, target_id));
    }

    /// Returns `true` if the given entity ID is dirty or if full re-compilation is requested.
    pub fn is_entity_dirty(&self, entity_id: &EntityId) -> bool {
        self.is_full_recompile || self.dirty_entities.contains(entity_id)
    }

    /// Returns `true` if the given fact ID is dirty or if full re-compilation is requested.
    pub fn is_fact_dirty(&self, fact_id: &FactId) -> bool {
        self.is_full_recompile || self.dirty_facts.contains(fact_id)
    }

    /// Returns `true` if the given relation edge is dirty or if full re-compilation is requested.
    pub fn is_relation_dirty(&self, source_id: &EntityId, target_id: &EntityId) -> bool {
        self.is_full_recompile
            || self
                .dirty_relations
                .iter()
                .any(|(s, t)| s == source_id && t == target_id)
    }

    /// Returns `true` if no entities or facts are marked dirty.
    pub fn is_empty(&self) -> bool {
        !self.is_full_recompile
            && self.dirty_entities.is_empty()
            && self.dirty_facts.is_empty()
            && self.dirty_relations.is_empty()
    }
}
