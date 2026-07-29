//! Unit and pipeline tests for Milestone A2 reconciliation passes.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_services::reconciliation::{
    AliasResolutionPass, EntityNormalizationPass, ReconciliationPass, ReconciliationPipeline,
};
use std::collections::HashMap;

#[test]
fn test_entity_normalization_pass_idempotence_and_immutability() {
    let id = EntityId::new();
    let mut entities = vec![CanonicalEntity {
        id,
        preferred_name: "  John  DOE  ".to_string(),
        aliases: vec![
            " J. Doe ".to_string(),
            "johnny".to_string(),
            "johnny".to_string(),
        ],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    let pass = EntityNormalizationPass::new();

    // First execution
    let report1 = pass.execute(&mut entities);
    assert_eq!(entities[0].id, id);
    assert_eq!(entities[0].preferred_name, "john  doe");
    assert_eq!(entities[0].aliases, vec!["j. doe", "johnny"]);
    assert!(report1.changes_applied > 0);

    // Second execution (Idempotence check)
    let report2 = pass.execute(&mut entities);
    assert_eq!(entities[0].id, id);
    assert_eq!(entities[0].preferred_name, "john  doe");
    assert_eq!(entities[0].aliases, vec!["j. doe", "johnny"]);
    assert_eq!(report2.changes_applied, 0);
}

#[test]
fn test_alias_resolution_pass_correctness_and_idempotence() {
    let canonical_id = EntityId::new();
    let alias_entity_id = EntityId::new();

    let mut alias_map = HashMap::new();
    alias_map.insert("j. doe".to_string(), canonical_id);

    let mut entities = vec![
        CanonicalEntity {
            id: canonical_id,
            preferred_name: "john doe".to_string(),
            aliases: vec!["j. doe".to_string()],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Verified,
        },
        CanonicalEntity {
            id: alias_entity_id,
            preferred_name: "j. doe".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
    ];

    let pass = AliasResolutionPass::new(alias_map);

    // First run
    let report1 = pass.execute(&mut entities);
    assert_eq!(entities[0].id, canonical_id);
    assert_eq!(entities[1].id, alias_entity_id);
    assert_eq!(report1.diagnostics.len(), 1);

    // Second run (Idempotence)
    let report2 = pass.execute(&mut entities);
    assert_eq!(entities[0].id, canonical_id);
    assert_eq!(entities[1].id, alias_entity_id);
    assert_eq!(report2.diagnostics.len(), 1);
}

#[test]
fn test_reconciliation_pipeline_composition_and_idempotence() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "  ALICE SMITH  ".to_string(),
            aliases: vec![" alice.smith ".to_string()],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id2,
            preferred_name: "BOB JONES".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
    ];

    let pipeline = ReconciliationPipeline::new()
        .with_pass(EntityNormalizationPass::new())
        .with_pass(AliasResolutionPass::new(HashMap::new()));

    // Run 1
    let reports1 = pipeline.execute(&mut entities);
    assert_eq!(reports1.len(), 2);
    assert_eq!(entities[0].preferred_name, "alice smith");
    assert_eq!(entities[0].aliases, vec!["alice.smith"]);

    let snapshot_after_run1 = entities.clone();

    // Run 2
    let reports2 = pipeline.execute(&mut entities);
    assert_eq!(reports2.len(), 2);
    assert_eq!(entities, snapshot_after_run1);
    assert_eq!(reports2[0].changes_applied, 0);
}
