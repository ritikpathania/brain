use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::*;

#[test]
fn test_entity_statistics_services_reexport() {
    let reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "stats");
}
