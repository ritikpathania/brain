use brain_domain::projection::id::*;
use brain_domain::projection::watermark::*;

#[test]
fn test_projection_id_and_watermark() {
    let id = ProjectionId::new("graph_adjacency");
    let version = ProjectionVersion(1);
    let watermark = Watermark(100);

    assert_eq!(id.as_str(), "graph_adjacency");
    assert_eq!(version.0, 1);
    assert_eq!(watermark.0, 100);
}
