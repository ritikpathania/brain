use brain_domain::query::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_semantic_binder_allocates_slots_and_binds_ast() {
    let p_var = QueryVar::new("p");
    let c_var = QueryVar::new("c");

    let query = Query::builder()
        .pattern(Pattern::triple(
            p_var.clone(),
            brain_domain::bkf::PredicateName::new("LivesIn").unwrap(),
            c_var.clone(),
        ))
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    assert_eq!(bound.ast.patterns.len(), 1);
    assert_eq!(bound.schema.slot_count(), 2);
    assert_eq!(bound.schema.get_var(SlotId(0)), Some(&p_var));
    assert_eq!(bound.schema.get_var(SlotId(1)), Some(&c_var));
}
