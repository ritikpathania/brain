#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    fn assert_no_dependency(crate_name: &str, forbidden_dep: &str) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let mut path = PathBuf::from(manifest_dir);
        path.pop(); // to crates/
        path.push(crate_name);
        path.push("Cargo.toml");

        let content = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Failed to read {}", path.display()));

        let target = format!("{} =", forbidden_dep);
        let target_quoted = format!("\"{}\"", forbidden_dep);

        assert!(
            !content.contains(&target) && !content.contains(&target_quoted),
            "Architectural Violation: Crate '{}' is forbidden from depending on '{}'.",
            crate_name,
            forbidden_dep
        );
    }

    #[test]
    fn test_dependency_boundaries() {
        // 1. brain-storage cannot depend on brain-tui
        assert_no_dependency("brain-storage", "brain-tui");

        // 2. brain-python cannot depend on brain-storage
        assert_no_dependency("brain-python", "brain-storage");

        // 3. brain-domain cannot depend on any other crate
        assert_no_dependency("brain-domain", "brain-core");
        assert_no_dependency("brain-domain", "brain-storage");
        assert_no_dependency("brain-domain", "brain-python");
        assert_no_dependency("brain-domain", "brain-tui");
        assert_no_dependency("brain-domain", "brain-services");
        assert_no_dependency("brain-domain", "brain-observability");
        assert_no_dependency("brain-domain", "brain-events");
        assert_no_dependency("brain-domain", "brain-config");
        assert_no_dependency("brain-domain", "brain-session");
        assert_no_dependency("brain-domain", "brain-tools");
        assert_no_dependency("brain-domain", "brain-plugins");

        // 4. brain-core cannot depend on brain-events
        assert_no_dependency("brain-core", "brain-events");
        assert_no_dependency("brain-core", "brain-storage");
        assert_no_dependency("brain-core", "brain-tui");
        assert_no_dependency("brain-core", "brain-services");
        assert_no_dependency("brain-core", "brain-python");
        assert_no_dependency("brain-core", "brain-plugins");

        // 5. brain-events cannot depend on downstream crates
        assert_no_dependency("brain-events", "brain-storage");
        assert_no_dependency("brain-events", "brain-services");
        assert_no_dependency("brain-events", "brain-tui");
        assert_no_dependency("brain-events", "brain-python");
        assert_no_dependency("brain-events", "brain-plugins");

        // 6. brain-config cannot depend on storage, services, tui, python, or plugins
        assert_no_dependency("brain-config", "brain-storage");
        assert_no_dependency("brain-config", "brain-services");
        assert_no_dependency("brain-config", "brain-tui");
        assert_no_dependency("brain-config", "brain-python");
        assert_no_dependency("brain-config", "brain-plugins");
    }
}
