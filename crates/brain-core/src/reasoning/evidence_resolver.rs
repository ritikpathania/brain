//! EvidenceResolver service materializing ArtifactViews lazily via iterator pipelines from EvidenceSets.

use brain_domain::{ArtifactStore, ArtifactView, EvidenceSet};

/// Dedicated resolution component separating evidence selection from artifact view materialization.
#[derive(Debug, Clone, Default)]
pub struct EvidenceResolver;

impl EvidenceResolver {
    /// Instantiates a new `EvidenceResolver`.
    pub fn new() -> Self {
        Self
    }

    /// Resolves an `EvidenceSet` against an `ArtifactStore` into a lazy zero-allocation iterator of `ArtifactView` items.
    pub fn resolve_iter<'a>(
        &self,
        set: &'a EvidenceSet,
        store: &'a ArtifactStore,
    ) -> impl Iterator<Item = ArtifactView<'a>> {
        set.iter().filter_map(move |id| store.get(*id))
    }

    /// Resolves an `EvidenceSet` against an `ArtifactStore` into a vector of `ArtifactView` items.
    pub fn resolve<'a>(&self, set: &'a EvidenceSet, store: &'a ArtifactStore) -> Vec<ArtifactView<'a>> {
        self.resolve_iter(set, store).collect()
    }
}
