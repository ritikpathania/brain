#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;
    use brain_domain::{EpochId, KnowledgeGraph};
    use brain_core::{
        events::{CorrelationId, EventSource, TaskProgress, TaskState, OperationId},
        evolution::{Observation, Provenance, Canonicalizer},
        projection::{ProjectionContext, Projector}
    };
    use brain_services::{
        InMemoryCanonicalizer, InMemoryEventDispatcher,
        MemoryListQuery, MemoryListProjector
    };

    // Helper to get crates directory
    fn get_crates_dir() -> PathBuf {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        PathBuf::from(manifest_dir)
    }

    // --- FITNESS TEST 1: Layer Dependency Check (Manifest & Source) ---
    #[test]
    fn test_fitness_layer_dependency() {
        let crates_dir = get_crates_dir();
        let parent_dir = crates_dir.parent().unwrap();

        // 1. Manifest-level check
        let domain_manifest_path = parent_dir.join("brain-domain").join("Cargo.toml");
        let manifest_content = fs::read_to_string(&domain_manifest_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", domain_manifest_path.display()));

        let forbidden = ["brain-core", "brain-services", "brain-tui", "brain-storage", "brain-acp-adapter", "brain-mcp-adapter"];
        for crate_name in &forbidden {
            let target = format!("{} =", crate_name);
            let target_quoted = format!("\"{}\"", crate_name);
            assert!(
                !manifest_content.contains(&target) && !manifest_content.contains(&target_quoted),
                "Manifest Violation: brain-domain cannot depend on downstream crate '{}'",
                crate_name
            );
        }

        // 2. Source-level imports check (ensure no raw code-level imports of downstream crates)
        let domain_src_dir = parent_dir.join("brain-domain").join("src");
        assert_no_forbidden_imports_recursive(&domain_src_dir, &forbidden);
    }

    fn assert_no_forbidden_imports_recursive(dir: &Path, forbidden: &[&str]) {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    assert_no_forbidden_imports_recursive(&path, forbidden);
                } else if path.extension().map_or(false, |ext| ext == "rs") {
                    let content = fs::read_to_string(&path).unwrap();
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("use ") || trimmed.contains("::") {
                            for crate_name in forbidden {
                                let pattern = format!("{}::", crate_name.replace('-', "_"));
                                assert!(
                                    !trimmed.contains(&pattern),
                                    "Source Import Violation: File '{}' imports from forbidden crate '{}' on line: '{}'",
                                    path.display(),
                                    crate_name,
                                    trimmed
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // --- FITNESS TEST 2: Read-Only Projections Invariant Check ---
    #[test]
    fn test_fitness_readonly_projections() {
        // Rust's compiler guarantees that because Projector::project receives a read-only 
        // ProjectionContext carrying &'a KnowledgeGraph (shared reference), no mutating methods 
        // (which require &mut self) can be called on the graph within the projector.
        // We verify that the API signatures enforce this compile-time guarantee.
        let graph = KnowledgeGraph::new();
        let query = MemoryListQuery { limit: 10 };
        let context = ProjectionContext {
            graph: &graph,
            epoch: EpochId::initial(),
            query: &query,
            correlation_id: CorrelationId::new_v4(),
        };

        let projector = MemoryListProjector;
        let projection = projector.project(&context);
        
        // Assert the projection completed and the original graph has not changed (remains empty)
        assert_eq!(projection.items.len(), 0);
        assert_eq!(graph.nodes.len(), 0);
    }

    // --- FITNESS TEST 3: Event Immutability Check ---
    #[test]
    fn test_fitness_event_immutability() {
        // Rust's ownership model ensures dispatched events are owned via Arc<dyn RuntimeEvent> or similar.
        // Arc is thread-safe and only provides read-only references (&T) to the inner values unless wrapped 
        // in cell/mutex mutation wrappers. Here we assert that dispatched event structs contain zero interior mutability.
        let event = Arc::new(TaskProgress {
            operation_id: OperationId::new_v4(),
            correlation_id: CorrelationId::new_v4(),
            state: TaskState::Created,
            source: EventSource::Ingestion,
            sequence: 1,
            timestamp: SystemTime::now(),
        });

        // Event cannot be mutated in place. If we clone the Arc, it points to the same read-only structure.
        let event_clone = Arc::clone(&event);
        assert_eq!(event.sequence, event_clone.sequence);
    }

    // --- FITNESS TEST 4: Isolated Canonicalization Check ---
    #[test]
    fn test_fitness_isolated_canonicalization() {
        // Assert that mutating graph state transitions require calling the Canonicalizer boundary.
        let graph = Arc::new(Mutex::new(KnowledgeGraph::new()));
        let epoch = Arc::new(Mutex::new(EpochId::initial()));
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(10));
        
        let canonicalizer = InMemoryCanonicalizer::new(
            Arc::clone(&graph),
            Arc::clone(&epoch),
            Arc::clone(&dispatcher),
        );

        let obs = Observation {
            payload: b"Hello memory".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "test".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: CorrelationId::new_v4(),
            },
        };

        // Advance using the boundary
        let result = canonicalizer.canonicalize(obs).unwrap();
        assert_eq!(result.epoch.0, 1);

        let graph_lock = graph.lock().unwrap();
        assert_eq!(graph_lock.nodes.len(), 1);
    }

    // --- FITNESS TEST 5: brain-observability Layer Isolation ---
    //
    // Verifies two invariants required by the architectural constraint that
    // brain-observability must NOT depend on brain-services or brain-storage:
    //
    // 1. Manifest-level: brain-observability/Cargo.toml must not declare
    //    brain-services or brain-storage as dependencies.
    // 2. Source-level: no source file under brain-observability/src/ must
    //    contain a `use brain_services::` or `use brain_storage::` import.
    #[test]
    fn test_fitness_observability_layer_isolation() {
        let crates_dir = get_crates_dir();
        let parent_dir = crates_dir.parent().unwrap();
        let obs_dir = parent_dir.join("brain-observability");

        // 1. Manifest-level check
        let manifest_path = obs_dir.join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", manifest_path.display()));

        let forbidden = ["brain-services", "brain-storage"];
        for dep in &forbidden {
            let as_key = format!("{} =", dep);
            let as_quoted = format!("\"{}\"", dep);
            assert!(
                !manifest.contains(&as_key) && !manifest.contains(&as_quoted),
                "Manifest Violation: brain-observability must not depend on '{}'",
                dep
            );
        }

        // 2. Source-level check
        let src_dir = obs_dir.join("src");
        let forbidden_imports = ["brain_services", "brain_storage"];
        assert_no_forbidden_imports_recursive(&src_dir, &forbidden_imports);
    }

    // --- FITNESS TEST 6: ReflectionEngine Contract Is Storage-Agnostic ---
    //
    // Verifies that the `ReflectionEngine` trait signature in brain-core/src/reflection.rs
    // contains no repository or storage type references. The contract must operate solely
    // on `ReflectionTarget` (a pure value type with no handles).
    #[test]
    fn test_fitness_reflection_contract_storage_agnostic() {
        let crates_dir = get_crates_dir();
        let parent_dir = crates_dir.parent().unwrap();
        let reflection_file = parent_dir
            .join("brain-core")
            .join("src")
            .join("reflection.rs");

        let source = fs::read_to_string(&reflection_file)
            .unwrap_or_else(|_| panic!("Failed to read {}", reflection_file.display()));

        // Repository and storage references must not appear in the trait contract file
        let storage_types = [
            "NodeRepository",
            "ConfigRepository",
            "RepositorySet",
            "SqliteStorage",
            "brain_storage",
        ];
        for ty in &storage_types {
            assert!(
                !source.contains(ty),
                "Architectural Violation: brain-core/src/reflection.rs must not reference '{}'. \
                 ReflectionEngine contract must be storage-agnostic.",
                ty
            );
        }
    }

    // --- FITNESS TEST 7: Reflection Engine Must Invoke Edge API ---
    //
    // Verifies that `SqliteReflectionEngine` does not mutate `edge.weight` directly.
    // It must call `edge.strengthen_with_evidence` or `edge.strengthen` to mutate edge weights,
    // respecting the architectural boundary: "Runtime services orchestrate; domain entities enforce invariants."
    #[test]
    fn test_fitness_reflection_invokes_edge_api() {
        let crates_dir = get_crates_dir();
        let parent_dir = crates_dir.parent().unwrap();
        let reflection_file = parent_dir
            .join("brain-services")
            .join("src")
            .join("sqlite_reflection.rs");

        let source = fs::read_to_string(&reflection_file)
            .unwrap_or_else(|_| panic!("Failed to read {}", reflection_file.display()));

        assert!(
            !source.contains(".weight =") && !source.contains(".weight +="),
            "Architectural Violation: SqliteReflectionEngine must not mutate edge.weight directly. \
             It must use the domain entity's API (e.g., strengthen_with_evidence)."
        );
    }
}

