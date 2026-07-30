use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::registry_dag::*;

struct DummyPass {
    id: PassId,
    deps: Vec<PassId>,
}

impl V2ReflectionPass for DummyPass {
    fn id(&self) -> PassId {
        self.id.clone()
    }

    fn dependencies(&self) -> &[PassId] {
        &self.deps
    }

    fn analyze(
        &self,
        _snapshot: &dyn KnowledgeSnapshotView,
        _context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String> {
        Ok(None)
    }
}

#[test]
fn test_topological_sort_independent_and_branching() {
    let mut registry = PassRegistryV2::new();
    let p_canon = DummyPass { id: PassId::new("canonicalization"), deps: vec![] };
    let p_contra = DummyPass { id: PassId::new("contradiction"), deps: vec![PassId::new("canonicalization")] };
    let p_dup = DummyPass { id: PassId::new("duplicate"), deps: vec![PassId::new("canonicalization")] };
    let p_conf = DummyPass { id: PassId::new("confidence"), deps: vec![PassId::new("contradiction"), PassId::new("duplicate")] };

    registry.register(Box::new(p_conf)).unwrap();
    registry.register(Box::new(p_dup)).unwrap();
    registry.register(Box::new(p_contra)).unwrap();
    registry.register(Box::new(p_canon)).unwrap();

    let sorted = registry.resolve_execution_order().unwrap();
    let names: Vec<String> = sorted.iter().map(|p| p.id().as_str().to_string()).collect();

    // Canonicalization must be first
    assert_eq!(names[0], "canonicalization");
    // Confidence must be last
    assert_eq!(names[3], "confidence");
}

#[test]
fn test_duplicate_registration_rejection() {
    let mut registry = PassRegistryV2::new();
    let p1 = DummyPass { id: PassId::new("p1"), deps: vec![] };
    let p2 = DummyPass { id: PassId::new("p1"), deps: vec![] };

    assert!(registry.register(Box::new(p1)).is_ok());
    assert!(registry.register(Box::new(p2)).is_err());
}

#[test]
fn test_missing_dependency_rejection() {
    let mut registry = PassRegistryV2::new();
    let p1 = DummyPass { id: PassId::new("p1"), deps: vec![PassId::new("missing_dep")] };

    registry.register(Box::new(p1)).unwrap();
    assert!(registry.resolve_execution_order().is_err());
}

#[test]
fn test_cyclic_dependency_rejection() {
    let mut registry = PassRegistryV2::new();
    let p1 = DummyPass { id: PassId::new("p1"), deps: vec![PassId::new("p2")] };
    let p2 = DummyPass { id: PassId::new("p2"), deps: vec![PassId::new("p1")] };

    registry.register(Box::new(p1)).unwrap();
    registry.register(Box::new(p2)).unwrap();
    assert!(registry.resolve_execution_order().is_err());
}
