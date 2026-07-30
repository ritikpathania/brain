use brain_domain::bkf::*;
use brain_domain::query::ast::*;
use brain_domain::query::filters::*;

#[test]
fn test_query_ast_builder() {
    let p_var = QueryVar::new("person");
    let c_var = QueryVar::new("city");

    let query = Query::builder()
        .pattern(Pattern::triple(
            p_var.clone(),
            PredicateName::new("LivesIn").unwrap(),
            c_var.clone(),
        ))
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    assert_eq!(query.patterns.len(), 1);
    assert_eq!(query.filters.len(), 1);
    assert_eq!(query.limit, Some(10));
}
