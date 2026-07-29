//! Idempotent Reflection Engine for deriving higher-order synthesis findings from CanonicalGraph snapshots.

use crate::compiler::graph::CanonicalGraph;
use crate::compiler::mutation::{FindingPayload, MutationKind, MutationRequest};
use brain_domain::SessionId;
use std::collections::HashSet;

/// Idempotent Reflection Engine deriving higher-level insights from canonical graph states.
#[derive(Debug, Clone, Default)]
pub struct ReflectionEngine {
    processed_synthesis_keys: HashSet<String>,
}

impl ReflectionEngine {
    /// Instantiates a new `ReflectionEngine`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyzes a `CanonicalGraph` snapshot and generates idempotent `MutationRequest::ReflectionFinding` requests.
    ///
    /// # Invariant
    /// Applying the same reflection analysis repeatedly over identical graph states is idempotent and will not
    /// produce duplicate mutations.
    pub fn analyze_graph(
        &mut self,
        session_id: SessionId,
        graph: &CanonicalGraph,
    ) -> Vec<MutationRequest> {
        let mut requests = Vec::new();

        // Discover orphan concepts or synthesis opportunities
        for (id, entity) in &graph.entities {
            let synthesis_key = format!("synth_{}_{}", id, entity.canonical_name);

            if !self.processed_synthesis_keys.contains(&synthesis_key) {
                self.processed_synthesis_keys.insert(synthesis_key.clone());

                let payload = FindingPayload {
                    analyzer: "concept_synthesis".to_string(),
                    description: format!(
                        "Synthesized canonical entity reflection for {}",
                        entity.canonical_name
                    ),
                };

                let req =
                    MutationRequest::new(session_id, MutationKind::ReflectionFinding(payload));
                requests.push(req);
            }
        }

        requests
    }
}
