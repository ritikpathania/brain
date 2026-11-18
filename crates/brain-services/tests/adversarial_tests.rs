use brain_core::agents::ExtractionAgent;
use brain_core::services::{ExtractionRequest, MemoryExtractor};
use brain_domain::KnowledgeGraph;
use brain_services::extraction::heuristics::HeuristicExtractor;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Test 1: Massive string (150k+ chars) stress test to ensure no OOM or stack overflow
#[test]
fn test_massive_string_stress() {
    let extractor = HeuristicExtractor::new();
    let base =
        "Rust is a systems language. Python depends on Docker. SQLite is stored in config.toml. ";
    let massive_text = base.repeat(2000); // ~170k characters
    assert!(massive_text.len() > 150_000);

    let (nodes, edges) = extractor
        .extract_graph(&massive_text)
        .expect("massive text extract_graph must succeed");
    assert!(!nodes.is_empty());
    assert!(!edges.is_empty());

    // Verify MemoryExtractor on massive text
    let req = ExtractionRequest {
        raw_content: massive_text.clone(),
        context_metadata: HashMap::new(),
    };
    let result = extractor
        .extract(req)
        .expect("massive text extract must succeed");
    assert!(!result.nodes.is_empty());
    assert!(!result.edges.is_empty());

    // Verify JSON FFI entrypoint
    let json_payload = serde_json::json!([
        {
            "id": "node-massive",
            "content": massive_text,
        }
    ])
    .to_string();
    let res_json = extractor
        .extract_semantic_nodes_json(&json_payload)
        .expect("extract_semantic_nodes_json must succeed");
    let val: Value = serde_json::from_str(&res_json).expect("valid json");
    assert_eq!(val["status"], "success");
}

/// Test 2: Multi-byte Unicode, Emojis, RTL, and Special Whitespace
#[test]
fn test_multibyte_unicode_and_emojis() {
    let extractor = HeuristicExtractor::new();
    let test_cases = [
        // 4-byte emojis
        "🦀 Rust is an open-source programming language. 🚀 Docker runs on 🐧 Linux.",
        // CJK characters
        "北京大学 開発者 データベース SQLite を使用します。Python は素晴らしい言語です。",
        // Mixed RTL & LTR
        "مرحبا بالعالم Rust uses Python في النظام. SQLite is used by John.",
        // Combining diacritics and accents
        "Café Münchhausen Übergröße élève e\u{0301}a\u{0300}u\u{0302}",
        // Unusual unicode whitespace separating sentences
        "Rust is great.\u{2003}\u{2002}\u{2009}Python is cool.\u{205F}Docker is fast.",
        // Unicode punctuation
        "Rust is fast。Python is friendly！Docker is light？",
    ];

    for case in &test_cases {
        let (_nodes, edges) = extractor
            .extract_graph(case)
            .unwrap_or_else(|e| panic!("Failed on case '{}': {:?}", case, e));

        let json_payload = serde_json::json!([{"content": case}]).to_string();
        let res_json = extractor
            .extract_semantic_nodes_json(&json_payload)
            .unwrap_or_else(|e| panic!("JSON failed on case '{}': {:?}", case, e));
        let val: Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(val["status"], "success");

        // Verify no self-loops
        for edge in &edges {
            assert_ne!(
                edge.source, edge.target,
                "Self-loop detected in unicode case: {:?}",
                edge
            );
        }
    }
}

/// Test 3: Cycle Stress Test ("A B A B A") and Self-Referential Statements
#[test]
fn test_cycles_and_self_referential_statements() {
    let extractor = HeuristicExtractor::new();
    let text =
        "Rust Python Rust Python Rust. Brain is Brain. Python uses Python. Docker runs on Docker.";
    let (_nodes, edges) = extractor.extract_graph(text).expect("extract graph");

    // 1. Strictly NO self-loops allowed
    for edge in &edges {
        assert_ne!(
            edge.source, edge.target,
            "Self-loop detected: source '{}' == target '{}'",
            edge.source, edge.target
        );
    }

    // 2. Edge deduplication: no duplicate (source, target, relation)
    let mut seen = HashSet::new();
    for edge in &edges {
        let key = (&edge.source, &edge.target, &edge.relation);
        assert!(seen.insert(key), "Duplicate edge detected: {:?}", key);
    }
}

