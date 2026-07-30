use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

struct MockReducer(usize);
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("mock") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &FactEvent) -> Result<(), ProjectionError> {
        self.0 += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.0 = 0;
        Ok(())
    }
}

#[test]
fn test_projection_runtime_graceful_shutdown() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let instance = ProjectionInstance::new(Box::new(MockReducer(0)));
    runtime.register_projection(instance).unwrap();

    let event = FactEvent::FactArchived {
        fact_id: FactVersionId(Uuid::new_v4()),
        archived_at: Timestamp::now(),
    };

    runtime.dispatch_event(&event, 1).unwrap();
    runtime.shutdown().unwrap();
}
