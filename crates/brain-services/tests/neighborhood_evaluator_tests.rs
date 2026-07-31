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

fn setup_test_snapshot() -> (KnowledgeEntityId, KnowledgeEntityId, KnowledgeEntityId, KnowledgeEntityId, Arc<ProjectionSnapshot>) {
    let e_a = KnowledgeEntityId(Uuid::from_u128(10));
    let e_b = KnowledgeEntityId(Uuid::from_u128(20));
    let e_c = KnowledgeEntityId(Uuid::from_u128(30));
    let e_d = KnowledgeEntityId(Uuid::from_u128(40));

    let mut adj_reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    let now = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    
    // A -> B (conf 0.9)
    let f1 = FactVersionId(Uuid::new_v4());
    let a1 = AssertionId(Uuid::new_v4());
    let event1 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f1,
            assertion_id: a1,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.9).unwrap(),
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

    // B -> C (conf 0.85)
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
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a2,
            kind: AssertionKind::Relationship,
            subject: e_b.clone(),
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Entity(e_c.clone()),
        }),
    };

    // C -> D (conf 0.80)
    let f3 = FactVersionId(Uuid::new_v4());
    let a3 = AssertionId(Uuid::new_v4());
    let event3 = FactEvent::FactRecorded {
        fact: FactVersion {
            id: f3,
            assertion_id: a3,
            lifecycle: FactLifecycle::Verified,
            confidence: Confidence::new(0.8).unwrap(),
            temporal: TemporalWindow::new(now, now, now, None).unwrap(),
            supersedes: None,
            provenance: FactProvenance {
                source: FactProvenanceSource::Manual { user_id: "test".to_string() },
                derived_from: vec![],
            },
        },
        assertion: Some(SemanticAssertion {
            id: a3,
            kind: AssertionKind::Relationship,
            subject: e_c.clone(),
            predicate: PredicateId(Uuid::new_v4()),
            object: AssertionTarget::Entity(e_d.clone()),
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

    (e_a, e_b, e_c, e_d, Arc::new(snapshot))
}

#[test]
fn test_neighborhood_max_hops_zero_and_boundary() {
    let (e_a, e_b, e_c, e_d, snapshot) = setup_test_snapshot();

    // max_hops = 0 -> returns only root_entity
    let query_zero = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 0,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_zero = NeighborhoodEvaluator::evaluate(&snapshot, &query_zero).unwrap();
    assert_eq!(res_zero.total_matched, 1);
    assert_eq!(res_zero.matches[0].entity_id, e_a);

    // max_hops = 1 -> discovers e_a, e_b
    let query_one = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 1,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_one = NeighborhoodEvaluator::evaluate(&snapshot, &query_one).unwrap();
    assert_eq!(res_one.total_matched, 2);
    assert_eq!(res_one.matches[0].entity_id, e_a);
    assert_eq!(res_one.matches[1].entity_id, e_b);

    // max_hops = 3 -> discovers e_a, e_b, e_c, e_d (sorted by confidence DESC: e_a 0.9, e_b 0.85, e_c 0.80, e_d 0.0)
    let query_three = NeighborhoodQuery {
        root_entity: e_a.clone(),
        max_hops: 3,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res_three = NeighborhoodEvaluator::evaluate(&snapshot, &query_three).unwrap();
    assert_eq!(res_three.total_matched, 4);
    assert_eq!(res_three.matches[0].entity_id, e_a);
    assert_eq!(res_three.matches[1].entity_id, e_b);
    assert_eq!(res_three.matches[2].entity_id, e_c);
    assert_eq!(res_three.matches[3].entity_id, e_d);
}

#[test]
fn test_neighborhood_missing_root_entity() {
    let (_, _, _, _, snapshot) = setup_test_snapshot();
    let missing_entity = KnowledgeEntityId(Uuid::from_u128(99999));

    let query = NeighborhoodQuery {
        root_entity: missing_entity,
        max_hops: 2,
        temporal_mode: TemporalMode::AllHistorical,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res = NeighborhoodEvaluator::evaluate(&snapshot, &query).unwrap();
    assert_eq!(res.total_matched, 0);
    assert!(res.matches.is_empty());
}
