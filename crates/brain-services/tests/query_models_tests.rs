use brain_domain::bkf::*;
use brain_services::query::*;
use uuid::Uuid;

#[test]
fn test_query_models_and_defaults() {
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let query = HybridSearchQuery {
        query_string: "rust graph".to_string(),
        root_entity: Some(entity_id.clone()),
        temporal_mode: TemporalMode::ValidAt(now),
        confidence_filter: Some(ConfidenceFilter {
            min_confidence: Confidence::new(0.8).unwrap(),
        }),
        ordering: Some(QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        }),
        pagination: PaginationParams::default(),
    };

    assert_eq!(query.pagination.limit, 50);
    assert_eq!(query.pagination.offset, 0);

    let match_item = EntityMatch {
        entity_id,
        active_facts_count: 5,
        average_confidence: Confidence::new(0.95).unwrap(),
        graph_metadata: Some(GraphMetadata {
            in_degree: 3,
            out_degree: 2,
        }),
        search_metadata: Some(SearchMetadata {
            matched_terms: vec!["rust".to_string(), "graph".to_string()],
        }),
    };

    let result = QueryFacadeResult {
        matches: vec![match_item],
        total_matched: 1,
        metadata: QueryResponseMetadata {
            execution_duration_us: 120,
            snapshot_watermark: 42,
        },
    };

    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.metadata.snapshot_watermark, 42);
}

#[test]
fn test_query_error_variants() {
    let err = QueryError::EntityNotFound("entity-1".to_string());
    assert_eq!(err.to_string(), "Entity not found: entity-1");
}
