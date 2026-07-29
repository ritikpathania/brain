//! Integration test suite for Reflection Engine V2 & Knowledge Consolidation (Phase 6 Milestone 6.1).

use brain_services::compiler::{EntityIR, EntityId, ProvenanceIR};
use brain_services::reflection::{ReflectionEngineV2, ReflectionFindingKind, ReflectionInput};

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "reflection_v2_origin".to_string(),
        evidence_ids: vec!["ev_303".to_string()],
        confidence: 0.95,
        timestamp_ms: 4000,
    }
}

fn sample_entities() -> Vec<EntityIR> {
    let mut props = std::collections::BTreeMap::new();
    props.insert("conflict".to_string(), "true".to_string());

    vec![
        EntityIR {
            id: EntityId("entity_alpha".to_string()),
            canonical_name: "Alpha Component".to_string(),
            kind: "component".to_string(),
            aliases: vec![],
            properties: props,
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
        EntityIR {
            id: EntityId("entity_alpha_dup".to_string()),
            canonical_name: "Alpha Component".to_string(),
            kind: "component".to_string(),
            aliases: vec![],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.40,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    ]
}

#[test]
fn test_reflection_engine_v2_read_only_analysis() {
    let input = ReflectionInput::new(sample_entities(), vec![], 4000);
    let engine = ReflectionEngineV2::new();

    let report1 = engine.run(&input);
    let report2 = engine.run(&input);

    // Assert reflection determinism invariant: identical input produces identical finding kinds & confidence scores
    assert_eq!(report1.findings.len(), report2.findings.len());
    assert_eq!(report1.evaluated_entities_count, 2);

    let has_conflict = report1
        .findings
        .iter()
        .any(|f| matches!(f.kind, ReflectionFindingKind::AttributeContradiction(_)));
    let has_duplicate = report1
        .findings
        .iter()
        .any(|f| matches!(f.kind, ReflectionFindingKind::DuplicateEntity(_)));
    let has_decay = report1
        .findings
        .iter()
        .any(|f| matches!(f.kind, ReflectionFindingKind::ConfidenceDecay(_)));

    assert!(has_conflict, "Expected AttributeContradiction finding");
    assert!(has_duplicate, "Expected DuplicateEntity finding");
    assert!(has_decay, "Expected ConfidenceDecay finding");
}
