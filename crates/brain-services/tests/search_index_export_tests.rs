use brain_domain::projection::search_index::*;
use brain_domain::projection::*;

#[test]
fn test_search_index_services_reexport() {
    let reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));
    assert_eq!(reducer.id().as_str(), "search");
}
