use brain_domain::{
    ReflectionDomainCommand, ReflectionFinding, ReflectionPlan,
};

/// Evaluates reflection findings and plans domain commands.
pub struct ReflectionPlanner {
    duplicate_confidence_threshold: f64,
    link_suggestion_confidence_threshold: f64,
}

impl Default for ReflectionPlanner {
    fn default() -> Self {
        Self {
            duplicate_confidence_threshold: 0.92,
            link_suggestion_confidence_threshold: 0.85,
        }
    }
}

impl ReflectionPlanner {
    /// Creates a new `ReflectionPlanner`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `ReflectionPlanner` with custom thresholds.
    pub fn with_thresholds(
        duplicate_confidence_threshold: f64,
        link_suggestion_confidence_threshold: f64,
    ) -> Self {
        Self {
            duplicate_confidence_threshold,
            link_suggestion_confidence_threshold,
        }
    }

    /// Evaluates findings and generates a consolidation plan.
    pub fn plan(&self, findings: Vec<ReflectionFinding>) -> ReflectionPlan {
        let mut commands = Vec::new();
        let mut skipped_findings = Vec::new();
        let findings_processed = findings.len();

        for finding in findings {
            match finding {
                ReflectionFinding::DuplicateFound { node_a, node_b, evidence } => {
                    // Configured threshold for merging duplicates
                    if evidence.confidence >= self.duplicate_confidence_threshold {
                        // Canonical node is chosen as the one with smaller ID (deterministic tie-breaking)
                        let (canonical_id, duplicate_id) = if node_a < node_b {
                            (node_a, node_b)
                        } else {
                            (node_b, node_a)
                        };
                        commands.push(ReflectionDomainCommand::MergeConcepts {
                            canonical_id,
                            duplicate_id,
                        });
                    } else {
                        skipped_findings.push((
                            ReflectionFinding::DuplicateFound { node_a, node_b, evidence },
                            format!("Confidence below merge threshold ({})", self.duplicate_confidence_threshold),
                        ));
                    }
                }
                ReflectionFinding::ContradictionFound { node_id, property_key, values, evidence } => {
                    // Contradictions are currently logged and skipped (awaiting manual intervention)
                    skipped_findings.push((
                        ReflectionFinding::ContradictionFound {
                            node_id,
                            property_key,
                            values,
                            evidence,
                        },
                        "Contradiction resolution requires human approval policy".to_string(),
                    ));
                }
                ReflectionFinding::LinkSuggested { source_id, target_id, relation_kind, evidence } => {
                    // Configured threshold for automatic inference
                    if evidence.confidence >= self.link_suggestion_confidence_threshold {
                        commands.push(ReflectionDomainCommand::CreateInferredRelation {
                            source_id,
                            target_id,
                            relation_kind,
                            confidence: evidence.confidence,
                        });
                    } else {
                        skipped_findings.push((
                            ReflectionFinding::LinkSuggested {
                                source_id,
                                target_id,
                                relation_kind,
                                evidence,
                            },
                            format!("Confidence below link inference threshold ({})", self.link_suggestion_confidence_threshold),
                        ));
                    }
                }
            }
        }

        ReflectionPlan {
            commands,
            findings_processed,
            skipped_findings,
        }
    }
}
