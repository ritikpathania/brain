//! Configuration models for Knowledge Compiler optimization passes and retention policies.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configurable policy parameters for Knowledge Compiler optimization and retention passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerOptimizationConfig {
    /// Maximum number of provenance items preserved per entity/fact during compression.
    pub provenance_limit: usize,
    /// Absolute confidence floor threshold below which non-canonical facts and orphan entities are pruned.
    pub confidence_floor: f64,
    /// Maximum retention epochs for superseded non-canonical facts.
    pub retention_epochs: u64,
    /// Explicit set of relation categories that support transitive reduction (DAG).
    pub transitive_reduction_relations: HashSet<String>,
}

impl Default for CompilerOptimizationConfig {
    fn default() -> Self {
        let mut transitive_relations = HashSet::new();
        transitive_relations.insert("parent_of".to_string());
        transitive_relations.insert("subclass_of".to_string());
        transitive_relations.insert("contains".to_string());
        transitive_relations.insert("part_of".to_string());

        Self {
            provenance_limit: 10,
            confidence_floor: 0.10,
            retention_epochs: 5,
            transitive_reduction_relations: transitive_relations,
        }
    }
}
