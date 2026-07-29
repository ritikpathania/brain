//! Unit and pipeline idempotence tests for Milestone A3 reconciliation passes.

use brain_domain::{
    CanonicalEntity, ContradictionKind, EntityId, KnowledgeEvidence, KnowledgeState,
};
use brain_services::reconciliation::{
    AliasResolutionPass, ContradictionDetectionPass, DefaultOrphanPolicy, DuplicateDetectionPass,
    EntityNormalizationPass, OrphanDetectionPass, ReconciliationPass, ReconciliationPipeline,
};
use std::collections::HashMap;

#[test]
fn test_duplicate_detection_pass_proposals_and_non_mutation() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "acme corp".to_string(),
            aliases: vec!["acme".to_string()],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id2,
            preferred_name: "acme corp".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
    ];

    let pass = DuplicateDetectionPass::new(0.8);
    let report = pass.execute(&mut entities);

    // Verify non-mutation invariant
    assert_eq!(report.changes_applied, 0);
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].id, id1);
    assert_eq!(entities[1].id, id2);

    // Verify proposal generation
    assert_eq!(report.merge_proposals.len(), 1);
    let proposal = &report.merge_proposals[0];
    assert_eq!(proposal.source_entity_id, id2);
    assert_eq!(proposal.target_entity_id, id1);
    assert_eq!(proposal.confidence, 1.0);
    assert!(proposal.feature_scores.contains_key("lexical_similarity"));
}

#[test]
fn test_contradiction_detection_pass_classification() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "quantum computing".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id2,
            preferred_name: "quantum computing".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Archived,
        },
    ];

    let pass = ContradictionDetectionPass::new();
    let report = pass.execute(&mut entities);

    assert_eq!(report.contradiction_records.len(), 1);
    let record = &report.contradiction_records[0];
    assert_eq!(record.kind, ContradictionKind::Temporal);
    assert_eq!(record.entity_a, id1);
    assert_eq!(record.entity_b, id2);
}

#[test]
fn test_orphan_detection_pass_policy_driven() {
    let id_valid = EntityId::new();
    let id_orphan = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id_valid,
            preferred_name: "valid concept".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence {
                retention: Default::default(),
                source_reliability: 0.9,
            },
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id_orphan,
            preferred_name: "low reliability noise".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence {
                retention: Default::default(),
                source_reliability: 0.1,
            },
            state: KnowledgeState::Observed,
        },
    ];

    let pass = OrphanDetectionPass::with_policy(DefaultOrphanPolicy {
        min_reliability: 0.2,
    });
    let report = pass.execute(&mut entities);

    assert_eq!(report.changes_applied, 1);
    assert_eq!(entities[0].state, KnowledgeState::Observed);
    assert_eq!(entities[1].state, KnowledgeState::Weak);
}

#[test]
fn test_full_5_pass_pipeline_idempotence_and_stability() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "  RUST LANG  ".to_string(),
            aliases: vec![" rust ".to_string()],
            merge_history: vec![],
            evidence: KnowledgeEvidence {
                retention: Default::default(),
                source_reliability: 0.95,
            },
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id2,
            preferred_name: "RUST LANG".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence {
                retention: Default::default(),
                source_reliability: 0.95,
            },
            state: KnowledgeState::Observed,
        },
    ];

    let pipeline = ReconciliationPipeline::new()
        .with_pass(EntityNormalizationPass::new())
        .with_pass(AliasResolutionPass::new(HashMap::new()))
        .with_pass(DuplicateDetectionPass::new(0.8))
        .with_pass(ContradictionDetectionPass::new())
        .with_pass(OrphanDetectionPass::new());

    // First Execution
    let reports_run1 = pipeline.execute(&mut entities);
    assert_eq!(reports_run1.len(), 5);
    assert_eq!(entities[0].preferred_name, "rust lang");
    assert_eq!(entities[0].aliases, vec!["rust"]);

    let snapshot_after_run1 = entities.clone();

    // Second Execution (Idempotence Assertion P(P(G)) == P(G))
    let reports_run2 = pipeline.execute(&mut entities);
    assert_eq!(reports_run2.len(), 5);

    // Assert zero mutations on run 2
    for report in &reports_run2 {
        assert_eq!(
            report.changes_applied, 0,
            "Pass {} applied unexpected changes on run 2",
            report.pass_name
        );
    }

    assert_eq!(
        entities, snapshot_after_run1,
        "Entity snapshot mutated on second pass"
    );
}
