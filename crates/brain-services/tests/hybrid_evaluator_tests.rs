use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use brain_services::query::evaluators::*;
use brain_services::query::*;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

fn setup_hybrid_test_snapshot() -> (KnowledgeEntityId, KnowledgeEntityId, KnowledgeEntityId, Arc<ProjectionSnapshot>) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(100));
    let e_b = KnowledgeEntityId(Uuid::from_u128(200));
    let e_c = KnowledgeEntityId(Uuid::from_u128(300));

    let mut adj_reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    // Entity A: "graph database" (conf 0.95), connected to Entity B
    let f1 = FactVersionId(Uuid::new_v4());
    let a1 = AssertionId(Uuid::new_v4());
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
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Entity(e_b.clone()),
        }),
    };

    // Entity A search literal "graph database"
    let f2 = FactVersionId(Uuid::new_v4());
    let a2 = AssertionId(Uuid::new_v4());
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
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String("graph database".to_string())),
        }),
    };

    // Entity C: "relational database"
    let f3 = FactVersionId(Uuid::new_v4());
    let a3 = AssertionId(Uuid::new_v4());
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
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String("relational database".to_string())),
        }),
    };

    for ev in &[&event1, &event2, &event3] {
        let _ = adj_reducer.apply_event(ev);
        let _ = temp_reducer.apply_event(ev);
        let _ = stats_reducer.apply_event(ev);
        let _ = search_reducer.apply_event(ev);
    }

    let snapshot = ProjectionSnapshot::new(
        Arc::new(adj_reducer.state().clone()),
        Arc::new(temp_reducer.state().clone()),
        Arc::new(stats_reducer.state().clone()),
        Arc::new(search_reducer.state().clone()),
        Watermark(3),
    );

    (e_a, e_b, e_c, Arc::new(snapshot))
}

#[test]
fn test_hybrid_multi_modal_fusion_and_metadata_merge() {
    let (e_a, e_b, _e_c, snapshot) = setup_hybrid_test_snapshot();

    // Query combines search query_string "graph" AND root_entity e_a
    let query_hybrid = HybridSearchQuery {
        query_string: "graph".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res = HybridEvaluator::evaluate(&snapshot, &query_hybrid).unwrap();
    // Candidates: e_a (search + graph root), e_b (graph neighbor of e_a)
    assert_eq!(res.total_matched, 2);

    // Entity A was found by BOTH lexical search and graph expansion -> metadata merged!
    let match_a = res.matches.iter().find(|m| m.entity_id == e_a).unwrap();
    assert!(match_a.search_metadata.is_some());
    assert!(match_a.graph_metadata.is_some());
    assert_eq!(match_a.search_metadata.as_ref().unwrap().matched_terms, vec!["graph"]);

    // Entity B was found by graph expansion only -> graph_metadata present, search_metadata None
    let match_b = res.matches.iter().find(|m| m.entity_id == e_b).unwrap();
    assert!(match_b.search_metadata.is_none());
    assert!(match_b.graph_metadata.is_some());
}

#[test]
fn test_hybrid_lexical_only_and_neighborhood_only() {
    let (e_a, _e_b, e_c, snapshot) = setup_hybrid_test_snapshot();

    // Lexical only (root_entity: None)
    let query_lexical = HybridSearchQuery {
        query_string: "relational".to_string(),
        root_entity: None,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res_lex = HybridEvaluator::evaluate(&snapshot, &query_lexical).unwrap();
    assert_eq!(res_lex.total_matched, 1);
    assert_eq!(res_lex.matches[0].entity_id, e_c);

    // Neighborhood only (query_string: "")
    let query_neigh = HybridSearchQuery {
        query_string: "".to_string(),
        root_entity: Some(e_a.clone()),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res_neigh = HybridEvaluator::evaluate(&snapshot, &query_neigh).unwrap();
    assert_eq!(res_neigh.total_matched, 2);
}

#[test]
fn test_hybrid_idempotent_fusion() {
    let (e_a, _, _, snapshot) = setup_hybrid_test_snapshot();

    let query_hybrid = HybridSearchQuery {
        query_string: "graph".to_string(),
        root_entity: Some(e_a),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        ordering: None,
        pagination: PaginationParams::default(),
    };

    let res1 = HybridEvaluator::evaluate(&snapshot, &query_hybrid).unwrap();
    let res2 = HybridEvaluator::evaluate(&snapshot, &query_hybrid).unwrap();

    assert_eq!(res1.total_matched, res2.total_matched);
    assert_eq!(res1.matches, res2.matches);
}
