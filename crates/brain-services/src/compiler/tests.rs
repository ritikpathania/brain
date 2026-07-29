//! Categorized Test Suite for the Knowledge Compiler.
//!
//! Organizes tests into three explicit architectural categories:
//! 1. Contract Tests (Public API, payload metadata, delta purity)
//! 2. Pipeline Tests (End-to-end MutationRequest::Observe execution)
//! 3. Determinism Tests (Bit-wise identical delta & result outputs given deterministic fixtures)

#[cfg(test)]
mod contract_tests {
    use crate::compiler::delta::GraphDelta;
    use crate::compiler::mutation::{MutationKind, MutationRequest, ObservationPayload};
    use crate::compiler::result::CompilerResult;
    use brain_domain::SessionId;

    #[test]
    fn test_mutation_request_metadata_contract() {
        let session_id = SessionId::new();
        let payload = ObservationPayload {
            source_origin: "contract_test".to_string(),
            content: "Sample observation".to_string(),
        };

        let req = MutationRequest::new(session_id, MutationKind::Observe(payload));
        assert_eq!(req.session_id, session_id);
        assert!(req.timestamp_ms > 0);
    }

    #[test]
    fn test_graph_delta_purity_contract() {
        let delta = GraphDelta::empty();
        assert!(delta.is_empty());
        assert!(delta.added_nodes.is_empty());
        assert!(delta.removed_nodes.is_empty());
    }

    #[test]
    fn test_compiler_result_empty_contract() {
        let res = CompilerResult::empty();
        assert!(res.is_empty());
        assert!(res.graph_delta.is_empty());
        assert!(res.events.is_empty());
    }

    #[test]
    fn test_compiler_state_and_trace_isolation() {
        use crate::compiler::ir::KnowledgeIR;
        use crate::compiler::plan::CompilerExecutionPlan;
        use crate::compiler::state::CompilerState;
        use crate::compiler::telemetry::PassId;

        let session_id = SessionId::new();
        let ctx = crate::compiler::context::CompilerContext::for_session(session_id);
        let ir = KnowledgeIR::new();

        let mut state = CompilerState::new(ctx, ir);
        let plan = CompilerExecutionPlan::standard_3tier_pipeline();
        plan.execute(&mut state);

        assert_eq!(state.trace.structural_records.len(), plan.passes().len());
        assert_eq!(
            state.trace.structural_records[0].pass_id,
            PassId::ObservationNormalization
        );
    }
}

#[cfg(test)]
mod pipeline_tests {
    use crate::compiler::context::CompilerContext;
    use crate::compiler::mutation::{MutationKind, MutationRequest, ObservationPayload};
    use crate::compiler::repository::{InMemoryKnowledgeRepository, KnowledgeRepository};
    use crate::compiler::KnowledgeCompiler;
    use brain_domain::{DomainEvent, SessionId};
    use std::sync::Arc;

    #[test]
    fn test_pipeline_observe_vertical_slice() {
        let compiler = KnowledgeCompiler::new();
        let repo = Arc::new(InMemoryKnowledgeRepository::new());
        let session_id = SessionId::new();
        let ctx = CompilerContext::for_session(session_id);

        let payload = ObservationPayload {
            source_origin: "pipeline_test".to_string(),
            content: "Discovered component SearchProjector in brain-services".to_string(),
        };
        let req = MutationRequest::new(session_id, MutationKind::Observe(payload));

        let res = compiler.compile_request_with_repository(&ctx, req, repo.as_ref());
        assert!(
            res.is_ok(),
            "compile_request_with_repository failed: {:?}",
            res
        );

        let result = res.unwrap();
        assert!(
            !result.is_empty(),
            "CompilerResult should not be empty for valid observation"
        );
        assert!(
            !result.graph_delta.is_empty(),
            "GraphDelta should contain compiled nodes"
        );
        assert_eq!(result.graph_delta.added_nodes.len(), 1);

        let compiled_node = &result.graph_delta.added_nodes[0];
        assert_eq!(compiled_node.label, "SearchProjector");

        // Verify repository received and applied delta
        let stored_nodes = repo.get_nodes().unwrap();
        assert_eq!(stored_nodes.len(), 1);
        assert_eq!(stored_nodes[0].label, "SearchProjector");

        // Verify emitted domain events
        assert!(result
            .events
            .iter()
            .any(|e| matches!(e, DomainEvent::MemoryCreated { .. })));
    }
}

