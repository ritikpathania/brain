//! Pure EvidenceSelector service evaluating algebraic EvidenceQueries against an ArtifactStore.

use brain_domain::{
    ArtifactStore, EvidenceArtifactId, EvidenceQuery, EvidenceSet, SelectionStrategy,
};
use std::collections::BTreeSet;

/// Service responsible for evaluating algebraic selection strategy queries against an ArtifactStore graph.
#[derive(Debug, Clone, Default)]
pub struct EvidenceSelector;

impl EvidenceSelector {
    /// Instantiates a new `EvidenceSelector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates an `EvidenceQuery` against an `ArtifactStore` in pure functional manner.
    pub fn select(&self, store: &ArtifactStore, query: &EvidenceQuery) -> EvidenceSet {
        let set_ids = self.evaluate_strategy(store, &query.strategy);
        EvidenceSet::new(set_ids, query.strategy.clone())
    }

    fn evaluate_strategy(
        &self,
        store: &ArtifactStore,
        strategy: &SelectionStrategy,
    ) -> BTreeSet<EvidenceArtifactId> {
        match strategy {
            SelectionStrategy::All => store.all_artifact_ids().into_iter().collect(),
            SelectionStrategy::PrerequisitesOf(step_id) => store
                .get_by_producer(*step_id)
                .map(|art| std::iter::once(art.id()).collect())
                .unwrap_or_default(),
            SelectionStrategy::ByKind(target_kind) => store
                .all_artifact_views()
                .filter(|view| view.metadata().kind == *target_kind)
                .map(|view| view.id())
                .collect(),
            SelectionStrategy::AncestorsOf(target_id) => {
                store.ancestors(*target_id).into_iter().collect()
            }
            SelectionStrategy::Union(left, right) => {
                let mut set_a = self.evaluate_strategy(store, left);
                let set_b = self.evaluate_strategy(store, right);
                set_a.extend(set_b);
                set_a
            }
            SelectionStrategy::Intersection(left, right) => {
                let set_a = self.evaluate_strategy(store, left);
                let set_b = self.evaluate_strategy(store, right);
                set_a.intersection(&set_b).copied().collect()
            }
            SelectionStrategy::Difference(left, right) => {
                let set_a = self.evaluate_strategy(store, left);
                let set_b = self.evaluate_strategy(store, right);
                set_a.difference(&set_b).copied().collect()
            }
        }
    }
}