/// Test 4: Extreme Malformed JSON Inputs to `extract_semantic_nodes_json`
#[test]
fn test_malformed_json_inputs() {
    let extractor = HeuristicExtractor::new();
    let malformed_inputs = [
        "",
        "   ",
        "{not valid json}",
        "12345",
        "true",
        "false",
        "null",
        "\"string\"",
        "[1, 2, 3]",
        "{}",
        "[{\"content\": 12345}]",
        "[{\"wrong_field\": \"hello\"}]",
        "[{\"id\": null, \"content\": \"\"}]",
    ];

    for input in &malformed_inputs {
        let res = extractor.extract_semantic_nodes_json(input);
        assert!(
            res.is_ok(),
            "Must return Ok(JSON) even on malformed input: '{}'",
            input
        );
        let val: Value = serde_json::from_str(&res.unwrap()).expect("must return parseable json");
        let status = val
            .get("status")
            .and_then(|s| s.as_str())
            .expect("status exists");
        assert!(
            status == "success" || status == "error",
            "Status must be success or error, got {}",
            status
        );
    }
}

/// Test 5: Deterministic UUID Stability on Repeated Extraction
#[test]
fn test_deterministic_uuid_stability() {
    let extractor = HeuristicExtractor::new();
    let content = "Alice develops Brain. Brain runs on Docker. SQLite is stored in config.toml.";

    let req1 = ExtractionRequest {
        raw_content: content.to_string(),
        context_metadata: [("conversation_id".to_string(), "conv-1".to_string())]
            .into_iter()
            .collect(),
    };
    let req2 = ExtractionRequest {
        raw_content: content.to_string(),
        context_metadata: [("conversation_id".to_string(), "conv-1".to_string())]
            .into_iter()
            .collect(),
    };

    let res1 = extractor.extract(req1).expect("res1 succeeds");
    let res2 = extractor.extract(req2).expect("res2 succeeds");

    assert_eq!(res1.nodes.len(), res2.nodes.len());
    assert_eq!(res1.edges.len(), res2.edges.len());

    for (n1, n2) in res1.nodes.iter().zip(res2.nodes.iter()) {
        assert_eq!(
            n1.id, n2.id,
            "Node IDs must be identical across repeated extractions"
        );
        assert_eq!(n1.label, n2.label);
        assert_eq!(n1.node_type, n2.node_type);
    }

    for (e1, e2) in res1.edges.iter().zip(res2.edges.iter()) {
        assert_eq!(
            e1.source, e2.source,
            "Edge source must match across repeated extractions"
        );
        assert_eq!(
            e1.target, e2.target,
            "Edge target must match across repeated extractions"
        );
        assert_eq!(e1.relation, e2.relation);
    }
}

/// Test 6: Referential Integrity Invariant
/// Verifies that when classification text contains a synonym or specific entity,
/// all emitted edges strictly reference valid nodes in the extracted node list (no dangling edges).
#[test]
fn test_referential_integrity_preserved_in_classification_overlap() {
    let extractor = HeuristicExtractor::new();
    let text = "Brain is an Open Source SQLite Database.";
    let (nodes, edges) = extractor.extract_graph(text).expect("extract_graph");
    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    assert!(!nodes.is_empty(), "Extracted nodes must not be empty");
    assert!(!edges.is_empty(), "Extracted edges must not be empty");

    let dangling_edges: Vec<_> = edges
        .iter()
        .filter(|e| !node_ids.contains(&e.source) || !node_ids.contains(&e.target))
        .collect();

    assert!(
        dangling_edges.is_empty(),
        "Referential integrity violated: dangling edges exist referencing nodes not present in node list: {:?}",
        dangling_edges
    );

    assert!(node_ids.contains("brain"), "Expected node 'brain'");
    assert!(node_ids.contains("sqlite"), "Expected node 'sqlite'");
    for edge in &edges {
        assert!(
            node_ids.contains(&edge.source),
            "Source '{}' not in nodes",
            edge.source
        );
        assert!(
            node_ids.contains(&edge.target),
            "Target '{}' not in nodes",
            edge.target
        );
    }
}

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Verifies that for arbitrary combinations of adjectives, synonyms, and nouns in classification
    /// clauses, all emitted edges strictly satisfy referential integrity (zero dangling edges).
    #[test]
    fn prop_classification_overlap_preserves_referential_integrity(
        syn in proptest::sample::select(vec!["SQLite", "Docker", "Python", "Rust", "DuckDB", "PostgreSQL", "Bun"]),
        adj in proptest::sample::select(vec!["Fast", "Modern", "Distributed", "Scalable", "Robust"]),
        noun in proptest::sample::select(vec!["Engine", "Database", "System", "Framework", "Platform"])
    ) {
        let extractor = HeuristicExtractor::new();
        let text = format!("Foo is a {} {} {}.", adj, syn, noun);
        let (nodes, edges) = extractor.extract_graph(&text).unwrap();
        let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

        for edge in edges {
            prop_assert!(
                node_ids.contains(&edge.source),
                "Edge source '{}' must exist in nodes: {:?}",
                edge.source,
                node_ids
            );
            prop_assert!(
                node_ids.contains(&edge.target),
                "Edge target '{}' must exist in nodes: {:?}",
                edge.target,
                node_ids
            );
        }
    }
}

