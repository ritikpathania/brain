use brain_domain::SessionId;
use brain_services::compiler::{
    CompilerContext, CompilerOptimizationConfig, EntityIR, EntityId, FactIR, FactId,
    KnowledgeCompiler, KnowledgeIR, ProvenanceIR, RelationIR,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn sample_provenance(origin: &str, ts: u64, conf: f64) -> ProvenanceIR {
    ProvenanceIR {
        source_origin: origin.to_string(),
        evidence_ids: vec![format!("ev_{}", ts)],
        confidence: conf,
        timestamp_ms: ts,
    }
}

#[test]
fn test_optimization_pass_idempotence_invariant() {
    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let e1 = EntityIR::new(
        EntityId("entity_a".to_string()),
        "Concept Alpha",
        "concept",
        0.90,
        sample_provenance("obs1", 1000, 0.90),
    );
    let e2 = EntityIR::new(
        EntityId("entity_b".to_string()),
        "Concept Beta",
        "concept",
        0.85,
        sample_provenance("obs2", 2000, 0.85),
    );
    ir.insert_entity(e1);
    ir.insert_entity(e2);

    let f1 = FactIR::new(
        FactId("fact_1".to_string()),
        EntityId("entity_a".to_string()),
        "status",
        "active",
        0.95,
        sample_provenance("obs1", 1000, 0.95),
    );
    ir.insert_fact(f1);

    ir.add_relation(RelationIR {
        source_id: EntityId("entity_a".to_string()),
        target_id: EntityId("entity_b".to_string()),
        relation_kind: "connects_to".to_string(),
        weight: 0.8,
        provenance: sample_provenance("obs1", 1000, 0.8),
        provenance_chain: vec![sample_provenance("obs1", 1000, 0.8)],
    });

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: CompilerOptimizationConfig::default(),
    };

    let (compiled_once, _report1) = compiler.compile(&context, &mut ir.clone());
    let (compiled_twice, _report2) = compiler.compile(&context, &mut compiled_once.clone());

    // Assert Idempotence: Optimize(Optimize(IR)) == Optimize(IR)
    assert_eq!(compiled_once, compiled_twice);
}

#[test]
fn test_relation_deduplication_pass() {
    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let e1 = EntityIR::new(
        EntityId("entity_a".to_string()),
        "Alpha",
        "concept",
        0.90,
        sample_provenance("obs1", 1000, 0.90),
    );
    let e2 = EntityIR::new(
        EntityId("entity_b".to_string()),
        "Beta",
        "concept",
        0.90,
        sample_provenance("obs2", 1000, 0.90),
    );
    ir.insert_entity(e1);
    ir.insert_entity(e2);

    // Parallel edges between entity_a and entity_b with identical relation_kind
    ir.add_relation(RelationIR {
        source_id: EntityId("entity_a".to_string()),
        target_id: EntityId("entity_b".to_string()),
        relation_kind: "depends_on".to_string(),
        weight: 0.60,
        provenance: sample_provenance("obs1", 1000, 0.60),
        provenance_chain: vec![sample_provenance("obs1", 1000, 0.60)],
    });
    ir.add_relation(RelationIR {
        source_id: EntityId("entity_a".to_string()),
        target_id: EntityId("entity_b".to_string()),
        relation_kind: "depends_on".to_string(),
        weight: 0.95,
        provenance: sample_provenance("obs2", 2000, 0.95),
        provenance_chain: vec![sample_provenance("obs2", 2000, 0.95)],
    });

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: CompilerOptimizationConfig::default(),
    };

    let (compiled_ir, _report) = compiler.compile(&context, &mut ir);

    // Assert relation edges deduplicated to 1 edge with max weight 0.95
    assert_eq!(compiled_ir.relations.len(), 1);
    assert_eq!(compiled_ir.relations[0].weight, 0.95);
    assert_eq!(compiled_ir.relations[0].provenance_chain.len(), 2);
}

