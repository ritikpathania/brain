use brain_domain::bkf::*;
use brain_services::query::filters::*;
use brain_services::query::*;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

#[test]
fn test_filter_by_confidence_threshold_order_preservation() {
    let e1 = KnowledgeEntityId(Uuid::from_u128(1));
    let e2 = KnowledgeEntityId(Uuid::from_u128(2));
    let e3 = KnowledgeEntityId(Uuid::from_u128(3));

    let mut candidates = vec![
        EntityMatch {
            entity_id: e1,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e2,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.5).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e3,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.85).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
    ];

    let filter = ConfidenceFilter {
        min_confidence: Confidence::new(0.8).unwrap(),
    };

    filter_by_confidence(&mut candidates, Some(&filter));

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].entity_id, e1);
    assert_eq!(candidates[1].entity_id, e3);
}

#[test]
fn test_is_valid_at_half_open_interval() {
    let t10 = Timestamp(UNIX_EPOCH + Duration::from_secs(10));
    let t20 = Timestamp(UNIX_EPOCH + Duration::from_secs(20));
    let t30 = Timestamp(UNIX_EPOCH + Duration::from_secs(30));

    // Valid: [10, 30) at t20
    assert!(is_valid_at(t10, Some(t30), t20));
    // Valid: inclusive lower bound [10, 30) at t10
    assert!(is_valid_at(t10, Some(t30), t10));
    // Invalid: exclusive upper bound [10, 30) at t30
    assert!(!is_valid_at(t10, Some(t30), t30));
    // Valid: open upper bound [10, None) at t30
    assert!(is_valid_at(t10, None, t30));
}

#[test]
fn test_sort_matches_deterministic_tie_breaking() {
    let uuid_b = Uuid::from_u128(20);
    let uuid_a = Uuid::from_u128(10);
    let uuid_c = Uuid::from_u128(30);

    let e_b = KnowledgeEntityId(uuid_b);
    let e_a = KnowledgeEntityId(uuid_a);
    let e_c = KnowledgeEntityId(uuid_c);

    let mut candidates = vec![
        EntityMatch {
            entity_id: e_b,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e_a,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e_c,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
    ];

    let ordering = QueryOrdering {
        field: SortField::Confidence,
        direction: SortDirection::Descending,
    };

    sort_matches(&mut candidates, Some(&ordering));

    // Primary keys equal (0.9), secondary tie-breaker EntityId ASC (uuid_a, uuid_b, uuid_c)
    assert_eq!(candidates[0].entity_id, e_a);
    assert_eq!(candidates[1].entity_id, e_b);
    assert_eq!(candidates[2].entity_id, e_c);
}

#[test]
fn test_paginate_matches_boundary_conditions() {
    let e1 = KnowledgeEntityId(Uuid::from_u128(1));
    let e2 = KnowledgeEntityId(Uuid::from_u128(2));

    let candidates = vec![
        EntityMatch {
            entity_id: e1,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.9).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
        EntityMatch {
            entity_id: e2,
            active_facts_count: 1,
            average_confidence: Confidence::new(0.8).unwrap(),
            graph_metadata: None,
            search_metadata: None,
        },
    ];

    let (res, total) = paginate_matches(
        &candidates,
        &PaginationParams {
            limit: 1,
            offset: 0,
        },
    );
    assert_eq!(total, 2);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].entity_id, e1);

    // Offset equal to total
    let (res_eq, total_eq) = paginate_matches(
        &candidates,
        &PaginationParams {
            limit: 10,
            offset: 2,
        },
    );
    assert_eq!(total_eq, 2);
    assert!(res_eq.is_empty());

    // Offset out of bounds (offset > total)
    let (res_oob, total_oob) = paginate_matches(
        &candidates,
        &PaginationParams {
            limit: 10,
            offset: 5,
        },
    );
    assert_eq!(total_oob, 2);
    assert!(res_oob.is_empty());
}
