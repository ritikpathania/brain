use brain_domain::*;
use std::path::Path;

fn make_def(
    id: &str,
    display: &str,
    inverse: Option<&str>,
    dir: Directionality,
    sym: bool,
) -> RelationDefinition {
    RelationDefinition {
        id: RelationId::new(id.to_string()),
        display_name: display.to_string(),
        inverse: inverse.map(|s| RelationId::new(s.to_string())),
        directionality: dir,
        symmetry: sym,
        transitivity: false,
        fallback_suppression: false,
        confidence_strategy: ConfidenceStrategy::SourceDefined,
        description: "Test definition".to_string(),
    }
}

#[test]
fn test_valid_registry_in_memory() {
    let defs = vec![
        make_def("uses", "uses", None, Directionality::Directed, false),
        make_def(
            "associated_with",
            "associated with",
            Some("associated_with"),
            Directionality::Undirected,
            true,
        ),
    ];
    let registry = RelationRegistry::new(defs).expect("Failed to build valid registry");
    assert_eq!(registry.len(), 2);
    assert!(registry.contains("uses"));
    assert!(registry.contains("associated_with"));
    assert!(!registry.is_empty());

    let uses_def = registry.get("uses").unwrap();
    assert_eq!(uses_def.display_name, "uses");
    assert_eq!(uses_def.directionality, Directionality::Directed);
}

#[test]
fn test_duplicate_relation_id() {
    let defs = vec![
        make_def("uses", "uses A", None, Directionality::Directed, false),
        make_def("uses", "uses B", None, Directionality::Directed, false),
    ];
    let result = RelationRegistry::new(defs);
    assert!(
        matches!(result, Err(RelationRegistryError::DuplicateRelation(ref id)) if id.as_str() == "uses")
    );
}

#[test]
fn test_empty_id() {
    let defs = vec![make_def(
        "",
        "empty id",
        None,
        Directionality::Directed,
        false,
    )];
    let result = RelationRegistry::new(defs);
    assert!(matches!(result, Err(RelationRegistryError::EmptyId)));
}

#[test]
fn test_empty_display_name() {
    let defs = vec![make_def("uses", "", None, Directionality::Directed, false)];
    let result = RelationRegistry::new(defs);
    assert!(
        matches!(result, Err(RelationRegistryError::EmptyDisplayName(ref id)) if id.as_str() == "uses")
    );
}

#[test]
fn test_undirected_not_symmetric() {
    // Undirected but symmetric = false -> should error
    let defs = vec![make_def(
        "associated_with",
        "associated",
        Some("associated_with"),
        Directionality::Undirected,
        false,
    )];
    let result = RelationRegistry::new(defs);
    assert!(
        matches!(result, Err(RelationRegistryError::UndirectedNotSymmetric(ref id)) if id.as_str() == "associated_with")
    );
}

#[test]
fn test_missing_inverse() {
    // Specifies inverse "does_not_exist" which is missing
    let defs = vec![make_def(
        "uses",
        "uses",
        Some("does_not_exist"),
        Directionality::Directed,
        false,
    )];
    let result = RelationRegistry::new(defs);
    assert!(
        matches!(result, Err(RelationRegistryError::MissingInverse { ref relation, ref inverse }) if relation.as_str() == "uses" && inverse.as_str() == "does_not_exist")
    );
}

#[test]
fn test_symmetric_has_distinct_inverse() {
    // Symmetric is true, but inverse is "depends_on" (not itself)
    let defs = vec![
        make_def(
            "uses",
            "uses",
            Some("depends_on"),
            Directionality::Directed,
            true,
        ),
        make_def(
            "depends_on",
            "depends on",
            Some("uses"),
            Directionality::Directed,
            true,
        ),
    ];
    let result = RelationRegistry::new(defs);
    assert!(
        matches!(result, Err(RelationRegistryError::SymmetricHasDistinctInverse { ref relation, ref inverse })
            if (relation.as_str() == "uses" && inverse.as_str() == "depends_on") || (relation.as_str() == "depends_on" && inverse.as_str() == "uses")
        )
    );
}

