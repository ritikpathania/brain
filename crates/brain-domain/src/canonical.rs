//! Scoped lexical normalization and semantic entity canonicalization utilities.

use crate::entities::NodeType;
use crate::identifiers::NodeId;
use std::collections::HashMap;

/// Pure normalizer to clean up raw node labels.
pub struct Normalizer;

impl Normalizer {
    /// Lexically normalizes a raw string label.
    ///
    /// This function is pure and idempotent: `normalize(normalize(s)) == normalize(s)`.
    pub fn normalize(s: &str) -> String {
        s.trim().to_lowercase()
    }
}

/// Scoped alias resolver mapped per graph-building session.
///
/// Assumes all keys are pre-normalized.
#[derive(Debug, Clone, Default)]
pub struct AliasResolver {
    mappings: HashMap<String, NodeId>,
}

impl AliasResolver {
    /// Creates a new `AliasResolver` with pre-defined mappings.
    ///
    /// Keys in the map must be pre-normalized.
    pub fn new(mappings: HashMap<String, NodeId>) -> Self {
        Self { mappings }
    }

    /// Resolves a pre-normalized key to its canonical `NodeId`.
    pub fn resolve(&self, normalized_key: &str) -> Option<NodeId> {
        self.mappings.get(normalized_key).copied()
    }

    /// Inserts a new alias mapping. Key must be pre-normalized.
    pub fn insert(&mut self, normalized_alias: String, canonical_id: NodeId) {
        self.mappings.insert(normalized_alias, canonical_id);
    }
}

/// Strategies for merging properties and values when deduplicating nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    /// Keep the attributes of the first node encountered.
    TakeFirst,
    /// Merge JSON properties maps (newer properties overwrite or append).
    MergeProperties,
    /// Sum confidence or weight fields.
    SumWeight,
}

impl MergePolicy {
    /// Resolves the data-driven merge policy associated with a node type.
    pub fn for_node_type(node_type: &NodeType) -> Self {
        match node_type {
            NodeType::Concept => MergePolicy::MergeProperties,
            NodeType::Project => MergePolicy::TakeFirst,
            _ => MergePolicy::MergeProperties,
        }
    }
}

/// Canonicalizer coordinating lexical normalization and alias lookup.
#[derive(Debug, Clone)]
pub struct EntityCanonicalizer {
    resolver: AliasResolver,
}

impl EntityCanonicalizer {
    /// Creates a new `EntityCanonicalizer` with a scoped resolver.
    pub fn new(resolver: AliasResolver) -> Self {
        Self { resolver }
    }

    /// Normalizes the label and resolves it to a canonical `NodeId` if mapped.
    ///
    /// Returns a tuple containing the normalized label and the resolved `NodeId` if found.
    pub fn canonicalize(&self, label: &str) -> (String, Option<NodeId>) {
        let normalized = Normalizer::normalize(label);
        let resolved = self.resolver.resolve(&normalized);
        (normalized, resolved)
    }
}
