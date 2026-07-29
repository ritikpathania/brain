//! Brain Architecture Fitness Tests
//!
//! Automated CI fitness checks enforcing the 11 Constitutional Invariants
//! and 4-Layer Dependency Hierarchy specified in `CONSTITUTION.md`.

use cargo_metadata::MetadataCommand;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Allowed exception entry in `allowlist.toml`.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct AllowEntry {
    /// The rule being exempted
    pub rule: String,
    /// Crate name
    pub crate_name: Option<String>,
    /// Reason for exception
    pub reason: String,
    /// Expiration date string (YYYY-MM-DD)
    pub expires: String,
}

/// Allowlist container
#[derive(Debug, Deserialize, Default)]
pub struct AllowList {
    #[serde(default)]
    pub allow: Vec<AllowEntry>,
}

impl AllowList {
    /// Load allowlist from path if present
    pub fn load(path: &Path) -> Self {
        if path.exists() {
            let content = fs::read_to_string(path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Check if a rule exception is allowed
    pub fn is_allowed(&self, rule: &str, target_crate: &str) -> bool {
        self.allow.iter().any(|entry| {
            entry.rule == rule
                && entry
                    .crate_name
                    .as_deref()
                    .is_none_or(|c| c == target_crate)
        })
    }
}

/// Helper to get workspace metadata
pub fn get_workspace_metadata() -> cargo_metadata::Metadata {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("Cargo.toml");
    MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .expect("Failed to execute cargo_metadata")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_layer_dependency_hierarchy() {
        let metadata = get_workspace_metadata();
        let allowlist_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("allowlist.toml");
        let allowlist = AllowList::load(&allowlist_path);

        let domain_crate = metadata
            .workspace_packages()
            .into_iter()
            .find(|p| p.name == "brain-domain")
            .expect("brain-domain crate missing in workspace");

        // brain-domain MUST have zero outgoing dependencies on other workspace crates
        let workspace_crate_names: HashSet<&str> = metadata
            .workspace_packages()
            .into_iter()
            .map(|p| p.name.as_str())
            .collect();

        for dep in &domain_crate.dependencies {
            if workspace_crate_names.contains(dep.name.as_str()) {
                assert!(
                    allowlist.is_allowed("domain_zero_workspace_deps", &dep.name),
                    "Constitutional Violation (Dependency Hierarchy): brain-domain depends on workspace crate '{}'",
                    dep.name
                );
            }
        }
    }

    #[test]
    fn test_adapter_storage_isolation() {
        let metadata = get_workspace_metadata();
        let allowlist_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("allowlist.toml");
        let allowlist = AllowList::load(&allowlist_path);

        let adapter_crates = vec!["daemon_bridge", "brain"];

        for package in metadata.workspace_packages() {
            if adapter_crates.contains(&package.name.as_str()) {
                for dep in &package.dependencies {
                    if dep.name == "brain-storage" {
                        assert!(
                            allowlist.is_allowed("adapter_storage_isolation", &package.name),
                            "Constitutional Violation (Invariant 4 - Adapter Isolation): Adapter crate '{}' directly depends on 'brain-storage'",
                            package.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_single_mutation_entry_declaration() {
        let metadata = get_workspace_metadata();
        let allowlist_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("allowlist.toml");
        let allowlist = AllowList::load(&allowlist_path);

        // Verify that presentation/TUI crate (brain-tui) does not write to brain-storage
        if let Some(tui_crate) = metadata
            .workspace_packages()
            .into_iter()
            .find(|p| p.name == "brain-tui")
        {
            for dep in &tui_crate.dependencies {
                if dep.name == "brain-storage" {
                    assert!(
                        allowlist.is_allowed("single_mutation_entry", "brain-tui"),
                        "Constitutional Violation (Invariant 1 - Single Mutation Entry): brain-tui depends directly on brain-storage"
                    );
                }
            }
        }
    }

    #[test]
    fn test_allowlist_parsing() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("allowlist.toml");
        let allowlist = AllowList::load(&path);
        assert!(!allowlist.is_allowed("non_existent_rule", "some_crate"));
    }
}
