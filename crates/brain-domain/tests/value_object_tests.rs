use brain_domain::bkf::value_objects::*;

#[test]
fn test_confidence_bounds() {
    assert!(Confidence::new(0.5).is_ok());
    assert!(Confidence::new(0.0).is_ok());
    assert!(Confidence::new(1.0).is_ok());
    assert!(Confidence::new(-0.1).is_err());
    assert!(Confidence::new(1.1).is_err());
}

#[test]
fn test_entity_name_normalization() {
    let name = EntityName::new("  John  ").unwrap();
    assert_eq!(name.as_str(), "John");
    assert!(EntityName::new("   ").is_err());
}

#[test]
fn test_predicate_name_normalization() {
    let pred = PredicateName::new("  LivesIn  ").unwrap();
    assert_eq!(pred.as_str(), "LivesIn");
    assert!(PredicateName::new("   ").is_err());
}
