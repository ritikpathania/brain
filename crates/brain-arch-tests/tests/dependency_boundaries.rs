//! Architecture boundary tests.
//!
//! These tests parse the workspace dependency graph and assert that the
//! layered architecture documented in `AGENTS.md` has not been violated.
//!
//! # Adding a new rule
//!
//! Append an [`ArchitectureRule`] to the `RULES` slice.  No test logic
//! needs to change — the test loop picks it up automatically.
//!
//! # Glob patterns
//!
//! `forbidden_deps` entries support a single `*` wildcard, matched as a
//! simple prefix + suffix check (e.g. `"brain-*-adapter"` matches
//! `"brain-mcp-adapter"`, `"brain-a2a-adapter"`, etc.).

use cargo_metadata::MetadataCommand;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Rule definitions
// ---------------------------------------------------------------------------

/// A single architectural boundary rule.
struct ArchitectureRule {
    /// The workspace crate this rule applies to.
    subject: &'static str,
    /// Human-readable description of the invariant being enforced.
    description: &'static str,
    /// Crate-name patterns that `subject` must **not** directly depend on.
    /// Supports one `*` wildcard (prefix + suffix match).
    forbidden_deps: &'static [&'static str],
    /// When `true`, `subject` must have zero direct workspace dependencies.
    must_have_no_workspace_deps: bool,
}

/// The architectural policy for this workspace.
///
/// Dependency direction: domain → services → application → adapters → binary
///
/// Each rule asserts one constraint on the **direct** dependencies of
/// `subject`.  Transitive enforcement is provided by transitivity: if A
/// cannot depend on B, and B cannot depend on C, then A cannot reach C
/// through B either.
const RULES: &[ArchitectureRule] = &[
    ArchitectureRule {
        subject: "brain-domain",
        description: "brain-domain is the bottom of the dependency DAG; \
                      it must have zero outgoing workspace dependencies so \
                      it can never pull in async runtimes, DB engines, or \
                      FFI modules (see AGENTS.md § DDD Invariants).",
        forbidden_deps: &[],
        must_have_no_workspace_deps: true,
    },
    ArchitectureRule {
        subject: "brain-services",
        description: "brain-services must not depend on any adapter crate. \
                      Adapters are consumers of services, not providers.",
        forbidden_deps: &["brain-*-adapter"],
        must_have_no_workspace_deps: false,
    },
    ArchitectureRule {
        subject: "brain-application",
        description: "brain-application is the use-case orchestration layer. \
                      It must not depend on any adapter; adapters depend on \
                      brain-application, not the other way around.",
        forbidden_deps: &["brain-*-adapter"],
        must_have_no_workspace_deps: false,
    },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `name` matches `pattern`.
///
/// `pattern` may contain at most one `*`, treated as a wildcard that matches
/// any substring.  If there is no `*`, the match is an exact equality check.
fn matches_pattern(name: &str, pattern: &str) -> bool {
    match pattern.find('*') {
        None => name == pattern,
        Some(star) => {
            let prefix = &pattern[..star];
            let suffix = &pattern[star + 1..];
            name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
        }
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn dependency_boundaries() {
    // Locate workspace root relative to this crate's manifest directory.
    // CARGO_MANIFEST_DIR = .../crates/brain-arch-tests
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("Could not resolve workspace root from CARGO_MANIFEST_DIR");

    let metadata = MetadataCommand::new()
        .manifest_path(workspace_root.join("Cargo.toml"))
        .exec()
        .expect("cargo metadata failed — is the Cargo.toml reachable?");

    // Build a map from package name → direct workspace-member dependencies.
    let workspace_names: HashSet<&str> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    // resolve_id -> package name, for looking up dependency names.
    let id_to_name: HashMap<&cargo_metadata::PackageId, &str> = metadata
        .packages
        .iter()
        .map(|p| (&p.id, p.name.as_str()))
        .collect();

    // For each workspace package, collect its direct workspace-member deps.
    let direct_workspace_deps: HashMap<&str, Vec<&str>> = metadata
        .workspace_packages()
        .iter()
        .map(|pkg| {
            let deps: Vec<&str> = pkg
                .dependencies
                .iter()
                .filter_map(|dep| {
                    // Match by name against workspace members; only count
                    // workspace-path deps (i.e. those whose name appears in
                    // the workspace member set).
                    if workspace_names.contains(dep.name.as_str()) {
                        Some(dep.name.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            (pkg.name.as_str(), deps)
        })
        .collect();

    let mut failures: Vec<String> = Vec::new();

    for rule in RULES {
        let deps = match direct_workspace_deps.get(rule.subject) {
            Some(d) => d,
            None => {
                // If the crate doesn't exist in the workspace at all, that's
                // a misconfiguration in the rule itself — fail loudly.
                failures.push(format!(
                    "[RULE CONFIG ERROR] subject crate '{}' not found in workspace",
                    rule.subject
                ));
                continue;
            }
        };

        // --- Check: no workspace deps at all ---
        if rule.must_have_no_workspace_deps && !deps.is_empty() {
            failures.push(format!(
                "\n[VIOLATION] {}\n  Rule   : must have zero workspace dependencies\n  Found  : {:?}",
                rule.subject, deps
            ));
        }

        // --- Check: no forbidden deps ---
        for dep in deps {
            for pattern in rule.forbidden_deps {
                if matches_pattern(dep, pattern) {
                    failures.push(format!(
                        "\n[VIOLATION] {}\n  Rule   : {}\n  Dep    : '{}' matches forbidden pattern '{}'",
                        rule.subject, rule.description, dep, pattern
                    ));
                }
            }
        }
    }

    // Drop id_to_name — used only during metadata loading above.
    drop(id_to_name);

    if !failures.is_empty() {
        panic!(
            "\n\nArchitecture boundary violations detected ({} failure(s)):\n{}\n\n\
             To fix: remove the forbidden dependency from the offending crate's \
             Cargo.toml, or update the rule in \
             crates/brain-arch-tests/tests/dependency_boundaries.rs if the \
             architecture has intentionally changed.\n",
            failures.len(),
            failures.join("\n")
        );
    }

    println!(
        "dependency_boundaries: {} rule(s) checked, 0 violations.",
        RULES.len()
    );
}
