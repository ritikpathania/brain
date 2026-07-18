use std::fs;
use std::path::{Path, PathBuf};

fn get_workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = Path::new(&manifest_dir);
    // CARGO_MANIFEST_DIR is crates/brain-domain
    manifest_path
        .parent()
        .expect("Missing crates dir")
        .parent()
        .expect("Missing workspace root")
        .to_path_buf()
}

// Map from crate name to the list of allowed workspace dependencies
fn get_allowed_dependencies(crate_name: &str) -> Vec<&str> {
    match crate_name {
        "brain-domain" => vec![],
        "brain-core" => vec!["brain-domain"],
        "brain-events" => vec!["brain-domain", "brain-core"],
        "brain-session" => vec!["brain-domain", "brain-core"],
        "brain-storage" => vec!["brain-domain", "brain-core", "brain-integrations"],
        "brain-config" => vec!["brain-domain", "brain-core"],
        "brain-tools" => vec!["brain-domain", "brain-core"],
        "brain-plugins" => vec!["brain-domain", "brain-core"],
        "brain-python" => vec!["brain-domain", "brain-core", "brain-plugins"],
        "brain-tui" => vec!["brain-domain", "brain-core", "brain-observability"],
        "brain-observability" => vec!["brain-core", "brain-domain"],
        "brain-integrations" => vec!["brain-domain"],
        "brain-sdk-rs" => vec!["brain-domain", "brain-integrations"],
        "brain-cli-adapter" => vec!["brain-sdk-rs", "brain-integrations", "brain-domain"],
        "brain-services" => vec![
            "brain-domain",
            "brain-core",
            "brain-config",
            "brain-storage",
            "brain-session",
            "brain-tools",
            "brain-plugins",
            "brain-python",
            "brain-observability",
            "brain-events",
        ],
        "brain-application" => vec![
            "brain-domain",
            "brain-core",
            "brain-services",
            "brain-integrations",
            "brain-storage",
            "brain-config",
        ],
        "brain-mcp-adapter" => vec![
            "brain-domain",
            "brain-services",
            "brain-integrations",
            "brain-application",
            "brain-adapter-core",
            "brain-config",
        ],
        "brain-acp-adapter" => vec![
            "brain-domain",
            "brain-services",
            "brain-integrations",
            "brain-application",
            "brain-adapter-core",
            "brain-config",
        ],
        "brain-a2a-adapter" => vec![
            "brain-domain",
            "brain-services",
            "brain-integrations",
            "brain-application",
            "brain-adapter-core",
            "brain-config",
        ],
        "brain-adapter-core" => vec![],
        _ => vec![], // Allow other / apps
    }
}

// Strips block comments and line comments from Rust source code to prevent false positives in comments/docs
fn clean_comments(content: &str) -> String {
    let mut result = String::new();
    let mut in_block_comment = false;
    let mut lines = content.lines();

    while let Some(line) = lines.next() {
        let mut cleaned_line = String::new();
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if in_block_comment {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block_comment = false;
                }
            } else if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                in_block_comment = true;
            } else if c == '/' && chars.peek() == Some(&'/') {
                // Line comment, ignore rest of the line
                break;
            } else {
                cleaned_line.push(c);
            }
        }

        if !in_block_comment {
            result.push_str(&cleaned_line);
            result.push('\n');
        }
    }
    result
}

// Word boundary check to avoid false positives (e.g. matching "Arc" in "ConversationArchived")
fn contains_word(text: &str, word: &str) -> bool {
    let mut pos = 0;
    while let Some(start) = text[pos..].find(word) {
        let absolute_start = pos + start;
        let absolute_end = absolute_start + word.len();

        let prev_char = if absolute_start > 0 {
            text.as_bytes()[absolute_start - 1] as char
        } else {
            ' '
        };

        let next_char = if absolute_end < text.len() {
            text.as_bytes()[absolute_end] as char
        } else {
            ' '
        };

        let left_boundary = !prev_char.is_alphanumeric() && prev_char != '_';
        let right_boundary = !next_char.is_alphanumeric() && next_char != '_';

        if left_boundary && right_boundary {
            return true;
        }

        pos = absolute_end;
    }
    false
}

