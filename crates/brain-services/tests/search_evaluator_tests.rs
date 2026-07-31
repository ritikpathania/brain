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

fn setup_search_test_snapshot() -> (
    KnowledgeEntityId,
    KnowledgeEntityId,
    KnowledgeEntityId,
    Arc<ProjectionSnapshot>,
) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(100));
    let e_b = KnowledgeEntityId(Uuid::from_u128(200));
    let e_c = KnowledgeEntityId(Uuid::from_u128(300));

    let mut adj_reducer =
        GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer =
        TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer =
        EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer =
        SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    // Entity A: "graph database engine" (conf 0.95)
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
                source: FactProvenanceSource::Manual {
                    user_id: "test".to_string(),
                },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a1,
            kind: AssertionKind::Attribute,
            subject: e_a,
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String(
                "graph database engine".to_string(),
            )),
        }),
    };

    // Entity B: "relational database query" (conf 0.85)
    let f2 = FactVersionId(Uuid::new_v4());
    let a2 = AssertionId(Uuid::new_v4());
    let event2 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f2,
            assertion_id: a2,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.85).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual {
                    user_id: "test".to_string(),
                },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a2,
            kind: AssertionKind::Attribute,
            subject: e_b,
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String(
                "relational database query".to_string(),
            )),
        }),
    };

    // Entity C: "graph database query" (conf 0.85)
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
                source: FactProvenanceSource::Manual {
                    user_id: "test".to_string(),
                },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a3,
            kind: AssertionKind::Attribute,
            subject: e_c,
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Value(LiteralValue::String(
                "graph database query".to_string(),
            )),
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
fn test_search_empty_and_whitespace_query() {
    let (_, _, _, snapshot) = setup_search_test_snapshot();

    let query_empty = LexicalSearchQuery {
        query_string: "".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res = SearchEvaluator::evaluate(&snapshot, &query_empty).unwrap();
    assert_eq!(res.total_matched, 0);
    assert!(res.matches.is_empty());
}

#[test]
fn test_search_lexical_token_match_and_partial_score() {
    let (e_a, _e_b, e_c, snapshot) = setup_search_test_snapshot();

    // Query "database" matches e_a, e_b, e_c (all 3)
    let query_db = LexicalSearchQuery {
        query_string: "database".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res = SearchEvaluator::evaluate(&snapshot, &query_db).unwrap();
    assert_eq!(res.total_matched, 3);
    assert_eq!(res.matches[0].entity_id, e_a);
    assert_eq!(
        res.matches[0]
            .search_metadata
            .as_ref()
            .unwrap()
            .matched_terms
            .len(),
        1
    );

    // Query "graph query" -> e_c matches both ["graph", "query"] (matched_terms len 2)
    let query_gq = LexicalSearchQuery {
        query_string: "graph query".to_string(),
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_gq = SearchEvaluator::evaluate(&snapshot, &query_gq).unwrap();
    assert_eq!(res_gq.total_matched, 3);

    let match_c = res_gq.matches.iter().find(|m| m.entity_id == e_c).unwrap();
    let meta_c = match_c.search_metadata.as_ref().unwrap();
    assert_eq!(meta_c.matched_terms.len(), 2);
}
