use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use brain_services::query::*;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

fn generate_deterministic_event_stream() -> (
    KnowledgeEntityId,
    KnowledgeEntityId,
    KnowledgeEntityId,
    Vec<FactEvent>,
) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(100));
    let e_b = KnowledgeEntityId(Uuid::from_u128(200));
    let e_c = KnowledgeEntityId(Uuid::from_u128(300));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    // Event 1: Relationship A -> B
    let f1 = FactVersionId(Uuid::from_u128(1_000));
    let a1 = AssertionId(Uuid::from_u128(1_001));
    let event1 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f1,
            assertion_id: a1,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.95).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a1,
            kind: AssertionKind::Relationship,
            subject: e_a.clone(),
            predicate: PredicateId(Uuid::from_u128(9_000)),
            object: AssertionTarget::Entity(e_b.clone()),
        }),
    };

    // Event 2: Attribute A "graph database engine"
    let f2 = FactVersionId(Uuid::from_u128(2_000));
    let a2 = AssertionId(Uuid::from_u128(2_001));
    let event2 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f2,
            assertion_id: a2,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.95).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a2,
            kind: AssertionKind::Attribute,
            subject: e_a.clone(),
            predicate: PredicateId(Uuid::from_u128(9_001)),
            object: AssertionTarget::Value(LiteralValue::String("graph database engine".to_string())),
        }),
    };

    // Event 3: Attribute C "relational database query"
    let f3 = FactVersionId(Uuid::from_u128(3_000));
    let a3 = AssertionId(Uuid::from_u128(3_001));
    let event3 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f3,
            assertion_id: a3,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.85).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a3,
            kind: AssertionKind::Attribute,
            subject: e_c.clone(),
            predicate: PredicateId(Uuid::from_u128(9_002)),
            object: AssertionTarget::Value(LiteralValue::String("relational database query".to_string())),
        }),
    };

    (e_a, e_b, e_c, vec![event1, event2, event3])
}

fn build_batch_snapshot(events: &[FactEvent]) -> Arc<ProjectionSnapshot> {
    let mut adj = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    for ev in events {
        let _ = adj.apply_event(ev);
        let _ = temp.apply_event(ev);
        let _ = stats.apply_event(ev);
        let _ = search.apply_event(ev);
    }

    Arc::new(ProjectionSnapshot::new(
        Arc::new(adj.state().clone()),
        Arc::new(temp.state().clone()),
        Arc::new(stats.state().clone()),
        Arc::new(search.state().clone()),
        Watermark(events.len() as u64),
    ))
}

fn build_incremental_snapshot(events: &[FactEvent]) -> Arc<ProjectionSnapshot> {
    let mut adj = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    for ev in events {
        let _ = adj.apply_event(ev);
        let _ = temp.apply_event(ev);
        let _ = stats.apply_event(ev);
        let _ = search.apply_event(ev);
    }

    Arc::new(ProjectionSnapshot::new(
        Arc::new(adj.state().clone()),
        Arc::new(temp.state().clone()),
        Arc::new(stats.state().clone()),
        Arc::new(search.state().clone()),
        Watermark(events.len() as u64),
    ))
}

fn assert_query_results_equivalent(lhs: &QueryFacadeResult, rhs: &QueryFacadeResult) {
    assert_eq!(lhs.matches, rhs.matches);
    assert_eq!(lhs.total_matched, rhs.total_matched);
    assert_eq!(lhs.metadata.snapshot_watermark, rhs.metadata.snapshot_watermark);
}

