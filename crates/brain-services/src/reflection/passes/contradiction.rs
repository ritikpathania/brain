//! Contradiction detection pass for exclusive predicate conflict resolution in Reflection Engine v2.

use crate::reflection::pass_context::*;
use brain_domain::bkf::*;
use std::collections::HashMap;

/// Legacy contradiction pass.
pub use legacy::ContradictionPass;

mod legacy {
    use crate::reflection::{ReflectionContext, ReflectionPass, ReflectionSnapshot};
    use brain_core::errors::BrainError;
    use brain_domain::{FindingEvidence, ReflectionFinding, ReflectionPassId};

    /// Legacy pass scanning node properties for logical or scalar contradictions.
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

    impl ReflectionPass for ContradictionPass {
        fn id(&self) -> ReflectionPassId {
            ReflectionPassId::Contradiction
        }

        fn version(&self) -> u32 {
            1
        }

        fn run(
            &self,
            snapshot: &ReflectionSnapshot,
            _context: &ReflectionContext,
        ) -> Result<Vec<ReflectionFinding>, BrainError> {
            let repos = snapshot.repositories();
            let nodes = repos.nodes().list_all()?;
            let mut findings = Vec::new();

            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    let node_a = &nodes[i];
                    let node_b = &nodes[j];

                    let label_a = node_a.label.to_lowercase();
                    let label_b = node_b.label.to_lowercase();

                    if label_a == label_b
                        || label_a.contains(&label_b)
                        || label_b.contains(&label_a)
                    {
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
                                                "Node {} vs Node {} property conflict on {}",
                                                node_a.id, node_b.id, key
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
}

/// Reflection v2 pass detecting conflicts across exclusive predicates and closing superseded temporal windows.
pub struct V2ContradictionPass;

impl V2ReflectionPass for V2ContradictionPass {
    fn id(&self) -> PassId {
        PassId::new("contradiction")
    }

    fn dependencies(&self) -> &[PassId] {
        lazy_static_deps()
    }

    fn analyze(
        &self,
        snapshot: &dyn KnowledgeSnapshotView,
        _context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        // 1. Index exclusive predicates
        let exclusive_predicates: HashMap<PredicateId, &Predicate> = snapshot
            .predicates()
            .iter()
            .filter(|p| p.cardinality == PredicateCardinality::Exclusive)
            .map(|p| (p.id, p))
            .collect();

        if exclusive_predicates.is_empty() {
            return Ok(None);
        }

        // 2. Map assertion IDs to assertions
        let assertions_by_id: HashMap<AssertionId, &SemanticAssertion> =
            snapshot.assertions().iter().map(|a| (a.id, a)).collect();

        // 3. Group active facts by (subject, predicate)
        let mut grouped_facts: HashMap<(KnowledgeEntityId, PredicateId), Vec<&FactVersion>> =
            HashMap::new();

        for fact in snapshot.active_facts() {
            if let Some(assertion) = assertions_by_id.get(&fact.assertion_id) {
                if exclusive_predicates.contains_key(&assertion.predicate) {
                    grouped_facts
                        .entry((assertion.subject, assertion.predicate))
                        .or_default()
                        .push(fact);
                }
            }
        }

        let mut ops = Vec::new();
        let mut diagnostics = Vec::new();

        for ((_subject, _pred_id), mut facts) in grouped_facts {
            if facts.len() <= 1 {
                continue;
            }

            // Sort facts by valid_from timestamp (newest last)
            facts.sort_by_key(|a| a.temporal.valid_from);

            let newest_fact = facts.last().unwrap();

            // All earlier active facts are superseded by the newest fact
            for older_fact in &facts[..facts.len() - 1] {
                ops.push(RewriteOperation::SupersedeFact {
                    old_fact_id: older_fact.id,
                    new_fact_id: newest_fact.id,
                    closed_at: newest_fact.temporal.valid_from,
                });

                diagnostics.push(PassDiagnostic {
                    severity: DiagnosticSeverity::Info,
                    code: "CONTRADICTION_SUPERSEDED".to_string(),
                    message: format!(
                        "Fact {} superseded by newer fact {} at timestamp {:?}",
                        older_fact.id.0, newest_fact.id.0, newest_fact.temporal.valid_from
                    ),
                });
            }
        }

        if ops.is_empty() {
            return Ok(None);
        }

        let plan = RewritePlan {
            pass_id: self.id(),
            reason: RewriteReason::Contradiction,
            rationale: format!("Superseded {} conflicting exclusive facts", ops.len()),
            execution_cost: ops.len() as u32,
            operations: ops,
        };

        Ok(Some(ReflectionOutcome { plan, diagnostics }))
    }
}

fn lazy_static_deps() -> &'static [PassId] {
    use std::sync::OnceLock;
    static DEPS: OnceLock<Vec<PassId>> = OnceLock::new();
    DEPS.get_or_init(|| vec![PassId::new("canonicalization")])
}
