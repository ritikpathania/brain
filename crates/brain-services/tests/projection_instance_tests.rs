use brain_domain::bkf::events::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;

struct MockReducer;
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId {
        ProjectionId::new("mock")
    }
    fn version(&self) -> ProjectionVersion {
        ProjectionVersion(1)
    }
    fn apply_event(&mut self, _event: &FactEvent) -> Result<(), ProjectionError> {
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        Ok(())
    }
}

#[test]
fn test_projection_instance_lifecycle_transitions() {
    let reducer = Box::new(MockReducer);
    let mut instance = ProjectionInstance::new(reducer);

    assert_eq!(instance.lifecycle(), ProjectionLifecycle::Registered);
    instance.set_lifecycle(ProjectionLifecycle::Live);
    assert_eq!(instance.lifecycle(), ProjectionLifecycle::Live);
    assert_eq!(instance.metrics().events_processed, 0);
}
