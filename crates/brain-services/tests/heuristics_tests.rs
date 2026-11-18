//! Native unit and property tests for `HeuristicExtractor` verifying 100% behavioral
//! parity against Python oracle `daemon/brain/extraction/heuristics.py` and `daemon/tests/test_extractor.py`.

use std::collections::{HashMap, HashSet};

use brain_core::agents::ExtractionAgent;
use brain_core::services::{ExtractionRequest, MemoryExtractor};
use brain_domain::{NodeId, RelationKind};
use brain_services::extraction::heuristics::HeuristicExtractor;
use proptest::prelude::*;
use serde_json::Value;

// ============================================================================
// Parity Unit Tests (16 tests matching daemon/tests/test_extractor.py)
// ============================================================================

#[test]
fn test_empty_input() {
    let extractor = HeuristicExtractor::new();
    let json_res = extractor
        .extract_semantic_nodes_json("[]")
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    assert_eq!(res["nodes"].as_array().expect("array").len(), 0);
    assert_eq!(res["edges"].as_array().expect("array").len(), 0);
}

#[test]
fn test_tech_and_database_config_extraction() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-1", "epoch": 0, "content": "setting up sqlite database configuration", "timestamp": 1000}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    let node_ids: Vec<&str> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|n| n["id"].as_str().expect("string id"))
        .collect();
    assert!(node_ids.contains(&"sqlite"));
    assert!(node_ids.contains(&"db-config"));

    let edges = res["edges"].as_array().expect("edges array");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["source"], "db-config");
    assert_eq!(edges[0]["target"], "sqlite");
    assert_eq!(edges[0]["relation"], "configures");
}

#[test]
fn test_credential_extraction() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-2", "epoch": 0, "content": "the API keys should be stored in environment variables", "timestamp": 1001}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    let node_ids: Vec<&str> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|n| n["id"].as_str().expect("string id"))
        .collect();
    assert!(node_ids.contains(&"api-key"));
    assert!(node_ids.contains(&"env-vars"));

    let edges = res["edges"].as_array().expect("edges array");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["source"], "api-key");
    assert_eq!(edges[0]["target"], "env-vars");
    assert_eq!(edges[0]["relation"], "stored_in");
}

#[test]
fn test_extractor_robustness_random_strings() {
    let extractor = HeuristicExtractor::new();
    let test_inputs = [
        "",
        "not a json",
        "{\"foo\": \"bar\"}",
        "[1, 2, 3]",
        "[]",
        r#"[{"id": "1", "content": "random text !@#$%"}]"#,
    ];
    for input in test_inputs {
        let res_str = extractor
            .extract_semantic_nodes_json(input)
            .expect("always returns Ok with JSON payload");
        let res: Value = serde_json::from_str(&res_str).expect("valid json response");
        assert!(res.get("status").is_some());
    }
}

#[test]
fn test_extractor_robustness_valid_json_structure() {
    let extractor = HeuristicExtractor::new();
    let payload = serde_json::json!([
        {
            "id": "node-fuzz",
            "epoch": 1,
            "content": "Rust Bun Docker Python",
            "timestamp": 12345
        }
    ])
    .to_string();
    let res_str = extractor
        .extract_semantic_nodes_json(&payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&res_str).expect("valid json response");
    assert_eq!(res["status"], "success");
    assert!(res.get("nodes").is_some());
    assert!(res.get("edges").is_some());
}

#[test]
fn test_classification_and_property_extraction() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-3", "epoch": 0, "content": "Brain is an AI agent engine. DuckDB is a SQL database engine.", "timestamp": 1002}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    let nodes_dict: HashMap<&str, &Value> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|n| (n["id"].as_str().expect("string id"), n))
        .collect();

    assert!(nodes_dict.contains_key("brain"));
    assert!(nodes_dict.contains_key("duckdb"));

    // Verify attributes (category, type)
    assert_eq!(
        nodes_dict["brain"]["attributes"]
            .get("category")
            .expect("category attribute"),
        "AI agent engine"
    );
    assert_eq!(
        nodes_dict["brain"]["attributes"]
            .get("type")
            .expect("type attribute"),
        "AI agent engine"
    );
    assert_eq!(
        nodes_dict["duckdb"]["attributes"]
            .get("category")
            .expect("category attribute"),
        "SQL database engine"
    );
    assert_eq!(
        nodes_dict["duckdb"]["attributes"]
            .get("type")
            .expect("type attribute"),
        "SQL database engine"
    );

    // Verify concept nodes were extracted
    assert!(nodes_dict.contains_key("ai-agent-engine"));
    assert!(nodes_dict.contains_key("sql-database-engine"));

    // Verify edges were created
    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();
    assert!(edges.contains(&("brain", "ai-agent-engine", "associated_with")));
    assert!(edges.contains(&("duckdb", "sql-database-engine", "associated_with")));
}