#[cfg(test)]
mod determinism_tests {
    use crate::compiler::context::CompilerContext;
    use crate::compiler::mutation::{
        MutationId, MutationKind, MutationRequest, ObservationPayload,
    };
    use crate::compiler::repository::{InMemoryKnowledgeRepository, KnowledgeRepository};
    use crate::compiler::KnowledgeCompiler;
    use brain_domain::SessionId;
    use tokio_util::sync::CancellationToken;
    use ulid::Ulid;
    use uuid::Uuid;

    #[test]
    fn test_deterministic_compilation_output() {
        let compiler = KnowledgeCompiler::new();

        // Fixed deterministic fixture context & session
        let fixed_session_ulid = Ulid::from_string("01H7X1N0000000000000000000").unwrap();
        let session_id = SessionId(fixed_session_ulid);

        let fixed_compilation_uuid = Uuid::nil();
        let ctx = CompilerContext {
            compilation_id: fixed_compilation_uuid,
            session_id,
            graph_version: 1,
            dirty_set: None,
            min_confidence_threshold: 0.5,
            time_budget_ms: 5000,
            cancellation_token: CancellationToken::new(),
            config: Default::default(),
        };

        let fixed_mutation_ulid = Ulid::from_string("01H7X1N0000000000000000001").unwrap();
        let req1 = MutationRequest {
            id: MutationId(fixed_mutation_ulid),
            timestamp_ms: 1700000000000,
            session_id,
            kind: MutationKind::Observe(ObservationPayload {
                source_origin: "determinism_test".to_string(),
                content: "Deterministic Node Alpha".to_string(),
            }),
        };

        let req2 = req1.clone();

        let repo1 = InMemoryKnowledgeRepository::new();
        let repo2 = InMemoryKnowledgeRepository::new();

        let res1 = compiler
            .compile_request_with_repository(&ctx, req1, &repo1)
            .unwrap();
        let res2 = compiler
            .compile_request_with_repository(&ctx, req2, &repo2)
            .unwrap();

        // Assert exact equality of compiled GraphDeltas and output results
        assert_eq!(res1.graph_delta, res2.graph_delta);
        assert_eq!(res1.events, res2.events);
        assert_eq!(res1.diagnostics, res2.diagnostics);
        assert_eq!(repo1.get_nodes().unwrap(), repo2.get_nodes().unwrap());
    }
}

#[cfg(test)]
mod projection_tests {
    use crate::compiler::delta::GraphDelta;
    use crate::compiler::projections::{GraphProjector, ProjectionEngine, SearchProjector};
    use brain_domain::dtos::NodeDTO;

    #[test]
    fn test_projection_engine_materialization() {
        let mut engine = ProjectionEngine::new();
        let search_proj = SearchProjector::new();
        let graph_proj = GraphProjector::new();

        engine.register(Box::new(search_proj));
        engine.register(Box::new(graph_proj));

        let delta = GraphDelta {
            added_nodes: vec![NodeDTO::new(
                "n1".to_string(),
                "Node 1".to_string(),
                "concept".to_string(),
                serde_json::json!({}),
            )],
            updated_nodes: Vec::new(),
            removed_nodes: Vec::new(),
            added_edges: Vec::new(),
            updated_edges: Vec::new(),
            removed_edges: Vec::new(),
        };

        let results = engine.apply_delta_all(&delta);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}

#[cfg(test)]
mod reflection_tests {
    use crate::compiler::graph::CanonicalGraph;
    use crate::compiler::ir::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
    use crate::compiler::reflection::ReflectionEngine;
    use brain_domain::SessionId;

    #[test]
    fn test_idempotent_reflection_engine() {
        let mut engine = ReflectionEngine::new();
        let session_id = SessionId::new();

        let mut ir = KnowledgeIR::new();
        let provenance = ProvenanceIR {
            source_origin: "test".to_string(),
            evidence_ids: vec!["ev1".to_string()],
            confidence: 1.0,
            timestamp_ms: 1000,
        };
        ir.insert_entity(EntityIR::new(
            EntityId("e1".to_string()),
            "Entity 1",
            "concept",
            1.0,
            provenance,
        ));
        let graph = CanonicalGraph::from_ir(&ir);

        // First analysis produces synthesis request
        let reqs1 = engine.analyze_graph(session_id, &graph);
        assert_eq!(reqs1.len(), 1);

        // Repeated analysis over identical graph produces NO duplicate requests (idempotency)
        let reqs2 = engine.analyze_graph(session_id, &graph);
        assert_eq!(reqs2.len(), 0);
    }
}
