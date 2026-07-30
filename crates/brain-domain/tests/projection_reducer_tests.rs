use brain_domain::bkf::events::*;
use brain_domain::projection::*;

struct DummyReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    count: usize,
}

impl ProjectionReducer for DummyReducer {
    fn id(&self) -> ProjectionId { self.id.clone() }
    fn version(&self) -> ProjectionVersion { self.version }
    fn apply_event(&mut self, _event: &FactEvent) -> Result<(), ProjectionError> {
        self.count += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.count = 0;
        Ok(())
    }
}

#[test]
fn test_projection_reducer_contract() {
    let reducer = DummyReducer {
        id: ProjectionId::new("dummy"),
        version: ProjectionVersion(1),
        count: 0,
    };
    assert_eq!(reducer.id().as_str(), "dummy");
    assert_eq!(reducer.count, 0);
}
