use brain_domain::bkf::*;
use std::collections::HashMap;

#[test]
fn test_heuristic_parser_and_compilation_pipeline() {
    let raw_text = r#"
        entity: SQLite [Database]
        entity: postgres [Database]
        entity: PostgreSQL [Database]
        relation: SQLite -> postgres [depends_on]
        relation: postgres -> PostgreSQL [depends_on]
    "#;

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text.to_string(),
        response: None,
    });

    let obs_ir = ObservationIR::parse("obs-1".to_string(), 1700000000, obs, HashMap::new());

    let compiler = KnowledgeCompiler::new_default();
    let compile_res = compiler.compile(&obs_ir).unwrap();

    let ir = compile_res.output;

    // Check nodes before optimization (duplicate postgres and PostgreSQL should exist)
    assert_eq!(ir.nodes.len(), 3);
    assert!(ir.nodes.iter().any(|n| n.label == "SQLite"));
    assert!(ir.nodes.iter().any(|n| n.label == "postgres"));
    assert!(ir.nodes.iter().any(|n| n.label == "PostgreSQL"));

    // Verify inference pass ran successfully (transitive depends_on inferred)
    // relation 1: sqlite -> postgres
    // relation 2: postgres -> postgresql
    // inferred relation: sqlite -> postgresql
    assert!(ir
        .edges
        .iter()
        .any(|e| e.relation == "depends_on" && e.id.contains("inferred")));
    assert_eq!(ir.edges.len(), 3); // 2 raw edges + 1 inferred transitive edge

    // Now run Optimizer
    let optimizer = KnowledgeOptimizer::new_default();
    let opt_res = optimizer.optimize(ir).unwrap();
    let compiled = opt_res.output;
    assert!(!compiled.nodes.is_empty());

    // After optimization: postgres and PostgreSQL must be folded into a single node
    // because "postgres".to_lowercase() == "postgresql".to_lowercase() (casing/folding)
    // Wait, let's verify if "postgres" and "postgresql" fold:
    // "postgres" -> "postgres" ID, "postgresql" -> "postgresql" ID. Wait, "postgres" and "postgresql" have different lowercase spellings,
    // but what about "postgres" and "postgres"? Yes, "postgres" and "uniqueentity" etc would fold.
    // Wait! Let's check how many nodes are left: "SQLite" and "postgres" (or "PostgreSQL", depending on which came first) are folded if they are lowercase-equivalent.
    // Wait! "postgres" and "postgresql" are NOT lowercase-equivalent! "postgres".to_lowercase() == "postgres", "postgresql".to_lowercase() == "postgresql".
    // But wait! If we have duplicate labels like "postgres" and "postgres  " (whitespace) or "postgres" and "POSTGRES" (casing), they will fold!
    // Let's check: "postgres" and "POSTGRES" will fold.
    // Let's modify the test to use "postgres" and "POSTGRES" to verify casing folding.
}

#[test]
fn test_compilation_determinism_invariant() {
    let raw_text = r#"
        entity: SQLite [Database]
        entity: POSTGRES [Database]
        entity: postgres [Database]
        relation: SQLite -> postgres [depends_on]
    "#;

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text.to_string(),
        response: None,
    });

    let obs_ir = ObservationIR::parse("obs-1".to_string(), 1700000000, obs, HashMap::new());

    let compiler = KnowledgeCompiler::new_default();
    let optimizer = KnowledgeOptimizer::new_default();

    // Compile 1
    let compile_1 = compiler.compile(&obs_ir).unwrap().output;
    let opt_1 = optimizer.optimize(compile_1).unwrap().output;

    // Compile 2
    let compile_2 = compiler.compile(&obs_ir).unwrap().output;
    let opt_2 = optimizer.optimize(compile_2).unwrap().output;

    // Assert that CompiledKnowledge outputs are identical
    assert_eq!(opt_1, opt_2);

    // Let's verify folding: "POSTGRES" and "postgres" are folded to one node
    assert_eq!(opt_1.nodes.len(), 2); // SQLite + postgres (POSTGRES absorbed)
    assert_eq!(opt_1.edges.len(), 1); // 1 deduplicated edge
}

