use brain_domain::projection::Watermark;
use brain_services::query::snapshot::ProjectionSnapshot;

#[test]
fn test_projection_snapshot_accessors_and_watermark() {
    let snapshot = ProjectionSnapshot::empty(Watermark(42));
    assert_eq!(snapshot.watermark(), Watermark(42));
    assert!(snapshot.graph().is_empty());
    assert!(snapshot.temporal().is_empty());
    assert!(snapshot.statistics().is_empty());
    assert!(snapshot.search().is_empty());
}
