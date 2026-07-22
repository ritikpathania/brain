use brain_domain::SessionId;
use brain_services::compiler::{
    CompilerContext, EntityIR, EntityId, FactIR, FactId, KnowledgeCompiler, KnowledgeIR,
    PassManager, ProvenanceIR, RelationIR,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn sample_provenance() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "event_log_seq_42".to_string(),
        evidence_ids: vec!["ev_101".to_string()],
        confidence: 0.85,
        timestamp_ms: 1700000000000,
    }
}

#[test]
fn test_compiler_pass_ordering_and_ir_transformations() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
    };

    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    // Insert entity needing whitespace normalization
    ir.insert_entity(EntityIR {
        id: EntityId("entity_rust".to_string()),
        canonical_name: "  Rust Programming Language  ".to_string(),
        kind: "concept".to_string(),
        aliases: vec!["Rust".to_string()],
        properties: Default::default(),
        confidence: 0.95,
        provenance: sample_provenance(),
    });

    // Insert low-confidence entity
    ir.insert_entity(EntityIR {
        id: EntityId("entity_draft".to_string()),
        canonical_name: "Draft Concept".to_string(),
        kind: "concept".to_string(),
        aliases: vec![],
        properties: Default::default(),
        confidence: 0.40, // Below min_confidence_threshold 0.70
        provenance: sample_provenance(),
    });

    // Insert fact with evidence
    ir.insert_fact(FactIR {
        id: FactId("fact_1".to_string()),
        subject_id: EntityId("entity_rust".to_string()),
        predicate: "type".to_string(),
        object_value: "Language".to_string(),
        confidence: 0.90,
        provenance: sample_provenance(),
    });

    // Insert fact missing evidence
    ir.insert_fact(FactIR {
        id: FactId("fact_no_evidence".to_string()),
        subject_id: EntityId("entity_rust".to_string()),
        predicate: "status".to_string(),
        object_value: "Active".to_string(),
        confidence: 0.80,
        provenance: ProvenanceIR {
            source_origin: "synthetic".to_string(),
            evidence_ids: vec![],
            confidence: 0.80,
            timestamp_ms: 1700000000000,
        },
    });

    // Insert relation edge
    ir.add_relation(RelationIR {
        source_id: EntityId("entity_rust".to_string()),
        target_id: EntityId("entity_draft".to_string()),
        relation_kind: "references".to_string(),
        weight: 0.8,
        provenance: sample_provenance(),
    });

    let (compiled_ir, report) = compiler.compile(&context, &mut ir);

    // 1. Verify entity canonical name normalization
    assert_eq!(
        compiled_ir
            .entities
            .get(&EntityId("entity_rust".to_string()))
            .unwrap()
            .canonical_name,
        "Rust Programming Language"
    );

    // 2. Verify report fields
    assert_eq!(report.passes_executed, 4);
    assert_eq!(report.entities_compiled, 2);
    assert_eq!(report.facts_compiled, 2);
    assert!(!report.diagnostics.is_empty());

    // 3. Verify deterministic diagnostics sorting
    let levels: Vec<&str> = report
        .diagnostics
        .iter()
        .map(|d| d.level.as_str())
        .collect();
    for i in 1..levels.len() {
        assert!(levels[i - 1] <= levels[i]);
    }
}

#[test]
fn test_custom_pass_manager_pipeline() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        min_confidence_threshold: 0.50,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
    };

    let pass_manager = PassManager::default_pipeline();
    let compiler = KnowledgeCompiler::with_pipeline(pass_manager);
    let mut ir = KnowledgeIR::new();

    let (_compiled_ir, report) = compiler.compile(&context, &mut ir);
    assert_eq!(report.passes_executed, 4);
    assert_eq!(report.entities_compiled, 0);
}