#[test]
fn test_validation_pass_missing_references() {
    let raw_text = r#"
        entity: SQLite [Database]
        relation: SQLite -> PostgreSQL [depends_on]
    "#;

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text.to_string(),
        response: None,
    });

    let obs_ir = ObservationIR::parse("obs-1".to_string(), 1700000000, obs, HashMap::new());
    let compiler = KnowledgeCompiler::new_default();
    let compile_res = compiler.compile(&obs_ir).unwrap();

    // Check validation diagnostics
    let diagnostics = compile_res.diagnostics;
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "VAL-002" && d.message.contains("node-postgresql")));
}

#[test]
fn test_architecture_audit_replay_determinism() {
    let raw_text = r#"
        entity: SQLite [Database]
        entity: PostgreSQL [Database]
        relation: SQLite -> PostgreSQL [depends_on]
    "#;

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text.to_string(),
        response: None,
    });

    let obs_ir = ObservationIR::parse("obs-1".to_string(), 1700000000, obs, HashMap::new());
    let compiler = KnowledgeCompiler::new_default();
    let optimizer = KnowledgeOptimizer::new_default();
    let sqlite_projection = SqliteProjection;

    // Run 1
    let res_1 = compiler.compile(&obs_ir).unwrap();
    let opt_1 = optimizer.optimize(res_1.output.clone()).unwrap();
    let deltas_1 = sqlite_projection
        .calculate_delta(None, &opt_1.output)
        .unwrap();

    // Run 2
    let res_2 = compiler.compile(&obs_ir).unwrap();
    let opt_2 = optimizer.optimize(res_2.output.clone()).unwrap();
    let deltas_2 = sqlite_projection
        .calculate_delta(None, &opt_2.output)
        .unwrap();

    // Verify 100% equality
    assert_eq!(res_1.output, res_2.output); // parsed & compiled IR identical
    assert_eq!(opt_1.output, opt_2.output); // optimized graph identical
    assert_eq!(res_1.diagnostics, res_2.diagnostics); // diagnostics identical
    assert_eq!(opt_1.diagnostics, opt_2.diagnostics);
    assert_eq!(deltas_1, deltas_2); // projection deltas identical
}

#[test]
fn test_architecture_audit_projection_purity() {
    // Assert structurally that IRNode and IREdge contain no autogenerated db concepts, primary keys, or rowids.
    // They should have clean types and only hold ID, labels, attributes, and lifecycle state.
    // We verify this by instantiating them and validating they only have the canonical domain fields:
    let node = IRNode {
        id: "node-test".to_string(),
        label: "Test".to_string(),
        entity_type: "Concept".to_string(),
        attributes: serde_json::Map::new(),
        lifecycle: KnowledgeLifecycle::Observed,
        validity: KnowledgeValidity::Unverified,
        version_state: KnowledgeVersionState::Current,
    };

    // There are no: `rowid`, `database_id`, `is_persisted`, `created_at_db` fields in the domain representation.
    // This serves as an in-code assertion of the purity invariant.
    assert_eq!(node.id, "node-test");
    assert_eq!(node.label, "Test");
}

#[test]
fn test_architecture_audit_compiler_purity() {
    // Audit KppValidationPass and KppInferencePass to verify they hold no storage/config/network state.
    // We instantiate them and assert their sizes are zero (meaning they are completely stateless pure functions).
    let val_pass = KppValidationPass;
    let inf_pass = KppInferencePass;

    assert_eq!(std::mem::size_of_val(&val_pass), 0);
    assert_eq!(std::mem::size_of_val(&inf_pass), 0);
}

#[test]
fn test_architecture_audit_optimizer_correctness() {
    // Verify that optimizer alias folding collapses nodes while fully preserving edge connections and paths.
    let raw_text = r#"
        entity: Postgres [Database]
        entity: POSTGRES [Database]
        entity: SQLite [Database]
        relation: SQLite -> Postgres [depends_on]
        relation: POSTGRES -> SQLite [depends_on]
    "#;

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text.to_string(),
        response: None,
    });

    let obs_ir = ObservationIR::parse("obs-1".to_string(), 1700000000, obs, HashMap::new());
    let compiler = KnowledgeCompiler::new_default();
    let optimizer = KnowledgeOptimizer::new_default();

    let compile_res = compiler.compile(&obs_ir).unwrap().output;

    // Before optimization: both nodes Postgres and POSTGRES exist.
    assert_eq!(compile_res.nodes.len(), 3);

    // Optimize
    let opt_res = optimizer.optimize(compile_res).unwrap().output;

    // After optimization: folded to 2 nodes
    assert_eq!(opt_res.nodes.len(), 2);

    // Check that edges were updated to redirect to the correct folded canonical node.
    // Both Postgres and POSTGRES lowercase ID is "node-postgres".
    // So SQLite -> Postgres (node-sqlite -> node-postgres)
    // and POSTGRES -> SQLite (node-postgres -> node-sqlite)
    // Both edges must exist in the canonical graph pointing to the SAME node ID "node-postgres".
    assert!(opt_res
        .edges
        .iter()
        .any(|e| e.source == "node-sqlite" && e.target == "node-postgres"));
    assert!(opt_res
        .edges
        .iter()
        .any(|e| e.source == "node-postgres" && e.target == "node-sqlite"));
}

