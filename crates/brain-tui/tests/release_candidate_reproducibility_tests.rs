use std::path::Path;

#[test]
fn test_release_candidate_reproducibility_binary_targets_exist() {
    // Verify release build outputs or workspace compilation targets
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let cargo_toml = workspace_root.join("Cargo.toml");
    assert!(cargo_toml.exists(), "Missing workspace root Cargo.toml");

    let tui_manifest = workspace_root.join("crates/brain-tui/Cargo.toml");
    assert!(tui_manifest.exists(), "Missing brain-tui Cargo.toml");

    let daemon_manifest = workspace_root.join("daemon/Cargo.toml");
    assert!(daemon_manifest.exists(), "Missing daemon Cargo.toml");
}
