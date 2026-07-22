use brain_domain::{
    FindingEvidence, FindingKind, ReflectionDomainCommand, ReflectionFinding, ReflectionPassId,
    ReflectionPlan, ReflectionPolicy, ReflectionRecommendation,
};

/// Evaluates reflection findings, generates recommendations with strict total ordering, and applies policy filters.
#[derive(Default)]
pub struct ReflectionPlanner {
    policy: ReflectionPolicy,
}

impl ReflectionPlanner {
    /// Creates a new `ReflectionPlanner` with default policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `ReflectionPlanner` with a custom `ReflectionPolicy`.
    pub fn with_policy(policy: ReflectionPolicy) -> Self {
        Self { policy }
    }

    /// Backwards-compatible constructor for custom threshold values.
    pub fn with_thresholds(
        duplicate_confidence_threshold: f64,
        link_suggestion_confidence_threshold: f64,
    ) -> Self {
        Self {
            policy: ReflectionPolicy {
                auto_merge_confidence_threshold: duplicate_confidence_threshold,
                auto_link_confidence_threshold: link_suggestion_confidence_threshold,
            },
        }
    }

    /// Evaluates findings, generates candidate recommendations, enforces total ordering, and applies policy rules.
    pub fn plan(&self, findings: Vec<ReflectionFinding>) -> ReflectionPlan {
        let mut recommendations = Vec::new();
        let mut skipped_findings = Vec::new();
        let findings_processed = findings.len();

        // 1. Generate candidate recommendations from findings
        for finding in findings {
            match finding {
                ReflectionFinding::DuplicateFound {
                    node_a,
                    node_b,
                    evidence,
                } => {
                    let (canonical_id, duplicate_id) = if node_a < node_b {
                        (node_a, node_b)
                    } else {
                        (node_b, node_a)
                    };

                    recommendations.push(ReflectionRecommendation {
                        pass_id: ReflectionPassId::DuplicateDetection,
                        finding_kind: FindingKind::Duplicate,
                        confidence: evidence.confidence,
                        target_ids: vec![canonical_id, duplicate_id],
                        rationale: evidence.details.clone(),
                        command: ReflectionDomainCommand::MergeConcepts {
                            canonical_id,
                            duplicate_id,
                        },
                    });
                }
                ReflectionFinding::ContradictionFound {
                    node_id,
                    property_key,
                    values,
                    evidence,
                } => {
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
                ReflectionFinding::LinkSuggested {
                    source_id,
                    target_id,
                    relation_kind,
                    evidence,
                } => {
                    recommendations.push(ReflectionRecommendation {
                        pass_id: ReflectionPassId::LinkSuggestion,
                        finding_kind: FindingKind::LinkSuggestion,
                        confidence: evidence.confidence,
                        target_ids: vec![source_id, target_id],
                        rationale: evidence.details.clone(),
                        command: ReflectionDomainCommand::CreateInferredRelation {
                            source_id,
                            target_id,
                            relation_kind,
                            confidence: evidence.confidence,
                        },
                    });
                }
            }
        }

        // 2. Enforce Deterministic Total Ordering on Recommendations
        // Order by:
        // 1. Descending confidence
        // 2. FindingKind display string
        // 3. First target_id
        // 4. Second target_id
        recommendations.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.finding_kind.to_string().cmp(&b.finding_kind.to_string()))
                .then_with(|| {
                    let a_t0 = a.target_ids.first();
                    let b_t0 = b.target_ids.first();
                    a_t0.cmp(&b_t0)
                })
                .then_with(|| {
                    let a_t1 = a.target_ids.get(1);
                    let b_t1 = b.target_ids.get(1);
                    a_t1.cmp(&b_t1)
                })
        });

        // 3. Evaluate Policy rules against recommendations to produce domain commands
        let mut commands = Vec::new();
        for rec in &recommendations {
            match &rec.command {
                ReflectionDomainCommand::MergeConcepts {
                    canonical_id,
                    duplicate_id,
                } => {
                    if rec.confidence >= self.policy.auto_merge_confidence_threshold {
                        commands.push(rec.command.clone());
                    } else {
                        skipped_findings.push((
                            ReflectionFinding::DuplicateFound {
                                node_a: *canonical_id,
                                node_b: *duplicate_id,
                                evidence: FindingEvidence {
                                    confidence: rec.confidence,
                                    semantic_similarity: None,
                                    edit_distance: None,
                                    overlap_ratio: None,
                                    details: rec.rationale.clone(),
                                },
                            },
                            format!(
                                "Confidence below merge threshold ({})",
                                self.policy.auto_merge_confidence_threshold
                            ),
                        ));
                    }
                }
                ReflectionDomainCommand::CreateInferredRelation {
                    source_id,
                    target_id,
                    relation_kind,
                    ..
                } => {
                    if rec.confidence >= self.policy.auto_link_confidence_threshold {
                        commands.push(rec.command.clone());
                    } else {
                        skipped_findings.push((
                            ReflectionFinding::LinkSuggested {
                                source_id: *source_id,
                                target_id: *target_id,
                                relation_kind: *relation_kind,
                                evidence: FindingEvidence {
                                    confidence: rec.confidence,
                                    semantic_similarity: None,
                                    edit_distance: None,
                                    overlap_ratio: None,
                                    details: rec.rationale.clone(),
                                },
                            },
                            format!(
                                "Confidence below link inference threshold ({})",
                                self.policy.auto_link_confidence_threshold
                            ),
                        ));
                    }
                }
            }
        }

        ReflectionPlan {
            recommendations,
            commands,
            findings_processed,
            skipped_findings,
        }
    }
}