#[test]
fn test_invalid_inverse_pair() {
    // A.inverse = B, but B.inverse = None
    let defs = vec![
        make_def(
            "uses",
            "uses",
            Some("depends_on"),
            Directionality::Directed,
            false,
        ),
        make_def(
            "depends_on",
            "depends on",
            None,
            Directionality::Directed,
            false,
        ),
    ];
    let result = RelationRegistry::new(defs);
    assert!(
        matches!(result, Err(RelationRegistryError::InvalidInversePair { ref rel_a, ref rel_b, .. }) if rel_a.as_str() == "uses" && rel_b.as_str() == "depends_on")
    );
}

#[test]
fn test_load_registry_from_file_conformance() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let json_path = Path::new(manifest_dir).join("../../protocol/relations.json");
    let file = std::fs::File::open(json_path).expect("Failed to open file");
    let defs: Vec<RelationDefinition> =
        serde_json::from_reader(file).expect("Failed to parse JSON");
    let registry = RelationRegistry::new(defs).expect("Failed to build registry");
    assert_eq!(registry.len(), 8);
    assert!(registry.contains("uses"));
    assert!(registry.contains("associated_with"));

    // Verify specific properties loaded correctly
    let assoc = registry.get("associated_with").unwrap();
    assert_eq!(assoc.directionality, Directionality::Undirected);
    assert!(assoc.symmetry);
    assert_eq!(
        assoc.inverse.as_ref().map(|i| i.as_str()),
        Some("associated_with")
    );

    let uses = registry.get("uses").unwrap();
    assert_eq!(uses.directionality, Directionality::Directed);
    assert!(!uses.symmetry);
    assert!(uses.transitivity);
    assert!(uses.fallback_suppression);
    assert_eq!(uses.confidence_strategy, ConfidenceStrategy::Maximum);
}

#[test]
fn test_registry_completeness() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let json_path = Path::new(manifest_dir).join("../../protocol/relations.json");
    let file = std::fs::File::open(json_path).expect("Failed to open file");
    let defs: Vec<RelationDefinition> =
        serde_json::from_reader(file).expect("Failed to parse JSON");
    let registry = RelationRegistry::new(defs).expect("Failed to build registry");

    // 1. Every compile-time RelationKind in ALL must exist in the registry
    for kind in RelationKind::ALL {
        assert!(
            registry.contains_kind(*kind),
            "Registry is missing RelationKind variant: {:?}",
            kind
        );
        let def = registry.get_kind(*kind).unwrap();
        assert_eq!(def.id, kind.id());
    }

    // 2. Every relation declared in relations.json must map to a valid compile-time RelationKind
    for def in registry.iter() {
        let parsed: RelationKind = def.id.as_str().parse().unwrap();
        assert_ne!(
            parsed,
            RelationKind::Unknown,
            "Registry has unregistered runtime ID: {}",
            def.id
        );
        assert_eq!(parsed.id(), def.id);
    }
}

#[test]
fn test_relation_id_invariants() {
    let id_a = RelationId::new("uses");
    let id_b = RelationId::new("uses");
    let id_c = RelationId::new("depends_on");

    // Eq / PartialEq
    assert_eq!(id_a, id_b);
    assert_ne!(id_a, id_c);

    // Ord / PartialOrd
    assert!(id_c < id_a); // "depends_on" < "uses" alphabetically

    // Hashing
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    id_a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    id_b.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());

    // Deref & AsRef
    assert_eq!(&*id_a, "uses");
    assert_eq!(id_a.as_ref(), "uses");
    assert_eq!(id_a.as_str(), "uses");

    // Conversions
    let s: String = id_a.clone().into();
    assert_eq!(s, "uses");
    let id_from_str = RelationId::from("runs_on");
    assert_eq!(id_from_str.as_str(), "runs_on");
}
