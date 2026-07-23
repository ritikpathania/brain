use brain_domain::SessionId;
use brain_services::compiler::{
    CompilationMode, CompilerContext, EntityIR, EntityId, FactIR, FactId, KnowledgeCompiler,
    KnowledgeIR, PassId, ProvenanceIR,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn sample_provenance() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "test_harness".to_string(),
        evidence_ids: vec!["ev_001".to_string()],
        confidence: 0.90,
        timestamp_ms: 1700000000000,
    }
}

#[test]
fn test_side_effect_free_observability_snapshot() {
    let compiler = KnowledgeCompiler::new();
    let initial_snap = compiler.runtime_state().live_snapshot();

    assert_eq!(initial_snap.graph_version, 1);
    assert_eq!(initial_snap.total_compilations, 0);

    // Call live_snapshot multiple times
    let snap2 = compiler.runtime_state().live_snapshot();
    let snap3 = compiler.runtime_state().live_snapshot();

    // Verify snapshots are strictly identical and read-only (zero side-effects)
    assert_eq!(initial_snap, snap2);
    assert_eq!(snap2, snap3);
    assert_eq!(compiler.runtime_state().graph_version(), 1);
}

#[test]
fn test_pass_execution_records_and_telemetry() {
    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let entity = EntityIR::new(
        EntityId("entity_rust".to_string()),
        "  Rust Language  ",
        "concept",
        0.95,
        sample_provenance(),
    );
    ir.insert_entity(entity);

    let fact = FactIR::new(
        FactId("fact_1".to_string()),
        EntityId("entity_rust".to_string()),
        "type",
        "Programming Language",
        0.90,
        sample_provenance(),
    );
    ir.insert_fact(fact);

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: brain_services::compiler::CompilerOptimizationConfig::default(),
    };

    let (_compiled_ir, report) = compiler.compile(&context, &mut ir);

    let snap = compiler.runtime_state().live_snapshot();

    // Verify counters updated
    assert_eq!(snap.total_compilations, 1);
    assert_eq!(snap.full_compilations, 1);
    assert_eq!(snap.incremental_compilations, 0);
    assert_eq!(snap.last_compilation_mode, Some(CompilationMode::Full));
    assert_eq!(report.passes_executed, 18);

    // Verify per-pass timing metrics captured
    assert!(!snap.pass_metrics.is_empty());
    let alias_pass = snap
        .pass_metrics
        .iter()
        .find(|pm| pm.pass_name == PassId::AliasResolution.as_str());
    assert!(alias_pass.is_some());
    assert_eq!(alias_pass.unwrap().executions, 1);
}

#[test]
fn test_history_ring_buffer_capacity() {
    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: brain_services::compiler::CompilerOptimizationConfig::default(),
    };

    // Run 25 compilations to exceed capacity of 20
    for _ in 0..25 {
        compiler.compile(&context, &mut ir);
    }

    let history = compiler.runtime_state().compilation_history();
    assert_eq!(history.len(), 20);

    let snap = compiler.runtime_state().live_snapshot();
    assert_eq!(snap.total_compilations, 25);
}
