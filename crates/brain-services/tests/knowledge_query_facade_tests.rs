use brain_domain::bkf::*;
use brain_domain::projection::Watermark;
use brain_services::query::*;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn test_knowledge_query_facade_lifecycle_and_evaluators() {
    let snapshot_v1 = Arc::new(ProjectionSnapshot::empty(Watermark(10)));
    let facade = KnowledgeQueryFacade::new(snapshot_v1);

    assert_eq!(facade.active_snapshot().watermark(), Watermark(10));

    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let query = NeighborhoodQuery {
        root_entity: entity_id,
        max_hops: 1,
        temporal_mode: TemporalMode::CurrentActive,
        confidence_filter: None,
        pagination: PaginationParams::default(),
    };

    let res = facade.query_neighborhood(&query).unwrap();
    assert_eq!(res.metadata.snapshot_watermark, 10);

    // Atomic snapshot update
    let snapshot_v2 = Arc::new(ProjectionSnapshot::empty(Watermark(20)));
    facade.update_snapshot(snapshot_v2);

    assert_eq!(facade.active_snapshot().watermark(), Watermark(20));
    let res2 = facade.query_neighborhood(&query).unwrap();
    assert_eq!(res2.metadata.snapshot_watermark, 20);
}

#[test]
fn test_knowledge_query_facade_concurrency_safety() {
    let snapshot_v1 = Arc::new(ProjectionSnapshot::empty(Watermark(100)));
    let facade = Arc::new(KnowledgeQueryFacade::new(snapshot_v1));

    let reader_handle = facade.active_snapshot();

    // Writer publishes new snapshot
    let snapshot_v2 = Arc::new(ProjectionSnapshot::empty(Watermark(200)));
    facade.update_snapshot(snapshot_v2);

    // Reader holding snapshot_v1 remains completely unaffected
    assert_eq!(reader_handle.watermark(), Watermark(100));
    assert_eq!(facade.active_snapshot().watermark(), Watermark(200));
}