#[test]
fn test_transitive_reduction_pass() {
    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let e_a = EntityIR::new(
        EntityId("a".to_string()),
        "Node A",
        "concept",
        0.9,
        sample_provenance("obs", 1000, 0.9),
    );
    let e_b = EntityIR::new(
        EntityId("b".to_string()),
        "Node B",
        "concept",
        0.9,
        sample_provenance("obs", 1000, 0.9),
    );
    let e_c = EntityIR::new(
        EntityId("c".to_string()),
        "Node C",
        "concept",
        0.9,
        sample_provenance("obs", 1000, 0.9),
    );
    ir.insert_entity(e_a);
    ir.insert_entity(e_b);
    ir.insert_entity(e_c);

    // Hierarchy edges: A -> B, B -> C, and redundant direct edge A -> C
    ir.add_relation(RelationIR {
        source_id: EntityId("a".to_string()),
        target_id: EntityId("b".to_string()),
        relation_kind: "parent_of".to_string(),
        weight: 0.9,
        provenance: sample_provenance("obs", 1000, 0.9),
        provenance_chain: vec![sample_provenance("obs", 1000, 0.9)],
    });
    ir.add_relation(RelationIR {
        source_id: EntityId("b".to_string()),
        target_id: EntityId("c".to_string()),
        relation_kind: "parent_of".to_string(),
        weight: 0.9,
        provenance: sample_provenance("obs", 1000, 0.9),
        provenance_chain: vec![sample_provenance("obs", 1000, 0.9)],
    });
    ir.add_relation(RelationIR {
        source_id: EntityId("a".to_string()),
        target_id: EntityId("c".to_string()),
        relation_kind: "parent_of".to_string(),
        weight: 0.9,
        provenance: sample_provenance("obs", 1000, 0.9),
        provenance_chain: vec![sample_provenance("obs", 1000, 0.9)],
    });

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: CompilerOptimizationConfig::default(),
    };

    let (compiled_ir, _report) = compiler.compile(&context, &mut ir);

    // Assert direct redundant edge A -> C was pruned by transitive reduction
    assert_eq!(compiled_ir.relations.len(), 2);
    assert!(compiled_ir
        .relations
        .iter()
        .all(|r| !(r.source_id == EntityId("a".to_string())
            && r.target_id == EntityId("c".to_string()))));
}

#[test]
fn test_dead_fact_elimination_and_confidence_pruning() {
    let compiler = KnowledgeCompiler::new();
    let mut ir = KnowledgeIR::new();

    let e = EntityIR::new(
        EntityId("concept_x".to_string()),
        "Concept X",
        "concept",
        0.9,
        sample_provenance("obs", 1000, 0.9),
    );
    ir.insert_entity(e);

    // Active canonical fact
    let f_active = FactIR::new(
        FactId("fact_active".to_string()),
        EntityId("concept_x".to_string()),
        "color",
        "blue",
        0.90,
        sample_provenance("obs", 1000, 0.90),
    );
    ir.insert_fact(f_active);

    // Low confidence fact (< 0.10)
    let f_low_conf = FactIR::new(
        FactId("fact_low".to_string()),
        EntityId("concept_x".to_string()),
        "temp",
        "cold",
        0.05,
        sample_provenance("obs", 1000, 0.05),
    );
    ir.insert_fact(f_low_conf);

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 5000,
        cancellation_token: CancellationToken::new(),
        config: CompilerOptimizationConfig::default(),
    };

    let (compiled_ir, _report) = compiler.compile(&context, &mut ir);

    // Assert low-confidence fact pruned and active canonical fact preserved
    assert_eq!(compiled_ir.facts.len(), 1);
    assert!(compiled_ir
        .facts
        .contains_key(&FactId("fact_active".to_string())));
    assert!(!compiled_ir
        .facts
        .contains_key(&FactId("fact_low".to_string())));
}
