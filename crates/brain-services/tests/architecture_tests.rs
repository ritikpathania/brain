use std::fs;
use std::path::{Path, PathBuf};

fn get_workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = Path::new(&manifest_dir);
    // CARGO_MANIFEST_DIR is crates/brain-services
    manifest_path
        .parent()
        .expect("Missing crates dir")
        .parent()
        .expect("Missing workspace root")
        .to_path_buf()
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

fn extract_impl_blocks(content: &str, trait_name: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let pattern = format!("impl {} for", trait_name);
    let mut pos = 0;
    while let Some(start) = content[pos..].find(&pattern) {
        let abs_start = pos + start;
        if let Some(brace_start) = content[abs_start..].find('{') {
            let abs_brace_start = abs_start + brace_start;
            let mut brace_count = 1;
            let mut current = abs_brace_start + 1;
            let bytes = content.as_bytes();
            while current < bytes.len() && brace_count > 0 {
                if bytes[current] == b'{' {
                    brace_count += 1;
                } else if bytes[current] == b'}' {
                    brace_count -= 1;
                }
                current += 1;
            }
            if brace_count == 0 {
                blocks.push(content[abs_brace_start..current].to_string());
            }
            pos = current;
        } else {
            pos = abs_start + pattern.len();
        }
    }
    blocks
}

#[test]
fn test_storage_cannot_depend_on_traversal() {
    let root = get_workspace_root();
    let storage_src = root.join("crates").join("brain-storage").join("src");

    fn verify_dir(dir: &Path) {
        let entries = fs::read_dir(dir).expect("Failed to read dir");
        for entry in entries {
            let entry = entry.expect("Invalid entry");
            let path = entry.path();
            if path.is_dir() {
                verify_dir(&path);
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                let raw = fs::read_to_string(&path).unwrap();
                let clean = clean_comments(&raw);
                assert!(
                    !contains_word(&clean, "Graph"),
                    "Architecture Violation: brain-storage refers to Graph in {:?}",
                    path
                );
                assert!(
                    !contains_word(&clean, "TraversalBudget"),
                    "Architecture Violation: brain-storage refers to TraversalBudget in {:?}",
                    path
                );
            }
        }
    }
    verify_dir(&storage_src);
}

#[test]
fn test_domain_cannot_depend_on_services_or_retrieval() {
    let root = get_workspace_root();
    let domain_src = root.join("crates").join("brain-domain").join("src");

    fn verify_dir(dir: &Path) {
        let entries = fs::read_dir(dir).expect("Failed to read dir");
        for entry in entries {
            let entry = entry.expect("Invalid entry");
            let path = entry.path();
            if path.is_dir() {
                verify_dir(&path);
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                let raw = fs::read_to_string(&path).unwrap();
                let clean = clean_comments(&raw);
                let forbidden = [
                    "brain_services",
                    "Graph",
                    "RetrievalService",
                    "RetrievalServiceImpl",
                ];
                for f in &forbidden {
                    assert!(
                        !contains_word(&clean, f),
                        "Architecture Violation: brain-domain refers to {} in {:?}",
                        f,
                        path
                    );
                }
            }
        }
    }
    verify_dir(&domain_src);
}

#[test]
fn test_traversal_cannot_depend_on_extractor_impls() {
    let root = get_workspace_root();
    let traversal_file = root
        .join("crates")
        .join("brain-services")
        .join("src")
        .join("retrieval")
        .join("graph_service.rs");
    if traversal_file.exists() {
        let raw = fs::read_to_string(&traversal_file).unwrap();
        let clean = clean_comments(&raw);
        let forbidden = [
            "DummyMemoryExtractor",
            "MockMemoryExtractor",
            "PythonMemoryExtractor",
            "BuiltinPythonExtractor",
        ];
        for f in &forbidden {
            assert!(
                !contains_word(&clean, f),
                "Architecture Violation: Graph refers to extractor implementation {} in {:?}",
                f,
                traversal_file
            );
        }
    }
}

#[test]
fn test_traversal_and_analyzer_perform_no_writes() {
    let root = get_workspace_root();
    let graph_service_file = root
        .join("crates")
        .join("brain-services")
        .join("src")
        .join("retrieval")
        .join("graph_service.rs");
    if graph_service_file.exists() {
        let raw = fs::read_to_string(&graph_service_file).unwrap();
        let clean = clean_comments(&raw);
        let forbidden = [
            ".save(",
            ".save_batch(",
            ".delete(",
            ".save_session(",
            ".delete_session(",
        ];
        for f in &forbidden {
            assert!(
                !clean.contains(f),
                "Behavioral Violation: Traversal/Analyzer writes to repository with '{}' in {:?}",
                f,
                graph_service_file
            );
        }
    }
}

#[test]
fn test_extractor_impls_never_invoke_storage_directly() {
    let root = get_workspace_root();
    let services_src = root.join("crates").join("brain-services").join("src");
    let tests_src = root.join("crates").join("brain-services").join("tests");

    let verify_extractors_in_dir = |dir: &Path| {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current_dir) = stack.pop() {
            if !current_dir.exists() {
                continue;
            }
            let entries = fs::read_dir(current_dir).expect("Failed to read dir");
            for entry in entries {
                let entry = entry.expect("Invalid entry");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |ext| ext == "rs") {
                    let raw = fs::read_to_string(&path).unwrap();
                    let clean = clean_comments(&raw);
                    let blocks = extract_impl_blocks(&clean, "MemoryExtractor");
                    for block in blocks {
                        let forbidden = [
                            "SqliteStorage",
                            "SqliteConnection",
                            "run_transaction",
                            "RepositorySet",
                        ];
                        for f in &forbidden {
                            assert!(
                                !contains_word(&block, f),
                                "Behavioral Violation: MemoryExtractor implementation uses storage type '{}' in {:?}",
                                f,
                                path
                            );
                        }
                    }
                }
            }
        }
    };
    verify_extractors_in_dir(&services_src);
    verify_extractors_in_dir(&tests_src);
}

#[test]
fn test_repository_impls_never_invoke_extraction() {
    let root = get_workspace_root();
    let storage_src = root.join("crates").join("brain-storage").join("src");

    fn verify_dir(dir: &Path) {
        let entries = fs::read_dir(dir).expect("Failed to read dir");
        for entry in entries {
            let entry = entry.expect("Invalid entry");
            let path = entry.path();
            if path.is_dir() {
                verify_dir(&path);
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                let raw = fs::read_to_string(&path).unwrap();
                let clean = clean_comments(&raw);

                // Extract repository impl blocks
                let repo_traits = [
                    "NodeRepository",
                    "EdgeRepository",
                    "EmbeddingRepository",
                    "SessionRepository",
                ];
                for r_trait in &repo_traits {
                    let blocks = extract_impl_blocks(&clean, r_trait);
                    for block in blocks {
                        let forbidden = [
                            "MemoryExtractor",
                            "extract",
                            "ExtractionRequest",
                            "ExtractionResult",
                        ];
                        for f in &forbidden {
                            assert!(
                                !contains_word(&block, f),
                                "Behavioral Violation: Repository implementation of {} invokes extraction via '{}' in {:?}",
                                r_trait,
                                f,
                                path
                            );
                        }
                    }
                }
            }
        }
    }
    verify_dir(&storage_src);
}
