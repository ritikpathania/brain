use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::registry::*;
use brain_services::projection::scheduler::*;
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
fn test_sequential_scheduler_dispatches_events() {
    let mut registry = ProjectionRegistry::new();
    let instance = ProjectionInstance::new(Box::new(MockReducer(0)));
    registry.register(instance).unwrap();

    let mut scheduler = SequentialProjectionScheduler::new();
    let event = FactEvent::FactArchived {
        fact_id: FactVersionId(Uuid::new_v4()),
        archived_at: Timestamp::now(),
    };
    scheduler.dispatch_event(&mut registry, &event, 1).unwrap();
}
