use brain_domain::bkf::events::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::registry::*;

struct MockReducer(String);
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId {
        ProjectionId::new(&self.0)
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
fn test_projection_registry_register_and_retrieve() {
    let mut registry = ProjectionRegistry::new();
    let instance = ProjectionInstance::new(Box::new(MockReducer("p1".to_string())));

    registry.register(instance).unwrap();
    assert!(registry.get(&ProjectionId::new("p1")).is_some());
}
