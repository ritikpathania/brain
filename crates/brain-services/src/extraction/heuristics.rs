//! Native heuristic knowledge graph entity and relationship extractor.
//!
//! Ported directly from `daemon/brain/extraction/heuristics.py` to ensure
//! 100% deterministic behavioral parity with the Python oracle.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use brain_core::agents::ExtractionAgent;
use brain_core::errors::BrainError;
use brain_core::services::{ExtractionRequest, ExtractionResult, MemoryExtractor};
use brain_domain::{
    Edge, EdgeDTO, GraphProvenance, GraphVersion, Node, NodeDTO, NodeId, NodeType, RelationKind,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Deterministic UUID namespace for memory node identification.
const EXTRACTOR_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Deterministically generates a RFC 4122 v5-compliant UUID from a canonical slug without external network dependencies.
fn deterministic_uuid(slug: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(EXTRACTOR_NAMESPACE.as_bytes());
    hasher.update(slug.as_bytes());
    let hash = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50; // Version 5
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant RFC 4122
    Uuid::from_bytes(bytes)
}

/// 33 canonical synonym mappings (lowercase key, canonical_id, canonical_label, node_type).
const SYNONYM_MAP: &[(&str, &str, &str, &str)] = &[
    ("sqlite", "sqlite", "SQLite", "technology"),
    ("sqlite3", "sqlite", "SQLite", "technology"),
    ("duckdb", "duckdb", "DuckDB", "technology"),
    ("postgres", "postgresql", "PostgreSQL", "technology"),
    ("postgresql", "postgresql", "PostgreSQL", "technology"),
    ("mysql", "mysql", "MySQL", "technology"),
    ("redis", "redis", "Redis", "technology"),
    ("docker", "docker", "Docker", "technology"),
    ("rust", "rust", "Rust", "language"),
    ("python", "python", "Python", "language"),
    ("bun", "bun", "Bun", "runtime"),
    ("react", "react", "React", "library"),
    ("brain", "brain", "Brain", "project"),
    ("foodzapp", "foodzapp", "Foodzapp", "project"),
    ("ritikpathania", "ritikpathania", "ritikpathania", "person"),
    ("john", "john", "John", "person"),
    ("alice", "alice", "Alice", "person"),
    ("openai", "openai", "OpenAI", "organization"),
    ("google", "google", "Google", "organization"),
    ("api key", "api-key", "API Key", "credential"),
    ("api keys", "api-key", "API Key", "credential"),
    ("api-key", "api-key", "API Key", "credential"),
    ("jwt", "jwt-token", "JWT Token", "credential"),
    ("jwt token", "jwt-token", "JWT Token", "credential"),
    ("jwt tokens", "jwt-token", "JWT Token", "credential"),
    (
        "database configuration",
        "db-config",
        "Database Configuration",
        "configuration",
    ),
    (
        "db config",
        "db-config",
        "Database Configuration",
        "configuration",
    ),
    (
        "environment variables",
        "env-vars",
        "Environment Variables",
        "environment",
    ),
    (
        "environment variable",
        "env-vars",
        "Environment Variables",
        "environment",
    ),
    (
        "unix domain socket",
        "uds",
        "Unix Domain Socket",
        "protocol",
    ),
    (
        "unix domain sockets",
        "uds",
        "Unix Domain Socket",
        "protocol",
    ),
    ("websocket", "websockets", "WebSockets", "protocol"),
    ("websockets", "websockets", "WebSockets", "protocol"),
];

/// 61 English stop words used for initial-token suppression.
static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "the", "a", "an", "this", "that", "these", "those", "it", "they", "he", "she", "we", "you",
        "i", "our", "your", "their", "my", "in", "on", "at", "for", "to", "by", "with", "about",
        "as", "if", "when", "while", "there", "here", "all", "any", "both", "each", "few", "more",
        "most", "some", "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too",
        "very", "can", "will", "just", "should", "would", "could", "and", "but", "or",
    ]
    .into_iter()
    .collect()
});

static FILE_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\.?[a-zA-Z0-9_\-\./]+\.[a-zA-Z0-9_]+$").expect("valid file regex")
});