#[test]
fn test_relation_sentence_parsing() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-4", "epoch": 0, "content": "ritikpathania develops Brain. SQLite is stored in config.toml. Brain runs on Docker. Brain depends on Python.", "timestamp": 1003}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();

    // Check relation: develops (ritikpathania -> brain)
    assert!(edges.contains(&("ritikpathania", "brain", "develops")));
    // Check relation: stored_in (sqlite -> config.toml)
    assert!(edges.contains(&("sqlite", "config.toml", "stored_in")));
    // Check relation: runs_on (brain -> docker)
    assert!(edges.contains(&("brain", "docker", "runs_on")));
    // Check relation: depends_on (brain -> python)
    assert!(edges.contains(&("brain", "python", "depends_on")));
}

#[test]
fn test_synonym_normalization_and_merging() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-5", "epoch": 0, "content": "We are using Postgres on the server. John was configuring PostgreSQL for the app.", "timestamp": 1004}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    let nodes_dict: HashMap<&str, &Value> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|n| (n["id"].as_str().expect("string id"), n))
        .collect();

    assert!(nodes_dict.contains_key("postgresql"));
    assert_eq!(nodes_dict["postgresql"]["label"], "PostgreSQL");
    assert_eq!(nodes_dict["postgresql"]["type"], "technology");

    // Should only have one 'postgresql' node (merged)
    let postgres_nodes: Vec<&Value> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .filter(|n| n["id"] == "postgresql")
        .collect();
    assert_eq!(postgres_nodes.len(), 1);
}

#[test]
fn test_sequential_associated_with_reproduction() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-1", "epoch": 0, "content": "We are migrating our services from Python to Rust.", "timestamp": 1005}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();

    assert!(edges.contains(&("python", "rust", "associated_with")));
    // No self-loops
    assert!(!edges.contains(&("python", "python", "associated_with")));
    assert!(!edges.contains(&("rust", "rust", "associated_with")));
}

#[test]
fn test_sequential_associated_with_multiple() {
    let extractor = HeuristicExtractor::new();
    let payload =
        r#"[{"id": "node-reprod-2", "epoch": 0, "content": "Python Rust Go", "timestamp": 1006}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();

    assert!(edges.contains(&("python", "rust", "associated_with")));
    assert!(edges.contains(&("rust", "go", "associated_with")));
    assert_eq!(edges.len(), 2);
}

#[test]
fn test_sequential_associated_with_single() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-3", "epoch": 0, "content": "Rust", "timestamp": 1007}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();
    assert_eq!(edges.len(), 0);
}

#[test]
fn test_sequential_associated_with_duplicate_cycle() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-4", "epoch": 0, "content": "Rust Python Rust", "timestamp": 1008}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();

    assert!(edges.contains(&("rust", "python", "associated_with")));
    assert!(edges.contains(&("python", "rust", "associated_with")));
    assert!(!edges.contains(&("rust", "rust", "associated_with")));
    assert!(!edges.contains(&("python", "python", "associated_with")));
}

#[test]
fn test_predefined_relation_precedence() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-5", "epoch": 0, "content": "The Rust application communicates via a Unix Domain Socket.", "timestamp": 1009}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();

    assert!(edges.contains(&("rust", "uds", "communicates_via")));
    assert!(!edges.contains(&("rust", "uds", "associated_with")));
}

#[test]
fn test_multi_word_proper_noun_regression() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-6", "epoch": 0, "content": "Visual Studio Code Rust", "timestamp": 1010}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let nodes_dict: HashMap<&str, &Value> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|n| (n["id"].as_str().expect("string id"), n))
        .collect();
    assert!(nodes_dict.contains_key("visual-studio-code"));
    assert!(nodes_dict.contains_key("rust"));

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();
    assert!(edges.contains(&("visual-studio-code", "rust", "associated_with")));
}

#[test]
fn test_multi_word_proper_noun_regression_vsc() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-7", "epoch": 0, "content": "The project uses Visual Studio Code.", "timestamp": 1011}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let nodes_dict: HashMap<&str, &Value> = res["nodes"]
        .as_array()
        .expect("array")
        .iter()
        .map(|n| (n["id"].as_str().expect("string id"), n))
        .collect();

    // Visual Studio Code should be extracted exactly as a single proper noun concept
    assert!(nodes_dict.contains_key("visual-studio-code"));
    assert_eq!(
        nodes_dict["visual-studio-code"]["label"],
        "Visual Studio Code"
    );
    assert_eq!(nodes_dict["visual-studio-code"]["type"], "concept");

    // Ensure no duplicate nodes or fragment concepts were emitted
    assert!(!nodes_dict.contains_key("visual"));
    assert!(!nodes_dict.contains_key("studio"));
    assert!(!nodes_dict.contains_key("code"));
}

