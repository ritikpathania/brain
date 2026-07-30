use brain_domain::bkf::Timestamp;
use brain_domain::projection::*;
use brain_services::projection::store::*;

#[test]
fn test_checkpoint_store_save_and_load() {
    let mut store = InMemoryCheckpointStore::new();
    let id = ProjectionId::new("p1");
    let checkpoint = Checkpoint {
        projection_id: id.clone(),
        version: ProjectionVersion(1),
        watermark: Watermark(50),
        timestamp: Timestamp::now(),
        state_hash: None,
    };

    store.save_checkpoint_atomic(&checkpoint).unwrap();
    let loaded = store.load_checkpoint(&id).unwrap().unwrap();
    assert_eq!(loaded.watermark, Watermark(50));
}