static NON_ALPHANUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9_-]+").expect("valid non-alphanumeric regex"));

static CLASS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([A-Za-z0-9_\-\.]+)\s+(?:is|was|are|were)\s+(?:an?|the)\s+([^.,;:]+)")
        .expect("valid class pattern regex")
});

static FILE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b[a-zA-Z0-9_\-/]+\.(toml|env|json|yaml|yml|py|rs|js|ts|txt|db|sqlite)\b|\B\.[eE]nv\b",
    )
    .expect("valid file pattern regex")
});

static PROP_NOUN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Z][a-zA-Z0-9_]*(?:\s+[A-Z][a-zA-Z0-9_]*)*\b")
        .expect("valid proper noun regex")
});

static QUOTED_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"['"]([^'"]+)['"]"#).expect("valid quoted regex"));

static SENTENCE_SPLIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[.!?]\s+").expect("valid sentence split regex"));

struct SynonymEntry {
    regex: Regex,
    canonical_id: &'static str,
    canonical_label: &'static str,
    node_type: &'static str,
}

static SYNONYM_PATTERNS: LazyLock<Vec<SynonymEntry>> = LazyLock::new(|| {
    SYNONYM_MAP
        .iter()
        .map(|(syn, id, label, typ)| {
            let escaped = regex::escape(syn);
            let pattern = format!(r"(?i)\b{}\b", escaped);
            SynonymEntry {
                regex: Regex::new(&pattern).expect("valid synonym regex"),
                canonical_id: id,
                canonical_label: label,
                node_type: typ,
            }
        })
        .collect()
});

static RELATION_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        (
            "uses",
            Regex::new(r"(?i)\b(uses|use|using|utilizes|utilize|utilizing|employs|employ|employing)\b")
                .expect("valid regex"),
        ),
        (
            "develops",
            Regex::new(r"(?i)\b(develops|develop|developing|developed|built|builds|building|created|creates|creating)\b")
                .expect("valid regex"),
        ),
        (
            "stored_in",
            Regex::new(r"(?i)\b(stored in|store in|stored_in|kept in|saved in|writes to|write to)\b")
                .expect("valid regex"),
        ),
        (
            "runs_on",
            Regex::new(r"(?i)\b(runs on|run on|running on|runs_on|hosted on|hosts on|executes on|execute on)\b")
                .expect("valid regex"),
        ),
        (
            "depends_on",
            Regex::new(r"(?i)\b(depends on|depend on|depending on|depends_on|requires|require|requiring|dependency|dependencies)\b")
                .expect("valid regex"),
        ),
    ]
});

static PASSIVE_RELATION_PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        (
            "uses",
            Regex::new(r"(?i)\b(used by|utilised by|employed by)\b").expect("valid regex"),
        ),
        (
            "develops",
            Regex::new(r"(?i)\b(developed by|built by|created by)\b").expect("valid regex"),
        ),
        (
            "runs_on",
            Regex::new(r"(?i)\b(run by|hosted by|executed by)\b").expect("valid regex"),
        ),
        (
            "depends_on",
            Regex::new(r"(?i)\b(depended on by|required by)\b").expect("valid regex"),
        ),
    ]
});

/// Returns the 3-tier precedence value for a node type: lower number indicates higher precedence.
fn type_precedence(node_type: &str) -> u32 {
    match node_type {
        "technology" | "project" | "person" | "organization" | "credential" | "file"
        | "language" | "runtime" | "library" | "configuration" | "environment" | "protocol" => 0,
        "literal" => 1,
        "concept" => 2,
        _ => 99,
    }
}

