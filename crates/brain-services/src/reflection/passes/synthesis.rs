use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_domain::{
    FindingEvidence, ReflectionFinding, RelationKind,
};
use crate::reflection::ReflectionContext;
use std::collections::HashSet;

/// Pass consolidating highly-connected clusters by closing triangles of generic associations.
pub struct SynthesisPass;

impl SynthesisPass {
    /// Creates a new `SynthesisPass`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SynthesisPass {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::reflection::ReflectionPass for SynthesisPass {
    fn run(
        &self,
        snapshot: &dyn RepositorySet,
        _context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError> {
        let edges = snapshot.edges().list_all()?;
        let mut findings = Vec::new();

        // 1. Gather all direct AssociatedWith links
        let mut adj: std::collections::HashMap<brain_domain::NodeId, Vec<brain_domain::NodeId>> = std::collections::HashMap::new();
        let mut existing_edges = HashSet::new();

        for edge in &edges {
            existing_edges.insert((edge.source, edge.target, edge.relation.id()));

            if edge.relation == RelationKind::AssociatedWith {
                adj.entry(edge.source)
                    .or_default()
                    .push(edge.target);
            }
        }

        // 2. Scan for transitive triads: A -AssociatedWith-> B -AssociatedWith-> C
        // and suggest missing A -AssociatedWith-> C
        for (&a, neighbors) in &adj {
            for &b in neighbors {
                if let Some(b_neighbors) = adj.get(&b) {
                    for &c in b_neighbors {
                        if a == c {
                            continue; // skip self loops
                        }

                        let target_rel_id = RelationKind::AssociatedWith.id();
                        let key = (a, c, target_rel_id.clone());

                        if !existing_edges.contains(&key) {
                            findings.push(ReflectionFinding::LinkSuggested {
                                source_id: a,
                                target_id: c,
                                relation_kind: RelationKind::AssociatedWith,
                                evidence: FindingEvidence {
                                    confidence: 0.7,
                                    semantic_similarity: None,
                                    edit_distance: None,
                                    overlap_ratio: None,
                                    details: "Synthesized transitive closure of AssociatedWith triad".to_string(),
                                },
                            });
                        }
                    }
                }
            }
        }

        Ok(findings)
    }
}