/// Test 8: KnowledgeGraph Ingestion Referential Integrity
/// Verifies that ExtractionResult from MemoryExtractor::extract can be successfully ingested
/// into brain_domain::KnowledgeGraph without DomainError::MissingTargetNode or MissingSourceNode.
#[test]
fn test_knowledge_graph_ingestion_succeeds_on_classification_overlap() {
    use brain_domain::KnowledgeGraph;

    let extractor = HeuristicExtractor::new();
    let req = ExtractionRequest {
        raw_content: "Brain is an Open Source SQLite Database.".to_string(),
        context_metadata: HashMap::new(),
    };
    let result = extractor.extract(req).expect("extract succeeds");

    assert!(!result.nodes.is_empty(), "Nodes must not be empty");
    assert!(!result.edges.is_empty(), "Edges must not be empty");

    let mut kg = KnowledgeGraph::new();
    let expected_nodes = result.nodes.len();
    for node in result.nodes {
        kg.add_node(node);
    }
    assert_eq!(kg.nodes.len(), expected_nodes);

    let expected_edges = result.edges.len();
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("KnowledgeGraph::add_edge must succeed without dangling edge errors");
    }
    assert_eq!(kg.edges.len(), expected_edges);
}

/// Test 9: Complex Classification Clauses Referential Integrity
/// Verifies across multiple classification clauses, embedded synonyms in subject/object,
/// parentheticals, and punctuation that all three interfaces (extract_graph, extract,
/// extract_semantic_nodes_json) maintain 100% referential integrity and 0 KnowledgeGraph errors.
#[test]
fn test_complex_classification_clauses_referential_integrity() {
    let extractor = HeuristicExtractor::new();
    let complex_inputs = [
        // Multiple classification clauses in one document
        "Brain is an Open Source SQLite Database. SQLite is a Relational Database Engine. Docker is a Container Virtualization Platform. Rust is a Systems Programming Language.",
        // Subject splitting in classification clause
        "Open Source SQLite Database is an Embedded Database Engine.",
        // Multiple embedded synonyms in classification predicate
        "Brain is an Open Source SQLite Redis Docker Database.",
        // Conjunctions and parentheticals
        "Brain (an Open Source SQLite Database) is fast. Alice uses SQLite in production.",
        // Hyphenated and numerical entities
        "System-1 is a v2-compatible PostgreSQL Storage System. App-99 is an S3-compatible Object Store.",
    ];

    for input in &complex_inputs {
        // 1. extract_graph
        let (nodes, edges) = extractor.extract_graph(input).expect("extract_graph");
        let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        for edge in &edges {
            assert!(
                node_ids.contains(&edge.source),
                "extract_graph: source '{}' missing from nodes for input '{}'",
                edge.source,
                input
            );
            assert!(
                node_ids.contains(&edge.target),
                "extract_graph: target '{}' missing from nodes for input '{}'",
                edge.target,
                input
            );
        }

        // 2. MemoryExtractor::extract and KnowledgeGraph ingestion
        let req = ExtractionRequest {
            raw_content: input.to_string(),
            context_metadata: HashMap::new(),
        };
        let result = extractor.extract(req).expect("extract");
        let mut kg = KnowledgeGraph::new();
        for node in result.nodes {
            kg.add_node(node);
        }
        for edge in result.edges {
            kg.add_edge(edge)
                .expect("KnowledgeGraph::add_edge must succeed");
        }

        // 3. extract_semantic_nodes_json
        let json_payload = serde_json::json!([{"content": input}]).to_string();
        let json_res = extractor
            .extract_semantic_nodes_json(&json_payload)
            .expect("extract_semantic_nodes_json");
        let val: Value = serde_json::from_str(&json_res).expect("valid json");
        assert_eq!(val["status"], "success");
        let json_node_ids: HashSet<&str> = val["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .filter_map(|n| n["id"].as_str())
            .collect();
        for edge in val["edges"].as_array().expect("edges array") {
            let src = edge["source"].as_str().expect("source");
            let tgt = edge["target"].as_str().expect("target");
            assert!(
                json_node_ids.contains(src),
                "JSON: source '{}' missing from nodes for input '{}'",
                src,
                input
            );
            assert!(
                json_node_ids.contains(tgt),
                "JSON: target '{}' missing from nodes for input '{}'",
                tgt,
                input
            );
        }
    }
}

/// Test 10: Massive String Stress Test (200k+ characters)
#[test]
fn test_massive_string_200k_chars() {
    let extractor = HeuristicExtractor::new();
    let base = "Brain is an Open Source SQLite Database. Alice uses SQLite. Bob writes Rust. Docker runs on Linux. PostgreSQL is stored in config.toml. ";
    let massive_text = base.repeat(1500); // ~207k chars
    assert!(massive_text.len() > 200_000);

    let start = std::time::Instant::now();
    let (nodes, edges) = extractor
        .extract_graph(&massive_text)
        .expect("extract_graph on 200k chars must succeed");
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 10,
        "200k char extraction took too long: {:?}",
        elapsed
    );
    assert!(!nodes.is_empty());
    assert!(!edges.is_empty());

    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    for edge in &edges {
        assert!(node_ids.contains(&edge.source));
        assert!(node_ids.contains(&edge.target));
    }

    let req = ExtractionRequest {
        raw_content: massive_text,
        context_metadata: HashMap::new(),
    };
    let result = extractor
        .extract(req)
        .expect("MemoryExtractor on 500k chars must succeed");
    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("KnowledgeGraph ingestion on 500k chars must succeed");
    }
}