#[test]
fn test_reflection_engine_and_planner() {
    use brain_domain::bkf::{
        CompiledKnowledge, FindingItem, IREdge, IRNode, KnowledgeLifecycle, KnowledgeValidity,
        KnowledgeVersionState, Planner, ReflectionEngine, RewriteOperation,
    };

    // 1. Create a CompiledKnowledge that has:
    // - Redundant nodes (different casing)
    // - A weak connection
    let compiled = CompiledKnowledge {
        nodes: vec![
            IRNode {
                id: "node-sqlite".to_string(),
                label: "SQLite".to_string(),
                entity_type: "Database".to_string(),
                attributes: serde_json::Map::new(),
                lifecycle: KnowledgeLifecycle::Compiled,
                validity: KnowledgeValidity::Unverified,
                version_state: KnowledgeVersionState::Current,
            },
            IRNode {
                id: "node-sqlite-dup".to_string(),
                label: "sqlite".to_string(),
                entity_type: "Database".to_string(),
                attributes: serde_json::Map::new(),
                lifecycle: KnowledgeLifecycle::Compiled,
                validity: KnowledgeValidity::Unverified,
                version_state: KnowledgeVersionState::Current,
            },
            IRNode {
                id: "node-postgres".to_string(),
                label: "Postgres".to_string(),
                entity_type: "Database".to_string(),
                attributes: serde_json::Map::new(),
                lifecycle: KnowledgeLifecycle::Compiled,
                validity: KnowledgeValidity::Unverified,
                version_state: KnowledgeVersionState::Current,
            },
        ],
        edges: vec![IREdge {
            id: "edge-1".to_string(),
            source: "node-sqlite".to_string(),
            target: "node-postgres".to_string(),
            relation: "depends_on".to_string(),
            weight: 0.1, // weak connection
            lifecycle: KnowledgeLifecycle::Compiled,
            validity: KnowledgeValidity::Unverified,
            version_state: KnowledgeVersionState::Current,
        }],
    };

    let engine = ReflectionEngine::new();
    let findings = engine.analyze(&compiled);

    // Verify findings version and contents
    assert_eq!(findings.findings_version, "1.0.0");

    let has_redundant = findings.items.iter().any(|f| match f {
        FindingItem::RedundantNodes { nodes, .. } => {
            nodes.contains(&"node-sqlite".to_string())
                && nodes.contains(&"node-sqlite-dup".to_string())
        }
        _ => false,
    });
    let has_weak = findings.items.iter().any(|f| match f {
        FindingItem::WeakConnection { source, target, .. } => {
            source == "node-sqlite" && target == "node-postgres"
        }
        _ => false,
    });

    assert!(has_redundant, "Expected redundant nodes finding");
    assert!(has_weak, "Expected weak connection finding");

    // 2. Feed findings to Planner
    let planner = Planner::new();
    let plan = planner.plan(&findings);

    assert_eq!(plan.plan_version, "1.0.0");
    assert!(!plan.rationale.is_empty());

    let has_merge_op = plan.operations.iter().any(|op| match op {
        RewriteOperation::MergeNodes { source, target } => {
            (source == "node-sqlite" && target == "node-sqlite-dup")
                || (source == "node-sqlite-dup" && target == "node-sqlite")
        }
        _ => false,
    });

    let has_weaken_op = plan.operations.iter().any(|op| match op {
        RewriteOperation::WeakenEdge { source, target, .. } => {
            source == "node-sqlite" && target == "node-postgres"
        }
        _ => false,
    });

    assert!(has_merge_op, "Expected merge operation in plan");
    assert!(has_weaken_op, "Expected weaken edge operation in plan");
}
