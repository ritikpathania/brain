use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;

#[test]
fn test_temporal_state_services_reexport() {
    let reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "temporal");
}