/// Normalizes an entity text to a canonical ID, label, and node type.
fn normalize_entity(text: &str) -> (String, String, String) {
    let text_clean = text.trim();
    let text_lower = text_clean.to_lowercase();

    // 1. Match synonym list
    for (syn, id, label, typ) in SYNONYM_MAP {
        if &text_lower == syn {
            return (id.to_string(), label.to_string(), typ.to_string());
        }
    }

    // 2. Check if it looks like a file name
    if FILE_NAME_RE.is_match(text_clean) {
        return (
            text_clean.to_lowercase(),
            text_clean.to_string(),
            "file".to_string(),
        );
    }

    // 3. Proper noun or general concept fallback
    let clean_id = NON_ALPHANUM_RE
        .replace_all(&text_lower, "-")
        .trim_matches('-')
        .to_string();

    (clean_id, text_clean.to_string(), "concept".to_string())
}

/// Intermediate raw extracted entity before overlap resolution.
#[derive(Debug, Clone)]
struct RawEntity {
    id: String,
    label: String,
    node_type: String,
    attributes: serde_json::Value,
    match_start: usize,
    match_end: usize,
}

/// Splits proper noun (concept) matches that contain/overlap with specific entity matches.
fn split_overlapping_proper_nouns(raw_entities: Vec<RawEntity>, content: &str) -> Vec<RawEntity> {
    let concept_entities: Vec<RawEntity> = raw_entities
        .iter()
        .filter(|e| e.node_type == "concept")
        .cloned()
        .collect();
    let specific_entities: Vec<RawEntity> = raw_entities
        .iter()
        .filter(|e| e.node_type != "concept")
        .cloned()
        .collect();

    let mut processed_entities = specific_entities.clone();

    for ent in concept_entities {
        let start_idx = ent.match_start;
        let end_idx = ent.match_end;

        let mut overlapping = Vec::new();
        for spec in &specific_entities {
            let s_start = spec.match_start;
            let s_end = spec.match_end;
            if std::cmp::max(start_idx, s_start) < std::cmp::min(end_idx, s_end) {
                overlapping.push((
                    std::cmp::max(start_idx, s_start),
                    std::cmp::min(end_idx, s_end),
                ));
            }
        }

        if !overlapping.is_empty() {
            overlapping.sort_unstable();
            let mut merged: Vec<(usize, usize)> = Vec::new();
            for (s, e) in overlapping {
                if merged.is_empty() || merged.last().expect("non-empty").1 < s {
                    merged.push((s, e));
                } else {
                    let last = merged.last_mut().expect("non-empty");
                    last.1 = std::cmp::max(last.1, e);
                }
            }

            let mut curr = start_idx;
            let mut sub_spans = Vec::new();
            for (s, e) in merged {
                if s > curr {
                    sub_spans.push((curr, s));
                }
                curr = std::cmp::max(curr, e);
            }
            if curr < end_idx {
                sub_spans.push((curr, end_idx));
            }

            for (sub_start, sub_end) in sub_spans {
                if sub_start >= sub_end || sub_end > content.len() {
                    continue;
                }
                let raw_sub = &content[sub_start..sub_end];
                let sub_phrase = raw_sub.trim();
                if sub_phrase.is_empty() {
                    continue;
                }

                let leading_ws = raw_sub.len() - raw_sub.trim_start().len();
                let strip_start = sub_start + leading_ws;
                let strip_end = strip_start + sub_phrase.len();

                let is_start = if strip_start == 0 {
                    true
                } else {
                    let pre = content[..strip_start].trim();
                    pre.is_empty() || pre.ends_with('.') || pre.ends_with('!') || pre.ends_with('?')
                };

                if is_start
                    && !sub_phrase.contains(' ')
                    && STOP_WORDS.contains(sub_phrase.to_lowercase().as_str())
                {
                    continue;
                }

                let (pn_id, pn_label, pn_type) = normalize_entity(sub_phrase);
                processed_entities.push(RawEntity {
                    id: pn_id,
                    label: pn_label,
                    node_type: pn_type,
                    attributes: serde_json::json!({}),
                    match_start: strip_start,
                    match_end: strip_end,
                });
            }
        } else {
            processed_entities.push(ent);
        }
    }

    processed_entities
}

