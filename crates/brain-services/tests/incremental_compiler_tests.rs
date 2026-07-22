use brain_domain::SessionId;
use brain_services::compiler::{
    CompilerContext, DirtySet, EntityIR, EntityId, FactIR, FactId, KnowledgeCompiler, KnowledgeIR,
    ProvenanceIR, RelationIR,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn sample_prov(origin: &str, ts: u64) -> ProvenanceIR {
    ProvenanceIR {
        source_origin: origin.to_string(),
        evidence_ids: vec![format!("ev_{}", ts)],
        confidence: 0.90,
        timestamp_ms: ts,
    }
}

fn build_sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    let mut e1 = EntityIR::new(
        EntityId("entity_rust".to_string()),
        "  Rust Language  ",
        "concept",
        0.95,
        sample_prov("log_1", 1000),
    );
    e1.aliases = vec!["Rust".to_string()];
    ir.insert_entity(e1);

    let e2 = EntityIR::new(
        EntityId("entity_agent".to_string()),
        "Antigravity Agent",
        "agent",
        0.90,
        sample_prov("log_2", 2000),
    );
    ir.insert_entity(e2);

    let f1 = FactIR::new(
        FactId("fact_1".to_string()),
        EntityId("entity_rust".to_string()),
        "type",
        "Language",
        0.90,
        sample_prov("log_1", 1000),
    );
    ir.insert_fact(f1);

    let f2 = FactIR::new(
        FactId("fact_2".to_string()),
        EntityId("entity_agent".to_string()),
        "written_in",
        "Rust",
        0.92,
        sample_prov("log_2", 2000),
    );
    ir.insert_fact(f2);

    ir.add_relation(RelationIR {
        source_id: EntityId("entity_agent".to_string()),
        target_id: EntityId("entity_rust".to_string()),
        relation_kind: "uses".to_string(),
        weight: 0.95,
        provenance: sample_prov("log_2", 2000),
        provenance_chain: vec![sample_prov("log_2", 2000)],
    });

    ir
}

#[test]
fn test_incremental_vs_full_compilation_equivalence() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.50,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
    };

    let compiler = KnowledgeCompiler::new();

    // 1. Full compilation baseline
    let mut full_ir = build_sample_ir();
    let (compiled_full_ir, full_report) = compiler.compile(&context, &mut full_ir);

    // 2. Incremental compilation with entity_agent marked dirty
    let mut inc_ir = build_sample_ir();
    let mut dirty_set = DirtySet::new(1);
    dirty_set.mark_entity(EntityId("entity_agent".to_string()));

    let (compiled_inc_ir, inc_report) =
        compiler.compile_incremental(&context, &mut inc_ir, dirty_set);

    // 3. Primary Correctness Guarantee: Full Compilation == Incremental Compilation
    assert_eq!(compiled_full_ir, compiled_inc_ir);
    assert_eq!(full_report.entities_compiled, inc_report.entities_compiled);
    assert_eq!(full_report.facts_compiled, inc_report.facts_compiled);
}

#[test]
fn test_graph_version_mismatch_forces_full_recompile() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 2, // Engine is at version 2
        dirty_set: None,
        min_confidence_threshold: 0.50,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
    };

    let compiler = KnowledgeCompiler::new();
    let mut ir = build_sample_ir();

    // Stale dirty set with graph_version = 1
    let mut dirty_set = DirtySet::new(1);
    dirty_set.mark_entity(EntityId("entity_rust".to_string()));

    let (_compiled_ir, report) = compiler.compile_incremental(&context, &mut ir, dirty_set);

    // Should complete successfully via fallback to full compilation
    assert_eq!(report.passes_executed, 12);
}
