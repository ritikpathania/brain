use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn test_cargo_toml_rust_version_is_1_99_1() {
    let cargo_path = workspace_root().join("Cargo.toml");
    let content = fs::read_to_string(&cargo_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", cargo_path.display(), e));
    let parsed: toml::Value =
        toml::from_str(&content).unwrap_or_else(|e| panic!("Failed to parse Cargo.toml: {}", e));

    let rust_version = parsed
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("rust-version"))
        .and_then(|v| v.as_str())
        .expect("workspace.package.rust-version must be declared as a string");

    assert_eq!(
        rust_version, "1.99.1",
        "workspace.package.rust-version in Cargo.toml must be 1.99.1, found {}",
        rust_version
    );
}

#[test]
fn test_rust_toolchain_channel_is_1_99_1() {
    let toolchain_path = workspace_root().join("rust-toolchain.toml");
    let content = fs::read_to_string(&toolchain_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", toolchain_path.display(), e));
    let parsed: toml::Value = toml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse rust-toolchain.toml: {}", e));

    let channel = parsed
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(|c| c.as_str())
        .expect("toolchain.channel must be declared as a string");

    assert_eq!(
        channel, "1.99.1",
        "toolchain.channel in rust-toolchain.toml must be 1.99.1, found {}",
        channel
    );
}

#[test]
fn test_ci_workflow_specifies_1_99_1_and_removes_obsolete_msrv_job() {
    let ci_path = workspace_root().join(".github/workflows/ci.yml");
    let content = fs::read_to_string(&ci_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", ci_path.display(), e));

    // Must not contain stable alias in rust-toolchain setup
    assert!(
        !content.contains("dtolnay/rust-toolchain@stable"),
        "CI workflow must not use stable alias: uses: dtolnay/rust-toolchain@stable"
    );

    // Must not contain obsolete 1.80.0 toolchain references
    assert!(
        !content.contains("1.80.0"),
        "CI workflow must not reference obsolete 1.80.0 toolchain"
    );

    // Must not contain the obsolete msrv-check job
    assert!(
        !content.contains("msrv-check:"),
        "CI workflow must have removed the obsolete msrv-check job"
    );

    // Must specify 1.99.1 explicitly for toolchains
    assert!(
        content.contains("dtolnay/rust-toolchain@1.99.1"),
        "CI workflow must specify dtolnay/rust-toolchain@1.99.1 explicitly"
    );
}

#[test]
fn test_release_workflow_specifies_1_99_1_explicitly() {
    let release_path = workspace_root().join(".github/workflows/release.yml");
    let content = fs::read_to_string(&release_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", release_path.display(), e));

    // Must not contain stable alias
    assert!(
        !content.contains("dtolnay/rust-toolchain@stable"),
        "Release workflow must not use stable alias"
    );

    // Must specify 1.99.1 explicitly
    assert!(
        content.contains("dtolnay/rust-toolchain@1.99.1"),
        "Release workflow must specify dtolnay/rust-toolchain@1.99.1 explicitly"
    );
}

#[test]
fn test_active_docs_specify_1_99_1() {
    let root = workspace_root();

    let readme = fs::read_to_string(root.join("README.md")).expect("README.md should be readable");
    assert!(
        !readme.contains("1.80%2B"),
        "README.md must not reference obsolete 1.80%2B MSRV badge"
    );
    assert!(
        !readme.contains("Rust 1.80+"),
        "README.md must not reference obsolete Rust 1.80+ baseline"
    );
    assert!(
        readme.contains("1.99.1%2B"),
        "README.md must reference 1.99.1%2B"
    );
    assert!(
        readme.contains("Rust 1.99.1+"),
        "README.md must reference Rust 1.99.1+"
    );

    let install = fs::read_to_string(root.join("docs/guides/installation.md"))
        .expect("docs/guides/installation.md should be readable");
    assert!(
        !install.contains("1.80.0"),
        "installation.md must not reference obsolete 1.80.0 MSRV"
    );
    assert!(
        install.contains("1.99.1"),
        "installation.md must reference 1.99.1 MSRV"
    );
}
