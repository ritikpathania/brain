//! Tests for Phase A.5 Reconciliation Observability DTOs and mapping.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_services::mapper::to_pass_report_dto;
use brain_services::reconciliation::{
    ContradictionDetectionPass, DuplicateDetectionPass, EntityNormalizationPass, ReconciliationPass,
};

#[test]
fn test_reconciliation_dto_mapping_and_serialization() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let mut entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "  ACME CORP  ".to_string(),
            aliases: vec![],
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
            state: KnowledgeState::Archived,
        },
    ];

    let norm_pass = EntityNormalizationPass::new();
    let report_norm = norm_pass.execute(&mut entities);
    let dto_norm = to_pass_report_dto(&report_norm);

    assert_eq!(dto_norm.pass_name, "EntityNormalizationPass");
    assert_eq!(dto_norm.items_processed, 2);
    assert_eq!(dto_norm.changes_applied, 1);

    let dup_pass = DuplicateDetectionPass::new(0.8);
    let report_dup = dup_pass.execute(&mut entities);
    let dto_dup = to_pass_report_dto(&report_dup);

    assert_eq!(dto_dup.pass_name, "DuplicateDetectionPass");
    assert_eq!(dto_dup.merge_proposals.len(), 1);
    assert_eq!(dto_dup.merge_proposals[0].confidence, 1.0);

    let contra_pass = ContradictionDetectionPass::new();
    let report_contra = contra_pass.execute(&mut entities);
    let dto_contra = to_pass_report_dto(&report_contra);

    assert_eq!(dto_contra.pass_name, "ContradictionDetectionPass");
    assert_eq!(dto_contra.contradiction_records.len(), 1);
    assert_eq!(dto_contra.contradiction_records[0].kind, "temporal");

    // Test JSON round-trip serialization of PassReportDTO
    let json = serde_json::to_string(&dto_contra).expect("JSON serialization failed");
    assert!(json.contains("\"kind\":\"temporal\""));
}