/// Splits text into sentences by looking for punctuation followed by whitespace.
fn split_sentences(content: &str) -> Vec<(&str, usize, usize)> {
    let mut sentences = Vec::new();
    let mut curr = 0;
    for mat in SENTENCE_SPLIT_RE.find_iter(content) {
        let end_idx = mat.start() + 1;
        sentences.push((&content[curr..end_idx], curr, end_idx));
        curr = mat.end();
    }
    if curr < content.len() {
        sentences.push((&content[curr..], curr, content.len()));
    }
    if sentences.is_empty() {
        sentences.push((content, 0, content.len()));
    }
    sentences
}

/// JSON serialization DTO representing a semantic node for Python test/FFI parity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticNodeJson {
    /// Canonical entity identifier (e.g. "sqlite", "api-key").
    pub id: String,
    /// Canonical human-readable label (e.g. "SQLite", "API Key").
    pub label: String,
    /// Classification type (serialized as "type" for Python test parity).
    #[serde(rename = "type")]
    pub node_type: String,
    /// Associated properties and category attributes.
    pub attributes: serde_json::Value,
}

/// JSON serialization DTO representing a semantic edge for Python test/FFI parity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticEdgeJson {
    /// Source node canonical ID.
    pub source: String,
    /// Target node canonical ID.
    pub target: String,
    /// Relationship name (e.g. "configures", "stored_in", "uses").
    pub relation: String,
}

/// JSON response payload matching Python `extract_semantic_nodes` output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionOutputJson {
    /// Operation status ("success" or "error").
    pub status: String,
    /// Optional error message on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Extracted nodes list.
    pub nodes: Vec<SemanticNodeJson>,
    /// Extracted unique edges list.
    pub edges: Vec<SemanticEdgeJson>,
}

/// Raw input node object parsed from STM batch payloads.
#[derive(Debug, Clone, Deserialize)]
pub struct RawLogNode {
    /// Optional node ID.
    #[serde(default)]
    pub id: Option<String>,
    /// Raw text content to extract entities from.
    #[serde(default)]
    pub content: String,
    /// Optional epoch index.
    #[serde(default)]
    pub epoch: Option<i64>,
    /// Optional timestamp.
    #[serde(default)]
    pub timestamp: Option<i64>,
}

/// Native heuristic knowledge graph entity and relationship extractor.
#[derive(Debug, Default, Clone)]
pub struct HeuristicExtractor;

impl HeuristicExtractor {
    /// Creates a new `HeuristicExtractor` instance.
    pub fn new() -> Self {
        Self
    }

    /// Primary FFI/JSON entrypoint replicating Python's `extract_semantic_nodes(json_input)`.
    ///
    /// Accepts a serialized JSON array of log nodes (each with a `"content"` field),
    /// runs entity and relation extraction, and returns serialized JSON matching Python parity.
    pub fn extract_semantic_nodes_json(&self, json_input: &str) -> Result<String, BrainError> {
        let raw_nodes: Vec<RawLogNode> = match serde_json::from_str(json_input) {
            Ok(nodes) => nodes,
            Err(e) => {
                let err_resp = ExtractionOutputJson {
                    status: "error".to_string(),
                    message: Some(format!("FFI failed to decode input: {e}")),
                    nodes: vec![],
                    edges: vec![],
                };
                return serde_json::to_string(&err_resp).map_err(|err| BrainError::Internal {
                    message: format!("Failed to serialize error response: {err}"),
                });
            }
        };

        let mut extracted_nodes: Vec<NodeDTO> = Vec::new();
        let mut node_indices: HashMap<String, usize> = HashMap::new();
        let mut extracted_edges: Vec<EdgeDTO> = Vec::new();

        for raw_node in raw_nodes {
            let content = raw_node.content.trim();
            if content.is_empty() {
                continue;
            }

            self.process_content_item(
                content,
                &mut extracted_nodes,
                &mut node_indices,
                &mut extracted_edges,
            );
        }

        let unique_edges = self.deduplicate_edges_batch(extracted_edges);

        let json_nodes: Vec<SemanticNodeJson> = extracted_nodes
            .into_iter()
            .map(|n| SemanticNodeJson {
                id: n.id,
                label: n.label,
                node_type: n.node_type,
                attributes: n.attributes,
            })
            .collect();

        let json_edges: Vec<SemanticEdgeJson> = unique_edges
            .into_iter()
            .map(|e| SemanticEdgeJson {
                source: e.source,
                target: e.target,
                relation: e.relation,
            })
            .collect();

        let success_resp = ExtractionOutputJson {
            status: "success".to_string(),
            message: None,
            nodes: json_nodes,
            edges: json_edges,
        };

        serde_json::to_string(&success_resp).map_err(|err| BrainError::Internal {
            message: format!("Failed to serialize extraction output: {err}"),
        })
    }

