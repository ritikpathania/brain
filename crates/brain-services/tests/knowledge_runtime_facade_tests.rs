//! Integration test suite for Knowledge Runtime Orchestration Façade (Phase 5 Milestone 5.3).

use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use brain_services::runtime::{
    ExecutionOptions, KnowledgeRuntime, KnowledgeRuntimeBuilder, KnowledgeRuntimeConfig, RequestId,
    RuntimeRequest,
};
use std::sync::Arc;
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "facade_test_origin".to_string(),
        evidence_ids: vec!["ev_202".to_string()],
        confidence: 0.95,
        timestamp_ms: 3000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_cargo".to_string()),
        EntityIR {
            id: EntityId("entity_cargo".to_string()),
            canonical_name: "Cargo Package Manager".to_string(),
            kind: "tool".to_string(),
            aliases: vec!["cargo".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

#[test]
fn test_knowledge_runtime_builder_and_config() {
    let config = KnowledgeRuntimeConfig {
        max_candidates_limit: 50,
        enable_telemetry: false,
    };

    let runtime = KnowledgeRuntimeBuilder::new().with_config(config).build();

    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);
    let query = KnowledgeQuery::new().with_text("Cargo");

    let req = RuntimeRequest::new(&query, &ctx);
    let res = runtime.query(req);

    assert_eq!(res.total_candidates, 1);
    assert_eq!(
        res.candidates[0].entity_id,
        EntityId("entity_cargo".to_string())
    );
}

#[test]
fn test_runtime_request_with_execution_options() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);
    let query = KnowledgeQuery::new().with_text("Cargo");

    let req_id = RequestId(Uuid::new_v4());
    let opts = ExecutionOptions {
        request_id: req_id,
        tracing_id: Some("trace_992".to_string()),
    };

    let req = RuntimeRequest::new(&query, &ctx).with_options(opts);
    assert_eq!(req.options.request_id, req_id);
    assert_eq!(req.options.tracing_id.as_deref(), Some("trace_992"));

    let runtime = KnowledgeRuntime::default();
    let resp = runtime.reason(req);

    assert_eq!(resp.primary_candidates.len(), 1);
    assert_eq!(
        resp.primary_candidates[0].entity_id,
        EntityId("entity_cargo".to_string())
    );
}

#[test]
fn test_stateless_knowledge_runtime_concurrent_safety() {
    let runtime = Arc::new(KnowledgeRuntime::default());

    let mut handles = Vec::new();

    for i in 0..5 {
        let rt = Arc::clone(&runtime);
        let handle = std::thread::spawn(move || {
            let ir = sample_ir();
            let ctx = InMemoryQueryContext::new(&ir);
            let query =
                KnowledgeQuery::new().with_text(if i % 2 == 0 { "Cargo" } else { "Missing" });

            let req = RuntimeRequest::new(&query, &ctx);
            let res = rt.query(req);

            if i % 2 == 0 {
                assert_eq!(res.total_candidates, 1);
            } else {
                assert_eq!(res.total_candidates, 0);
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }
}
