use brain_domain::SessionId;
use brain_services::compiler::{
    CompilerContext, EntityIR, EntityId, FactIR, FactId, KnowledgeCompiler, KnowledgeIR,
    ProvenanceIR, RelationIR,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn prov(origin: &str, ts: u64, conf: f64) -> ProvenanceIR {
    ProvenanceIR {
        source_origin: origin.to_string(),
        evidence_ids: vec![format!("ev_{}", ts)],
        confidence: conf,
        timestamp_ms: ts,
    }
}

#[test]
fn test_entity_merge_tie_breaking_policy_and_additive_provenance() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.50,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: brain_services::compiler::CompilerOptimizationConfig::default(),
    };

    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    // Entity 1: lower confidence (0.80), earlier timestamp (1000)
    let mut e1 = EntityIR::new(
        EntityId("entity_a".to_string()),
        "Antigravity Agent",
        "agent",
        0.80,
        prov("log_1", 1000, 0.80),
    );
    e1.aliases = vec!["AGY".to_string()];
    e1.properties
        .insert("author".to_string(), "Deepmind".to_string());
    ir.insert_entity(e1);

    // Entity 2: higher confidence (0.95), later timestamp (2000) -> Should win tie break
    let mut e2 = EntityIR::new(
        EntityId("entity_b".to_string()),
        "antigravity agent",
        "agent",
        0.95,
        prov("log_2", 2000, 0.95),
    );
    e2.properties
        .insert("version".to_string(), "2.0".to_string());
    ir.insert_entity(e2);

    let (compiled_ir, report) = compiler.compile(&context, &mut ir);

    // 1. Duplicate entity_a should be merged into winner entity_b
    assert_eq!(compiled_ir.entities.len(), 1);
    let winner = compiled_ir
        .entities
        .get(&EntityId("entity_b".to_string()))
        .unwrap();

    // 2. Properties merged additively
    assert_eq!(winner.properties.get("author").unwrap(), "Deepmind");
    assert_eq!(winner.properties.get("version").unwrap(), "2.0");

    // 3. Aliases merged additively
    assert!(winner.aliases.contains(&"AGY".to_string()));
    assert!(winner.aliases.contains(&"Antigravity Agent".to_string()));

    // 4. Additive provenance chain preserved without data loss
    assert_eq!(winner.provenance_chain.len(), 2);
    assert_eq!(winner.provenance_chain[0].source_origin, "log_2");
    assert_eq!(winner.provenance_chain[1].source_origin, "log_1");

    // 5. Diagnostics emitted
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.kind == "ambiguous_identity"));
}

#[test]
fn test_canonical_fact_selection_and_contradiction_detection() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.50,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: brain_services::compiler::CompilerOptimizationConfig::default(),
    };

    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let e1 = EntityIR::new(
        EntityId("brain_core".to_string()),
        "Brain Core",
        "component",
        0.99,
        prov("log_1", 1000, 0.99),
    );
    ir.insert_entity(e1);

    // Fact 1: older status fact (confidence 0.85, ts 1000)
    let f1 = FactIR::new(
        FactId("fact_old".to_string()),
        EntityId("brain_core".to_string()),
        "status",
        "experimental",
        0.85,
        prov("log_1", 1000, 0.85),
    );
    ir.insert_fact(f1);

    // Fact 2: newer status fact (confidence 0.98, ts 2000) -> Should be selected as canonical
    let f2 = FactIR::new(
        FactId("fact_new".to_string()),
        EntityId("brain_core".to_string()),
        "status",
        "stable",
        0.98,
        prov("log_2", 2000, 0.98),
    );
    ir.insert_fact(f2);

    let (compiled_ir, _report) = compiler.compile(&context, &mut ir);

    let winner_fact = compiled_ir
        .facts
        .get(&FactId("fact_new".to_string()))
        .unwrap();

    assert!(winner_fact.is_canonical);
    // Superseded non-canonical fact_old is pruned by DeadFactEliminationPass in KPP v1.4 retention pipeline
    assert!(!compiled_ir
        .facts
        .contains_key(&FactId("fact_old".to_string())));
}

#[test]
fn test_relation_normalization_prunes_self_loops() {
    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.50,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: brain_services::compiler::CompilerOptimizationConfig::default(),
    };

    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let e1 = EntityIR::new(
        EntityId("node_x".to_string()),
        "Node X",
        "concept",
        0.90,
        prov("log_1", 1000, 0.90),
    );
    ir.insert_entity(e1);

    // Self-loop relation
    ir.add_relation(RelationIR {
        source_id: EntityId("node_x".to_string()),
        target_id: EntityId("node_x".to_string()),
        relation_kind: "SELF_LINK".to_string(),
        weight: 1.5, // Unclamped weight
        provenance: prov("log_1", 1000, 0.90),
        provenance_chain: vec![prov("log_1", 1000, 0.90)],
    });

    let (compiled_ir, report) = compiler.compile(&context, &mut ir);

    // Self-loop should be pruned
    assert!(compiled_ir.relations.is_empty());
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.message.contains("self-referential")));
}
