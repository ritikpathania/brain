use brain_domain::bkf::events::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::replay::*;

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
fn test_replay_engine_catchup_with_iterator() {
    let reducer = Box::new(MockReducer(0));
    let mut instance = ProjectionInstance::new(reducer);

    let empty_events: Vec<FactEvent> = vec![];
    ReplayEngine::replay_catchup(&mut instance, empty_events.iter(), Watermark(0)).unwrap();
    assert_eq!(instance.lifecycle(), ProjectionLifecycle::Live);
}
