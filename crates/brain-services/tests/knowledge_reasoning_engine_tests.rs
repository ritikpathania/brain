//! Integration test suite for Knowledge Reasoning & Synthesis Engine (Phase 5 Milestone 5.2).

use brain_domain::RelationId;
use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use brain_services::reasoning::{
    InferenceGraph, InferenceKind, KnowledgeReasoningEngine, Proposition,
};

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "reasoning_test_origin".to_string(),
        evidence_ids: vec!["ev_101".to_string()],
        confidence: 0.95,
        timestamp_ms: 2000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_tokio".to_string()),
        EntityIR {
            id: EntityId("entity_tokio".to_string()),
            canonical_name: "Tokio Async Runtime".to_string(),
            kind: "library".to_string(),
            aliases: vec!["Tokio-rs".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir.entities.insert(
        EntityId("entity_rust".to_string()),
        EntityIR {
            id: EntityId("entity_rust".to_string()),
            canonical_name: "Rust Systems Language".to_string(),
            kind: "language".to_string(),
            aliases: vec!["Rustlang".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

#[test]
fn test_inference_graph_ir_invariants() {
    let mut graph = InferenceGraph::new();

    let n0 = graph.add_node(
        Proposition {
            subject: EntityId("entity_tokio".to_string()),
            relation_kind: RelationId::from("written_in"),
            object: EntityId("entity_rust".to_string()),
            confidence: 0.95,
        },
        vec![],
    );

    let n1 = graph.add_node(
        Proposition {
            subject: EntityId("entity_rust".to_string()),
            relation_kind: RelationId::from("enables"),
            object: EntityId("entity_tokio".to_string()),
            confidence: 0.90,
        },
        vec![],
    );

    graph.add_edge(n0, n1, InferenceKind::Causes);

    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].source, n0);
    assert_eq!(graph.edges[0].target, n1);
    assert_eq!(graph.edges[0].kind, InferenceKind::Causes);
}

#[test]
fn test_knowledge_reasoning_engine_end_to_end() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let query = KnowledgeQuery::new().with_text("Tokio");
    let engine = KnowledgeReasoningEngine::new();

    let response1 = engine.execute(&query, &ctx);
    let response2 = engine.execute(&query, &ctx);

    // Assert deterministic reproducibility invariant: Identical inputs produce identical responses
    assert_eq!(response1.answer_summary, response2.answer_summary);
    assert_eq!(
        response1.reasoning_trace.len(),
        response2.reasoning_trace.len()
    );
    assert_eq!(response1.confidence, response2.confidence);
    assert_eq!(response1.primary_candidates, response2.primary_candidates);

    assert!(!response1.primary_candidates.is_empty());
    assert_eq!(
        response1.primary_candidates[0].entity_id,
        EntityId("entity_tokio".to_string())
    );
    assert!(response1.confidence.composite_confidence > 0.0);
}
