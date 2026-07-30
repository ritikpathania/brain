//! Knowledge snapshot view abstraction for storage-agnostic reflection passes.

use crate::bkf::fact_version::*;

/// Storage-agnostic read-only snapshot view over entities, assertions, predicates, and active facts.
pub trait KnowledgeSnapshotView: Send + Sync {
    /// Returns a slice of all active entities in the snapshot.
    fn entities(&self) -> &[KnowledgeEntity];

    /// Returns a slice of all semantic assertions in the snapshot.
    fn assertions(&self) -> &[SemanticAssertion];

    /// Returns a slice of all predicate definitions in the snapshot.
    fn predicates(&self) -> &[Predicate];

    /// Returns a slice of all active fact versions in the snapshot.
    fn active_facts(&self) -> &[FactVersion];
}