#[test]
fn test_no_violating_cargo_toml_dependencies() {
    let root = get_workspace_root();
    let crates_dir = root.join("crates");

    let entries = fs::read_dir(crates_dir).expect("Failed to read crates dir");
    for entry in entries {
        let entry = entry.expect("Invalid entry");
        if entry.file_type().expect("Failed to get type").is_dir() {
            let crate_path = entry.path();
            let cargo_toml_path = crate_path.join("Cargo.toml");
            if cargo_toml_path.exists() {
                let content =
                    fs::read_to_string(&cargo_toml_path).expect("Failed to read Cargo.toml");
                let crate_name = crate_path.file_name().unwrap().to_str().unwrap();
                let allowed = get_allowed_dependencies(crate_name);

                for line in content.lines() {
                    if line.contains("path =") {
                        let dep_name = line.split('=').next().unwrap().trim();
                        if dep_name != "package" {
                            let is_allowed = allowed.contains(&dep_name) || dep_name == crate_name;
                            assert!(
                                is_allowed,
                                "Architecture Violation: Crate '{}' has disallowed dependency on '{}' in {:?}",
                                crate_name, dep_name, cargo_toml_path
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_domain_crate_purity_and_no_infrastructure_imports() {
    let root = get_workspace_root();
    let domain_src = root.join("crates").join("brain-domain").join("src");

    fn verify_dir(dir: &Path) {
        let entries = fs::read_dir(dir).expect("Failed to read dir");
        for entry in entries {
            let entry = entry.expect("Invalid entry");
            let path = entry.path();
            if path.is_dir() {
                verify_dir(&path);
            } else if path
                .extension()
                .map_or(false, |ext| ext == "rust" || ext == "rs")
            {
                let raw_content = fs::read_to_string(&path).expect("Failed to read source file");
                let content = clean_comments(&raw_content);

                for (line_num, line) in content.lines().enumerate() {
                    let line_stripped = line.trim();
                    if line_stripped.is_empty() {
                        continue;
                    }

                    // 1. Assert no dependencies on other workspace crates
                    let forbidden_imports = [
                        "brain_core",
                        "brain_services",
                        "brain_storage",
                        "brain_plugins",
                        "brain_python",
                        "brain_tui",
                        "brain_events",
                        "brain_observability",
                    ];
                    for forbidden in &forbidden_imports {
                        if contains_word(line_stripped, forbidden) {
                            panic!(
                                "Architecture Violation: Forbidden import/reference to workspace crate '{}' found in {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }

                    // 2. Assert no synchronization primitives
                    let forbidden_sync = [
                        "Mutex",
                        "RwLock",
                        "Arc",
                        "AtomicBool",
                        "AtomicU64",
                        "AtomicI32",
                    ];
                    for forbidden in &forbidden_sync {
                        if contains_word(line_stripped, forbidden) {
                            panic!(
                                "Architecture Violation: Forbidden sync primitive '{}' found in domain layer at {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }

                    // 3. Assert no Tokio runtime references
                    let forbidden_runtime = ["tokio", "spawn_blocking", "async_trait"];
                    for forbidden in &forbidden_runtime {
                        if contains_word(line_stripped, forbidden) {
                            panic!(
                                "Architecture Violation: Forbidden async/Tokio primitive '{}' found in domain layer at {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }
                    if contains_word(line_stripped, "spawn") && !line_stripped.contains(".spawn") {
                        panic!(
                            "Architecture Violation: Forbidden async/Tokio primitive 'spawn' found in domain layer at {:?}:{}",
                            path, line_num + 1
                        );
                    }

                    // 4. Assert no logging/tracing/println leakage
                    let forbidden_logging = ["tracing", "log", "println!"];
                    for forbidden in &forbidden_logging {
                        if contains_word(line_stripped, forbidden) {
                            panic!(
                                "Architecture Violation: Forbidden logging/tracing call '{}' found in domain layer at {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }

                    // 5. Assert no filesystem access
                    let forbidden_fs = ["std::fs", "tokio::fs", "File::open", "File::create"];
                    for forbidden in &forbidden_fs {
                        if line_stripped.contains(forbidden) {
                            panic!(
                                "Architecture Violation: Forbidden filesystem access '{}' found in domain layer at {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }

                    // 6. Assert no database/repository leakage
                    let forbidden_storage = [
                        "Sqlite",
                        "rusqlite",
                        "DuckDb",
                        "r2d2",
                        "Connection",
                        "Repository",
                    ];
                    for forbidden in &forbidden_storage {
                        // Allow DomainError because it matches the word DomainError which contains "Error"
                        if contains_word(line_stripped, forbidden)
                            && !line_stripped.contains("DomainError")
                        {
                            panic!(
                                "Architecture Violation: Database/Repository type reference '{}' found in domain layer at {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }
                }
            }
        }
    }

    verify_dir(&domain_src);
}

#[test]
fn test_core_crate_no_upward_infrastructure_imports() {
    let root = get_workspace_root();
    let core_src = root.join("crates").join("brain-core").join("src");

    fn verify_dir(dir: &Path) {
        let entries = fs::read_dir(dir).expect("Failed to read dir");
        for entry in entries {
            let entry = entry.expect("Invalid entry");
            let path = entry.path();
            if path.is_dir() {
                verify_dir(&path);
            } else if path
                .extension()
                .map_or(false, |ext| ext == "rust" || ext == "rs")
            {
                let raw_content = fs::read_to_string(&path).expect("Failed to read source file");
                let content = clean_comments(&raw_content);

                for (line_num, line) in content.lines().enumerate() {
                    let line_stripped = line.trim();
                    if line_stripped.is_empty() {
                        continue;
                    }

                    let forbidden_imports = [
                        "brain_services",
                        "brain_plugins",
                        "brain_python",
                        "brain_tui",
                        "brain_observability",
                    ];
                    for forbidden in &forbidden_imports {
                        if contains_word(line_stripped, forbidden) {
                            panic!(
                                "Architecture Violation: Forbidden upward import '{}' found in {:?}:{}",
                                forbidden, path, line_num + 1
                            );
                        }
                    }
                }
            }
        }
    }

    verify_dir(&core_src);
}

#[test]
fn test_event_publishing_boundary_enforcement() {
    let root = get_workspace_root();
    let crates_dir = root.join("crates");

    let entries = fs::read_dir(crates_dir).expect("Failed to read crates dir");
    for entry in entries {
        let entry = entry.expect("Invalid entry");
        let path = entry.path();
        let crate_name = path.file_name().unwrap().to_str().unwrap();

        if crate_name == "brain-services"
            || crate_name == "brain-events"
            || crate_name == "brain-observability"
        {
            continue;
        }

        let src_dir = path.join("src");
        if src_dir.exists() {
            fn verify_no_event_envelope(dir: &Path, crate_name: &str) {
                let entries = fs::read_dir(dir).expect("Failed to read dir");
                for entry in entries {
                    let entry = entry.expect("Invalid entry");
                    let path = entry.path();
                    if path.is_dir() {
                        verify_no_event_envelope(&path, crate_name);
                    } else if path
                        .extension()
                        .map_or(false, |ext| ext == "rust" || ext == "rs")
                    {
                        let raw_content =
                            fs::read_to_string(&path).expect("Failed to read source file");
                        let content = clean_comments(&raw_content);

                        for (line_num, line) in content.lines().enumerate() {
                            let line_stripped = line.trim();
                            if line_stripped.contains("EventEnvelope")
                                && line_stripped.contains(".publish(")
                            {
                                panic!(
                                    "Architecture Violation: Crate '{}' publishes EventEnvelope directly. Only brain-services should act as publishing boundaries. Found in {:?}:{}",
                                    crate_name, path, line_num + 1
                                );
                            }
                        }
                    }
                }
            }
            verify_no_event_envelope(&src_dir, crate_name);
        }
    }
}