/// Test 11: Complex Unicode Graphemes, ZWJ sequences, RTL, and Embedded Nulls
#[test]
fn test_complex_unicode_graphemes_and_null_bytes() {
    let extractor = HeuristicExtractor::new();
    let test_cases = [
        // ZWJ Emoji family and technologist
        "👨‍👩‍👧‍👦 Brain is a database for 👩🏾‍💻 engineers using 🦀 Rust. 🏳️‍🌈 Flags: 🇺🇸 🇯🇵 🇩🇪.",
        // Farsi/Persian with ZWNJ (Zero-Width Non-Joiner)
        "نرم‌افزار Brain از SQLite و Docker استفاده می‌کند.",
        // Devanagari script
        "नमस्ते दुनिया! Brain एक Open Source SQLite Database है और Rust का उपयोग करता है।",
        // Embedded null bytes
        "Alice\0uses\0SQLite\0in\0Brain\0on\0Docker\0.",
        // Consecutive exotic unicode spaces
        "Rust\u{00A0}is\u{2000}fast.\u{2001}Python\u{2002}is\u{2003}cool.\u{200B}SQLite\u{3000}is\u{202F}light.",
    ];

    for case in &test_cases {
        let (nodes, edges) = extractor
            .extract_graph(case)
            .unwrap_or_else(|e| panic!("Failed on unicode case '{}': {:?}", case, e));
        let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

        for edge in &edges {
            assert_ne!(edge.source, edge.target, "Self loop in unicode: {:?}", edge);
            assert!(
                node_ids.contains(&edge.source),
                "Source '{}' not in nodes for case '{}'",
                edge.source,
                case
            );
            assert!(
                node_ids.contains(&edge.target),
                "Target '{}' not in nodes for case '{}'",
                edge.target,
                case
            );
        }

        let req = ExtractionRequest {
            raw_content: case.to_string(),
            context_metadata: HashMap::new(),
        };
        let result = extractor.extract(req).expect("extract succeeds");
        let mut kg = KnowledgeGraph::new();
        for node in result.nodes {
            kg.add_node(node);
        }
        for edge in result.edges {
            kg.add_edge(edge).expect("KnowledgeGraph add_edge succeeds");
        }
    }
}