#[test]
fn test_predefined_relation_dominance_suppresses_fallback() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-reprod-8", "epoch": 0, "content": "The Rust application communicates via a Unix Domain Socket. The Unix Domain Socket Rust is fast.", "timestamp": 1012}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    let edges: Vec<(&str, &str, &str)> = res["edges"]
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            (
                e["source"].as_str().expect("source"),
                e["target"].as_str().expect("target"),
                e["relation"].as_str().expect("relation"),
            )
        })
        .collect();

    // We should have the predefined communicates_via edge
    assert!(edges.contains(&("rust", "uds", "communicates_via")));
    // We should NOT have a redundant fallback associated_with edge in the opposite direction
    assert!(!edges.contains(&("uds", "rust", "associated_with")));
    // Exactly one edge should be present between this pair
    assert_eq!(edges.len(), 1);
}

#[test]
fn test_classification_overlap_referential_integrity() {
    let extractor = HeuristicExtractor::new();
    let payload = r#"[{"id": "node-overlap", "epoch": 0, "content": "Brain is an Open Source SQLite Database.", "timestamp": 1013}]"#;
    let json_res = extractor
        .extract_semantic_nodes_json(payload)
        .expect("valid extraction");
    let res: Value = serde_json::from_str(&json_res).expect("valid json");

    assert_eq!(res["status"], "success");
    let node_ids: HashSet<&str> = res["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .map(|n| n["id"].as_str().expect("string id"))
        .collect();

    let edges = res["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty(), "Valid edges should be extracted");

    for edge in edges {
        let src = edge["source"].as_str().expect("source");
        let tgt = edge["target"].as_str().expect("target");
        assert!(
            node_ids.contains(src),
            "Edge source '{}' must exist in nodes: {:?}",
            src,
            node_ids
        );
        assert!(
            node_ids.contains(tgt),
            "Edge target '{}' must exist in nodes: {:?}",
            tgt,
            node_ids
        );
    }

    assert!(!node_ids.contains("open-source-sqlite-database"));
    assert!(node_ids.contains("brain"));
    assert!(node_ids.contains("sqlite"));
}

#[test]
fn test_knowledge_graph_ingestion_after_classification_overlap() {
    use brain_domain::KnowledgeGraph;

    let extractor = HeuristicExtractor::new();
    let req = ExtractionRequest {
        raw_content: "Brain is an Open Source SQLite Database.".to_string(),
        context_metadata: HashMap::new(),
    };
    let result = extractor.extract(req).expect("extract succeeds");

    assert!(!result.nodes.is_empty());
    assert!(!result.edges.is_empty());

    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }

    for edge in result.edges {
        kg.add_edge(edge)
            .expect("KnowledgeGraph::add_edge must succeed without domain errors");
    }
}

// ============================================================================
// Direct Trait Contract Tests (ExtractionAgent & MemoryExtractor)
// ============================================================================

#[test]
fn test_extraction_agent_trait() {
    let extractor = HeuristicExtractor::new();
    assert_eq!(extractor.name(), "HeuristicExtractor");

    let (nodes, edges) = extractor
        .extract_graph("setting up sqlite database configuration")
        .expect("extract_graph succeeds");
    let node_ids: HashSet<String> = nodes.into_iter().map(|n| n.id).collect();
    assert!(node_ids.contains("sqlite"));
    assert!(node_ids.contains("db-config"));

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source, "db-config");
    assert_eq!(edges[0].target, "sqlite");
    assert_eq!(edges[0].relation, "configures");
    assert_eq!(edges[0].weight, 1.0);
}

#[test]
fn test_memory_extractor_trait_deterministic_ids() {
    let extractor = HeuristicExtractor::new();
    let mut metadata = HashMap::new();
    metadata.insert("conversation_id".to_string(), "conv-123".to_string());
    metadata.insert("message_id".to_string(), "msg-456".to_string());

    let req = ExtractionRequest {
        raw_content: "ritikpathania develops Brain.".to_string(),
        context_metadata: metadata.clone(),
    };

    let result1 = extractor.extract(req.clone()).expect("extract 1");
    let result2 = extractor.extract(req).expect("extract 2");

    assert_eq!(result1.nodes.len(), 2);
    assert_eq!(result1.edges.len(), 1);

    // Deterministic UUID checks across identical extractions
    assert_eq!(result1.nodes[0].id, result2.nodes[0].id);
    assert_eq!(result1.nodes[1].id, result2.nodes[1].id);
    assert_eq!(result1.edges[0].source, result2.edges[0].source);
    assert_eq!(result1.edges[0].target, result2.edges[0].target);
    assert_eq!(result1.edges[0].relation, RelationKind::Develops);

    // Relational integrity: all edge endpoints exist in node set
    let node_id_set: HashSet<NodeId> = result1.nodes.iter().map(|n| n.id).collect();
    assert!(node_id_set.contains(&result1.edges[0].source));
    assert!(node_id_set.contains(&result1.edges[0].target));

    // Provenance validation
    assert_eq!(
        result1.nodes[0].provenance.source_conversation.as_deref(),
        Some("conv-123")
    );
    assert_eq!(
        result1.edges[0].provenance.source_conversation.as_deref(),
        Some("conv-123")
    );
}

