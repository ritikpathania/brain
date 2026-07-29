//! Integration test suite for Knowledge Query Engine (Phase 5 Milestone 5.1).

use brain_domain::RelationId;
use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::query::{
    ExecutionStep, ExecutionStepId, FusionStrategy, InMemoryQueryContext, KnowledgeQuery,
    NoOpOptimizer, PlanOptimizer, QueryExecutor, QueryPipeline, QueryPlanner, ReciprocalRankFusion,
    TemporalRange,
};

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "test_origin".to_string(),
        evidence_ids: vec!["ev_1".to_string()],
        confidence: 0.9,
        timestamp_ms: 1000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_rust".to_string()),
        EntityIR {
            id: EntityId("entity_rust".to_string()),
            canonical_name: "Rust Systems Programming".to_string(),
            kind: "concept".to_string(),
            aliases: vec!["Rustlang".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.9,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir.entities.insert(
        EntityId("entity_python".to_string()),
        EntityIR {
            id: EntityId("entity_python".to_string()),
            canonical_name: "Python Dynamic Language".to_string(),
            kind: "concept".to_string(),
            aliases: vec!["Py".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.9,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

#[test]
fn test_query_ast_builder() {
    let query = KnowledgeQuery::new()
        .with_text("Rust")
        .with_semantic_prompt("systems language")
        .with_relation_filter(
            RelationId::from("depends_on"),
            EntityId("entity_cargo".to_string()),
        )
        .with_temporal_range(TemporalRange {
            start_ms: Some(500),
            end_ms: Some(1500),
        })
        .with_limit(10);

    assert_eq!(query.text.as_deref(), Some("Rust"));
    assert_eq!(query.semantic_prompt.as_deref(), Some("systems language"));
    assert_eq!(query.relation_filters.len(), 1);
    assert_eq!(
        query.relation_filters[0].relation_kind,
        RelationId::from("depends_on")
    );
    assert_eq!(
        query.relation_filters[0].target_id,
        EntityId("entity_cargo".to_string())
    );
    assert_eq!(query.limit, 10);
}

#[test]
fn test_query_planner_and_execution_plan() {
    let query = KnowledgeQuery::new()
        .with_text("Rust")
        .with_semantic_prompt("systems language");

    let planner = QueryPlanner::new();
    let plan = planner.create_plan(&query);

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].0, ExecutionStepId(0));
    assert_eq!(plan.steps[1].0, ExecutionStepId(1));

    match &plan.steps[0].1 {
        ExecutionStep::Text(t) => assert_eq!(t.pattern, "Rust"),
        _ => panic!("Expected Text step at index 0"),
    }

    match &plan.steps[1].1 {
        ExecutionStep::Semantic(s) => assert_eq!(s.prompt, "systems language"),
        _ => panic!("Expected Semantic step at index 1"),
    }
}

#[test]
fn test_noop_plan_optimizer() {
    let query = KnowledgeQuery::new().with_text("Rust");
    let planner = QueryPlanner::new();
    let plan = planner.create_plan(&query);

    let optimizer = NoOpOptimizer::new();
    let optimized = optimizer.optimize(plan.clone());

    assert_eq!(plan, optimized);
}

#[test]
fn test_query_executor_and_fusion_engine() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let query = KnowledgeQuery::new()
        .with_text("Rust")
        .with_semantic_prompt("Rust");

    let planner = QueryPlanner::new();
    let plan = planner.create_plan(&query);

    let executor = QueryExecutor::new();
    let candidate_sets = executor.execute_plan(&plan, &ctx);

    assert_eq!(candidate_sets.len(), 2);

    let fusion = ReciprocalRankFusion::default();
    let result = fusion.fuse(&candidate_sets, 10);

    assert_eq!(result.total_candidates, 1);
    assert_eq!(
        result.candidates[0].entity_id,
        EntityId("entity_rust".to_string())
    );
    assert!(result.candidates[0].score > 0.0);
}

#[test]
fn test_query_pipeline_end_to_end() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let query = KnowledgeQuery::new().with_text("Python");
    let pipeline = QueryPipeline::new();

    let result = pipeline.execute(&query, &ctx);

    assert_eq!(result.total_candidates, 1);
    assert_eq!(
        result.candidates[0].entity_id,
        EntityId("entity_python".to_string())
    );
}