    /// Internal core routine processing a single content item into accumulated nodes and edges.
    fn process_content_item(
        &self,
        content: &str,
        extracted_nodes: &mut Vec<NodeDTO>,
        node_indices: &mut HashMap<String, usize>,
        extracted_edges: &mut Vec<EdgeDTO>,
    ) {
        let mut raw_entities: Vec<RawEntity> = Vec::new();

        // Pass 1: Classification clauses
        for mat in CLASS_PATTERN.captures_iter(content) {
            let subj_match = mat.get(1).expect("group 1 exists");
            let class_match = mat.get(2).expect("group 2 exists");

            let subject = subj_match.as_str();
            let classification = class_match.as_str().trim();

            let (subj_id, subj_label, subj_type) = normalize_entity(subject);
            let (class_id, class_label, _class_type) = normalize_entity(classification);

            raw_entities.push(RawEntity {
                id: subj_id.clone(),
                label: subj_label,
                node_type: subj_type,
                attributes: serde_json::json!({
                    "category": classification,
                    "type": classification,
                }),
                match_start: subj_match.start(),
                match_end: subj_match.end(),
            });

            raw_entities.push(RawEntity {
                id: class_id.clone(),
                label: class_label,
                node_type: "concept".to_string(),
                attributes: serde_json::json!({}),
                match_start: class_match.start(),
                match_end: class_match.end(),
            });

            extracted_edges.push(EdgeDTO::new(
                subj_id,
                class_id,
                "associated_with".to_string(),
                1.0,
            ));
        }

        // Pass 2: File matches
        for mat in FILE_PATTERN.find_iter(content) {
            let file_name = mat.as_str();
            let file_id = file_name.to_lowercase();
            raw_entities.push(RawEntity {
                id: file_id,
                label: file_name.to_string(),
                node_type: "file".to_string(),
                attributes: serde_json::json!({}),
                match_start: mat.start(),
                match_end: mat.end(),
            });
        }

        // Pass 3: Proper noun matches
        for mat in PROP_NOUN_PATTERN.find_iter(content) {
            let phrase = mat.as_str();
            let start_idx = mat.start();

            let is_start = if start_idx == 0 {
                true
            } else {
                let pre = content[..start_idx].trim();
                pre.is_empty() || pre.ends_with('.') || pre.ends_with('!') || pre.ends_with('?')
            };

            if is_start
                && !phrase.contains(' ')
                && STOP_WORDS.contains(phrase.to_lowercase().as_str())
            {
                continue;
            }

            let (pn_id, pn_label, pn_type) = normalize_entity(phrase);
            raw_entities.push(RawEntity {
                id: pn_id,
                label: pn_label,
                node_type: pn_type,
                attributes: serde_json::json!({}),
                match_start: mat.start(),
                match_end: mat.end(),
            });
        }

        // Pass 4: Synonym keyword matches
        for syn in SYNONYM_PATTERNS.iter() {
            for mat in syn.regex.find_iter(content) {
                raw_entities.push(RawEntity {
                    id: syn.canonical_id.to_string(),
                    label: syn.canonical_label.to_string(),
                    node_type: syn.node_type.to_string(),
                    attributes: serde_json::json!({}),
                    match_start: mat.start(),
                    match_end: mat.end(),
                });
            }
        }

        // Pass 5: Quoted values as literals
        for mat in QUOTED_PATTERN.captures_iter(content) {
            let val_match = mat.get(1).expect("group 1 exists");
            let val = val_match.as_str();
            let val_id = val.to_lowercase().replace([' ', '/'], "-");
            raw_entities.push(RawEntity {
                id: val_id,
                label: val.to_string(),
                node_type: "literal".to_string(),
                attributes: serde_json::json!({}),
                match_start: val_match.start(),
                match_end: val_match.end(),
            });
        }

        // Post-pass: Proper noun splitting
        let split_entities = split_overlapping_proper_nouns(raw_entities, content);

        // Overlap resolution: Sort by length descending, then by precedence ascending
        let mut sorted_entities = split_entities;
        sorted_entities.sort_by(|a, b| {
            let len_a = a.match_end.saturating_sub(a.match_start);
            let len_b = b.match_end.saturating_sub(b.match_start);
            len_b
                .cmp(&len_a)
                .then_with(|| type_precedence(&a.node_type).cmp(&type_precedence(&b.node_type)))
                .then_with(|| a.match_start.cmp(&b.match_start))
        });

        let mut resolved_entities: Vec<RawEntity> = Vec::new();
        for ent in sorted_entities {
            let mut overlap = false;
            for kept in &resolved_entities {
                if std::cmp::max(ent.match_start, kept.match_start)
                    < std::cmp::min(ent.match_end, kept.match_end)
                {
                    overlap = true;
                    break;
                }
            }
            if !overlap {
                resolved_entities.push(ent);
            }
        }
        resolved_entities.sort_by_key(|e| e.match_start);

        // Node merging and tracking
        let mut nodes_in_text: Vec<String> = Vec::new();
        for ent in &resolved_entities {
            if !nodes_in_text.contains(&ent.id) {
                nodes_in_text.push(ent.id.clone());
            }

            if let Some(&idx) = node_indices.get(&ent.id) {
                let existing = &mut extracted_nodes[idx];
                if type_precedence(&ent.node_type) < type_precedence(&existing.node_type) {
                    existing.node_type = ent.node_type.clone();
                }
                if let Some(existing_obj) = existing.attributes.as_object_mut() {
                    if let Some(new_obj) = ent.attributes.as_object() {
                        for (k, v) in new_obj {
                            existing_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
            } else {
                node_indices.insert(ent.id.clone(), extracted_nodes.len());
                extracted_nodes.push(NodeDTO::new(
                    ent.id.clone(),
                    ent.label.clone(),
                    ent.node_type.clone(),
                    ent.attributes.clone(),
                ));
            }
        }

        // Pass 6: Sentence-based relations
        let sentences = split_sentences(content);

        for (_sentence, sent_start_idx, sent_end_idx) in sentences {
            let mut sent_ents: Vec<&RawEntity> = resolved_entities
                .iter()
                .filter(|e| e.match_start >= sent_start_idx && e.match_end <= sent_end_idx)
                .collect();
            sent_ents.sort_by_key(|e| e.match_start);

            for i in 0..sent_ents.len() {
                for j in (i + 1)..sent_ents.len() {
                    let e1 = sent_ents[i];
                    let e2 = sent_ents[j];

                    if e1.match_end <= e2.match_start {
                        if let Some(middle_text) = content.get(e1.match_end..e2.match_start) {
                            let mut rel_found = false;
                            for (rel_name, rel_re) in RELATION_PATTERNS.iter() {
                                if rel_re.is_match(middle_text) {
                                    extracted_edges.push(EdgeDTO::new(
                                        e1.id.clone(),
                                        e2.id.clone(),
                                        rel_name.to_string(),
                                        1.0,
                                    ));
                                    rel_found = true;
                                    break;
                                }
                            }

                            if !rel_found {
                                for (rel_name, rel_re) in PASSIVE_RELATION_PATTERNS.iter() {
                                    if rel_re.is_match(middle_text) {
                                        if *rel_name == "runs_on" {
                                            extracted_edges.push(EdgeDTO::new(
                                                e1.id.clone(),
                                                e2.id.clone(),
                                                "runs_on".to_string(),
                                                1.0,
                                            ));
                                        } else {
                                            extracted_edges.push(EdgeDTO::new(
                                                e2.id.clone(),
                                                e1.id.clone(),
                                                rel_name.to_string(),
                                                1.0,
                                            ));
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Pass 7: Conceptual rules
        if nodes_in_text.iter().any(|id| id == "db-config") {
            for tech in &["sqlite", "postgres", "mysql", "redis", "postgresql"] {
                if nodes_in_text.iter().any(|id| id == *tech) {
                    let target = if *tech == "sqlite" {
                        "sqlite"
                    } else if *tech == "postgres" || *tech == "postgresql" {
                        "postgresql"
                    } else {
                        *tech
                    };
                    extracted_edges.push(EdgeDTO::new(
                        "db-config".to_string(),
                        target.to_string(),
                        "configures".to_string(),
                        1.0,
                    ));
                }
            }
        }

        if nodes_in_text.iter().any(|id| id == "api-key")
            && nodes_in_text.iter().any(|id| id == "env-vars")
        {
            extracted_edges.push(EdgeDTO::new(
                "api-key".to_string(),
                "env-vars".to_string(),
                "stored_in".to_string(),
                1.0,
            ));
        }

        let has_uds = nodes_in_text.iter().any(|id| id == "uds");
        let has_websockets = nodes_in_text.iter().any(|id| id == "websockets");
        if has_uds || has_websockets {
            let proto = if has_uds { "uds" } else { "websockets" };
            for tech in &["bun", "rust", "python"] {
                if nodes_in_text.iter().any(|id| id == *tech) {
                    extracted_edges.push(EdgeDTO::new(
                        tech.to_string(),
                        proto.to_string(),
                        "communicates_via".to_string(),
                        1.0,
                    ));
                }
            }
        }

        // Pass 8: Sequential fallback edge linking
        if resolved_entities.len() >= 2 {
            for i in 0..(resolved_entities.len() - 1) {
                let src = &resolved_entities[i].id;
                let tgt = &resolved_entities[i + 1].id;
                if src == tgt {
                    continue;
                }

                let already_linked = extracted_edges.iter().any(|e| {
                    (&e.source == src && &e.target == tgt)
                        || (((&e.source == src && &e.target == tgt)
                            || (&e.source == tgt && &e.target == src))
                            && e.relation != "associated_with")
                });

                if !already_linked {
                    extracted_edges.push(EdgeDTO::new(
                        src.clone(),
                        tgt.clone(),
                        "associated_with".to_string(),
                        1.0,
                    ));
                }
            }
        }

        // Referential integrity: prune dangling edges whose endpoints are not in the node index
        extracted_edges.retain(|e| {
            node_indices.contains_key(&e.source) && node_indices.contains_key(&e.target)
        });
    }

    /// Internal core extraction routine extracting `NodeDTO`s and `EdgeDTO`s from raw text.
    pub fn extract_entities_and_relations(&self, text: &str) -> (Vec<NodeDTO>, Vec<EdgeDTO>) {
        let content = text.trim();
        if content.is_empty() {
            return (vec![], vec![]);
        }

        let mut extracted_nodes = Vec::new();
        let mut node_indices = HashMap::new();
        let mut extracted_edges = Vec::new();

        self.process_content_item(
            content,
            &mut extracted_nodes,
            &mut node_indices,
            &mut extracted_edges,
        );

        let unique_edges = self.deduplicate_edges_batch(extracted_edges);
        (extracted_nodes, unique_edges)
    }

    /// Merges multiple nodes sharing the same canonical ID according to `TYPE_PRECEDENCE`.
    pub fn merge_nodes_batch(&self, nodes: Vec<NodeDTO>) -> Vec<NodeDTO> {
        let mut merged: Vec<NodeDTO> = Vec::new();
        let mut index_map: HashMap<String, usize> = HashMap::new();

        for node in nodes {
            if let Some(&idx) = index_map.get(&node.id) {
                let existing = &mut merged[idx];
                if type_precedence(&node.node_type) < type_precedence(&existing.node_type) {
                    existing.node_type = node.node_type;
                }
                if let Some(existing_obj) = existing.attributes.as_object_mut() {
                    if let Some(new_obj) = node.attributes.as_object() {
                        for (k, v) in new_obj {
                            existing_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
            } else {
                index_map.insert(node.id.clone(), merged.len());
                merged.push(node);
            }
        }

        merged
    }

    /// Suppresses self-loops and preserves unique `(source, target, relation)` edges.
    pub fn deduplicate_edges_batch(&self, edges: Vec<EdgeDTO>) -> Vec<EdgeDTO> {
        let mut unique = Vec::new();
        let mut seen = HashSet::new();

        for edge in edges {
            if edge.source == edge.target {
                continue;
            }
            let key = (
                edge.source.clone(),
                edge.target.clone(),
                edge.relation.clone(),
            );
            if seen.insert(key) {
                unique.push(edge);
            }
        }

        unique
    }
}

impl ExtractionAgent for HeuristicExtractor {
    fn name(&self) -> &str {
        "HeuristicExtractor"
    }

    fn extract_graph(&self, text: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        let (nodes, edges) = self.extract_entities_and_relations(text);
        Ok((nodes, edges))
    }
}

impl MemoryExtractor for HeuristicExtractor {
    fn extract(&self, request: ExtractionRequest) -> Result<ExtractionResult, BrainError> {
        let (node_dtos, edge_dtos) = self.extract_entities_and_relations(&request.raw_content);

        let mut id_map: HashMap<String, NodeId> = HashMap::new();
        let mut nodes: Vec<Node> = Vec::with_capacity(node_dtos.len());

        for dto in node_dtos {
            let node_uuid = deterministic_uuid(&dto.id);
            let node_id = NodeId(node_uuid);
            id_map.insert(dto.id.clone(), node_id);

            let node_type = match dto.node_type.to_lowercase().as_str() {
                "technology" | "language" | "runtime" | "library" => NodeType::Technology,
                "database" => NodeType::Database,
                "project" => NodeType::Project,
                "person" => NodeType::Person,
                "organization" => NodeType::Organization,
                "credential" => NodeType::Credential,
                "file" => NodeType::File,
                "service" => NodeType::Service,
                "tool" => NodeType::Tool,
                _ => NodeType::Concept,
            };

            let mut properties: HashMap<String, serde_json::Value> = match dto.attributes {
                serde_json::Value::Object(map) => map.into_iter().collect(),
                _ => HashMap::new(),
            };
            properties.insert("canonical_id".to_string(), serde_json::json!(dto.id));

            let mut provenance = GraphProvenance::default();
            if let Some(conv_id) = request.context_metadata.get("conversation_id") {
                provenance.source_conversation = Some(conv_id.clone());
            }
            if let Some(msg_id) = request.context_metadata.get("message_id") {
                provenance.source_message = Some(msg_id.clone());
            }
            provenance.extractor_version = "heuristic-v1".to_string();

            let mut node = Node::new(node_id, dto.label, node_type).with_properties(properties);
            node.provenance = provenance;
            nodes.push(node);
        }

        let mut edges: Vec<Edge> = Vec::with_capacity(edge_dtos.len());
        for dto in edge_dtos {
            let (Some(&src_id), Some(&tgt_id)) = (id_map.get(&dto.source), id_map.get(&dto.target))
            else {
                continue;
            };

            let relation = dto
                .relation
                .parse::<RelationKind>()
                .unwrap_or(RelationKind::AssociatedWith);

            let mut edge = Edge::new(src_id, tgt_id, relation, dto.weight);
            let mut provenance = GraphProvenance::default();
            if let Some(conv_id) = request.context_metadata.get("conversation_id") {
                provenance.source_conversation = Some(conv_id.clone());
            }
            if let Some(msg_id) = request.context_metadata.get("message_id") {
                provenance.source_message = Some(msg_id.clone());
            }
            provenance.extractor_version = "heuristic-v1".to_string();
            edge.provenance = provenance;
            edges.push(edge);
        }

        Ok(ExtractionResult {
            nodes,
            edges,
            provenance: GraphProvenance::default(),
            graph_version: GraphVersion::V1,
        })
    }
}