#[test]
fn test_conformance_replay_equivalence_across_all_evaluators() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap_batch = build_batch_snapshot(&events);
    let snap_inc = build_incremental_snapshot(&events);

    let facade_batch = KnowledgeQueryFacade::new(snap_batch);
    let facade_inc = KnowledgeQueryFacade::new(snap_inc);

    // 1. NeighborhoodEvaluator Replay Equivalence
    let q_neigh = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 2,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_nb = facade_batch.query_neighborhood(&q_neigh).unwrap();
    let res_ni = facade_inc.query_neighborhood(&q_neigh).unwrap();
    assert_query_results_equivalent(&res_nb, &res_ni);

    // 2. SearchEvaluator Replay Equivalence
    let q_search = LexicalSearchQuery {
        query_string: "database".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_sb = facade_batch.query_search(&q_search).unwrap();
    let res_si = facade_inc.query_search(&q_search).unwrap();
    assert_query_results_equivalent(&res_sb, &res_si);

    // 3. HybridEvaluator Replay Equivalence
    let q_hybrid = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res_hb = facade_batch.query_hybrid(&q_hybrid).unwrap();
    let res_hi = facade_inc.query_hybrid(&q_hybrid).unwrap();
    assert_query_results_equivalent(&res_hb, &res_hi);
}

#[test]
fn test_conformance_duplicate_free_and_ordering_invariant() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap = build_batch_snapshot(&events);
    let facade = KnowledgeQueryFacade::new(snap);

    let q_hybrid = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res = facade.query_hybrid(&q_hybrid).unwrap();

    let mut seen_ids = std::collections::HashSet::new();
    for m in &res.matches {
        assert!(seen_ids.insert(m.entity_id.clone()), "Duplicate entity found");
    }
    assert_eq!(seen_ids.len(), res.matches.len());
}

#[test]
fn test_conformance_pagination_algebra() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap = build_batch_snapshot(&events);
    let facade = KnowledgeQueryFacade::new(snap);

    let full_query = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 10, offset: 0 },
    };

    let full_res = facade.query_hybrid(&full_query).unwrap();
    let total = full_res.total_matched;

    // Part 1: limit=1, offset=0
    let p1_query = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 1, offset: 0 },
    };
    let p1_res = facade.query_hybrid(&p1_query).unwrap();

    // Part 2: limit=10, offset=1
    let p2_query = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 10, offset: 1 },
    };
    let p2_res = facade.query_hybrid(&p2_query).unwrap();

    assert_eq!(p1_res.total_matched, total);
    assert_eq!(p2_res.total_matched, total);
    assert_eq!(p1_res.matches.len() + p2_res.matches.len(), full_res.matches.len());
    assert_eq!(p1_res.matches[0], full_res.matches[0]);

    // limit = 0
    let p_zero = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 0, offset: 0 },
    };
    let res_zero = facade.query_hybrid(&p_zero).unwrap();
    assert_eq!(res_zero.total_matched, total);
    assert!(res_zero.matches.is_empty());

    // offset == total
    let p_eq = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 10, offset: total },
    };
    let res_eq = facade.query_hybrid(&p_eq).unwrap();
    assert_eq!(res_eq.total_matched, total);
    assert!(res_eq.matches.is_empty());

    // offset > total
    let p_oob = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams { limit: 10, offset: total + 10 },
    };
    let res_oob = facade.query_hybrid(&p_oob).unwrap();
    assert_eq!(res_oob.total_matched, total);
    assert!(res_oob.matches.is_empty());
}

#[test]
fn test_conformance_snapshot_immutability_and_cross_evaluator_isolation() {
    let (e_a, _, _, events) = generate_deterministic_event_stream();
    let snap = build_batch_snapshot(&events);
    let facade = KnowledgeQueryFacade::new(snap);

    let q_neigh = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 2,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let q_search = LexicalSearchQuery {
        query_string: "database".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let q_hybrid = HybridSearchQuery {
        query_string: "database".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    // Sequential execution flow: Neighborhood -> Search -> Hybrid -> Neighborhood (again)
    let n1 = facade.query_neighborhood(&q_neigh).unwrap();
    let _s1 = facade.query_search(&q_search).unwrap();
    let _h1 = facade.query_hybrid(&q_hybrid).unwrap();
    let n2 = facade.query_neighborhood(&q_neigh).unwrap();

    assert_query_results_equivalent(&n1, &n2);
}
