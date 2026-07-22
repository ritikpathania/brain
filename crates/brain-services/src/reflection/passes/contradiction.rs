use crate::reflection::ReflectionContext;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_domain::{FindingEvidence, ReflectionFinding};

/// Pass scanning node properties for logical or scalar contradictions.
pub struct ContradictionPass;

impl ContradictionPass {
    /// Creates a new `ContradictionPass`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContradictionPass {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::reflection::ReflectionPass for ContradictionPass {
    fn run(
        &self,
        snapshot: &dyn RepositorySet,
        _context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError> {
        let nodes = snapshot.nodes().list_all()?;
        let mut findings = Vec::new();

        // Compare all node pairs for conflicting property assertions
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let node_a = &nodes[i];
                let node_b = &nodes[j];

                let label_a = node_a.label.to_lowercase();
                let label_b = node_b.label.to_lowercase();

                // Compute basic syntactic match to see if they refer to the same concept
                if label_a == label_b || label_a.contains(&label_b) || label_b.contains(&label_a) {
                    // Compare their properties for contradictions
                    for (key, val_a) in &node_a.properties {
                        if let Some(val_b) = node_b.properties.get(key) {
                            if val_a != val_b {
                                let confidence = 0.8;
                                findings.push(ReflectionFinding::ContradictionFound {
                                    node_id: node_a.id,
                                    property_key: key.clone(),
                                    values: vec![val_a.clone(), val_b.clone()],
                                    evidence: FindingEvidence {
                                        confidence,
                                        semantic_similarity: None,
                                        edit_distance: None,
                                        overlap_ratio: None,
                                        details: format!(
                                            "Matching concepts '{}' and '{}' assert conflicting values for key '{}': {:?} vs {:?}",
                                            node_a.label, node_b.label, key, val_a, val_b
                                        ),
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(findings)
    }
}