/// Test 12: Topological Cycles, Knots, Dense Cliques, and Self-Loops
#[test]
fn test_topological_cycles_cliques_and_self_loops() {
    let extractor = HeuristicExtractor::new();
    let cycle_cases = [
        // 2-cycle
        "Alice uses Bob. Bob uses Alice.",
        // 3-cycle
        "Alice uses Bob. Bob uses Charlie. Charlie uses Alice.",
        // 5-cycle
        "NodeA uses NodeB. NodeB uses NodeC. NodeC uses NodeD. NodeD uses NodeE. NodeE uses NodeA.",
        // Pure self-references
        "Brain is Brain. Rust is Rust. Docker runs on Docker. Python uses Python.",
        // Dense clique
        "Alice, Bob, Charlie, and Dave use Alice, Bob, Charlie, and Dave.",
    ];

    for case in &cycle_cases {
        let (nodes, edges) = extractor.extract_graph(case).expect("extract_graph");
        let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

        let mut seen = HashSet::new();
        for edge in &edges {
            // Invariant 1: No self-loops
            assert_ne!(
                edge.source, edge.target,
                "Self-loop forbidden: source '{}' == target '{}' in case '{}'",
                edge.source, edge.target, case
            );

            // Invariant 2: No duplicate edge triples
            let triple = (&edge.source, &edge.target, &edge.relation);
            assert!(
                seen.insert(triple),
                "Duplicate edge triple: {:?} in case '{}'",
                triple,
                case
            );

            // Invariant 3: Referential integrity
            assert!(
                node_ids.contains(&edge.source),
                "Source missing from nodes in case '{}'",
                case
            );
            assert!(
                node_ids.contains(&edge.target),
                "Target missing from nodes in case '{}'",
                case
            );
        }

        // Invariant 4: KnowledgeGraph ingestion succeeds
        let req = ExtractionRequest {
            raw_content: case.to_string(),
            context_metadata: HashMap::new(),
        };
        let result = extractor.extract(req).expect("extract succeeds");
        let mut kg = KnowledgeGraph::new();
        for node in result.nodes {
            kg.add_node(node);
        }
        for edge in result.edges {
            kg.add_edge(edge)
                .expect("KnowledgeGraph ingestion must succeed");
        }
    }
}

/// Test 13: Adversarial and Deeply Malformed JSON Inputs
#[test]
fn test_adversarial_and_deeply_malformed_json() {
    let extractor = HeuristicExtractor::new();
    let adversarial_inputs = [
        // Truncated JSON
        r#"[{"id": "node-1", "content": "Brain is a database"#,
        r#"{"nodes": ["#,
        // Primitive literals
        "42",
        "3.14159",
        "true",
        "false",
        "null",
        "\"a simple string\"",
        // Array of non-objects
        r#"[1, "string", true, null, [1, 2], {"unrelated": 42}]"#,
        // Objects with missing or null content
        r#"[{"id": "node-null", "content": null}]"#,
        r#"[{"id": "node-bool", "content": false}]"#,
        r#"[{"id": "node-num", "content": 12345}]"#,
        r#"[{"id": "node-obj", "content": {"text": "nested"}}]"#,
    ];

    for input in &adversarial_inputs {
        let res = extractor.extract_semantic_nodes_json(input);
        assert!(
            res.is_ok(),
            "extract_semantic_nodes_json must not panic or error out on malformed input"
        );
        let val: Value =
            serde_json::from_str(&res.unwrap()).expect("output must always be valid JSON");
        let status = val
            .get("status")
            .and_then(|s| s.as_str())
            .expect("status field must exist");
        assert!(status == "success" || status == "error");
    }
}

/// Test 14: Multi-Threaded Concurrent Deterministic UUID Stability
#[test]
fn test_concurrent_deterministic_uuid_multithreaded() {
    use std::sync::Arc;
    let extractor = Arc::new(HeuristicExtractor::new());
    let mut handles = Vec::new();

    for thread_idx in 0..16 {
        let ext = Arc::clone(&extractor);
        handles.push(std::thread::spawn(move || {
            for i in 0..20 {
                let text =
                    "Alice develops Brain. Brain runs on Docker. SQLite is stored in config.toml.";
                let req = ExtractionRequest {
                    raw_content: text.to_string(),
                    context_metadata: [
                        (
                            "conversation_id".to_string(),
                            format!("conv-{}", thread_idx),
                        ),
                        ("iteration".to_string(), i.to_string()),
                    ]
                    .into_iter()
                    .collect(),
                };
                let res = ext.extract(req).expect("extract succeeds");
                assert_eq!(res.nodes.len(), 5);
                assert!(!res.edges.is_empty());

                for node in &res.nodes {
                    let bytes = node.id.0.as_bytes();
                    // Version 5 check: bits 4..7 of byte 6
                    assert_eq!((bytes[6] >> 4), 5, "UUID must be version 5");
                    // Variant RFC 4122 check: bits 6..7 of byte 8 == 0b10
                    assert_eq!((bytes[8] >> 6), 2, "UUID must be RFC 4122 variant");
                }

                // Verify referential integrity within thread
                let node_id_set: HashSet<_> = res.nodes.iter().map(|n| n.id).collect();
                for edge in &res.edges {
                    assert!(node_id_set.contains(&edge.source));
                    assert!(node_id_set.contains(&edge.target));
                }
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread must not panic");
    }
}