// ============================================================================
// Property-Based Tests (proptest)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Property 1: Arbitrary text input resilience (no panics)
    #[test]
    fn prop_arbitrary_string_resilience(s in ".*") {
        let extractor = HeuristicExtractor::new();
        let _ = extractor.extract_graph(&s);
        let _ = extractor.extract_semantic_nodes_json(&s);
    }

    /// Property 2: Referential integrity across structured semantic text and overlap patterns
    #[test]
    fn prop_referential_integrity(
        subj in proptest::sample::select(vec!["Brain", "Alice", "Bob", "Server", "Client", "Daemon", "App"]),
        adj in proptest::sample::select(vec!["Open Source", "Fast", "Modern", "Distributed", "Scalable", "Robust"]),
        syn in proptest::sample::select(vec!["SQLite", "PostgreSQL", "Postgres", "Docker", "Python", "Rust", "DuckDB", "Bun"]),
        noun in proptest::sample::select(vec!["Database", "Engine", "System", "Framework", "Platform", "Application"]),
        pattern_type in 0u8..4u8,
    ) {
        let extractor = HeuristicExtractor::new();
        let text = match pattern_type {
            0 => format!("{subj} is an {adj} {syn} {noun}."),
            1 => format!("{subj} develops {syn}. {syn} runs on Docker."),
            2 => format!("{syn} is stored in config.toml. {subj} uses {syn}."),
            _ => format!("{syn} Rust Python {noun}"),
        };

        if let Ok((nodes, edges)) = extractor.extract_graph(&text) {
            let node_ids: HashSet<String> = nodes.into_iter().map(|n| n.id).collect();
            for edge in edges {
                prop_assert!(
                    node_ids.contains(&edge.source),
                    "Edge source '{}' must exist in extracted nodes for input '{}'",
                    edge.source,
                    text
                );
                prop_assert!(
                    node_ids.contains(&edge.target),
                    "Edge target '{}' must exist in extracted nodes for input '{}'",
                    edge.target,
                    text
                );
            }
        }
    }

    /// Property 3: Graph invariant: no self-loops (source != target)
    #[test]
    fn prop_no_self_loops(s in "\\PC{0,500}") {
        let extractor = HeuristicExtractor::new();
        if let Ok((_nodes, edges)) = extractor.extract_graph(&s) {
            for edge in edges {
                prop_assert_ne!(
                    &edge.source,
                    &edge.target,
                    "Self-loop detected: source '{}' == target '{}'",
                    edge.source,
                    edge.target
                );
            }
        }
    }

    /// Property 4: Idempotency invariant: extract(x) == extract(x)
    #[test]
    fn prop_extraction_idempotency(s in "\\PC{0,500}") {
        let extractor = HeuristicExtractor::new();
        let run1 = extractor.extract_graph(&s);
        let run2 = extractor.extract_graph(&s);

        prop_assert_eq!(run1.is_ok(), run2.is_ok());
        if let (Ok((nodes1, edges1)), Ok((nodes2, edges2))) = (run1, run2) {
            prop_assert_eq!(nodes1, nodes2, "Nodes extraction must be 100% deterministic and idempotent");
            prop_assert_eq!(edges1, edges2, "Edges extraction must be 100% deterministic and idempotent");
        }
    }

    /// Property 5: Edge uniqueness invariant (no duplicate source/target/relation triples)
    #[test]
    fn prop_edge_uniqueness(s in "\\PC{0,500}") {
        let extractor = HeuristicExtractor::new();
        if let Ok((_nodes, edges)) = extractor.extract_graph(&s) {
            let mut seen_keys = HashSet::new();
            for edge in edges {
                let key = (edge.source, edge.target, edge.relation);
                prop_assert!(
                    seen_keys.insert(key.clone()),
                    "Duplicate edge detected: {:?}",
                    key
                );
            }
        }
    }
}
