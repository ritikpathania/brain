#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use brain_core::agents::{ChatAgent, EmbeddingAgent, ExtractionAgent};
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::services::{ExtractionRequest, ExtractionResult, MemoryExtractor};
use brain_domain::{
    Edge, EdgeDTO, GraphProvenance, GraphVersion, Node, NodeDTO, NodeId, NodeType, RelationKind,
    SessionId,
};
use brain_storage::SqliteStorage;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Temporary SQLite Database container ensuring isolated, self-cleaning storage for tests.
pub struct TestDb {
    _temp_dir: TempDir,
    db_path: String,
    storage: Arc<SqliteStorage>,
}

impl TestDb {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create tempdir for test DB");
        let db_path = temp_dir
            .path()
            .join("brain_e2e_test.db")
            .to_string_lossy()
            .to_string();
        let storage = Arc::new(
            SqliteStorage::new(&db_path, 2, true)
                .expect("Failed to initialize SqliteStorage for test DB"),
        );
        Self {
            _temp_dir: temp_dir,
            db_path,
            storage,
        }
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    pub fn storage(&self) -> Arc<SqliteStorage> {
        Arc::clone(&self.storage)
    }
}

/// Lightweight, in-process mock HTTP server for 100% offline testing of HTTP providers.
#[allow(dead_code)]
pub struct MockHttpServer {
    base_url: String,
    shutdown: Arc<AtomicBool>,
    request_count: Arc<AtomicUsize>,
}

#[allow(dead_code)]
impl MockHttpServer {
    /// Spawns a mock server that returns the specified HTTP status code and body.
    pub async fn spawn(status_code: u16, response_body: String, delay: Option<Duration>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind mock server listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);
        let shutdown = Arc::new(AtomicBool::new(false));
        let request_count = Arc::new(AtomicUsize::new(0));

        let shutdown_clone = Arc::clone(&shutdown);
        let request_count_clone = Arc::clone(&request_count);

        tokio::spawn(async move {
            while !shutdown_clone.load(Ordering::Acquire) {
                tokio::select! {
                    Ok((mut socket, _)) = listener.accept() => {
                        request_count_clone.fetch_add(1, Ordering::SeqCst);
                        if let Some(d) = delay {
                            tokio::time::sleep(d).await;
                        }

                        let mut buf = [0u8; 4096];
                        let _ = socket.read(&mut buf).await;

                        let status_text = match status_code {
                            200 => "OK",
                            400 => "Bad Request",
                            401 => "Unauthorized",
                            403 => "Forbidden",
                            404 => "Not Found",
                            500 => "Internal Server Error",
                            _ => "Unknown",
                        };

                        let response = format!(
                            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status_code,
                            status_text,
                            response_body.len(),
                            response_body
                        );

                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        if shutdown_clone.load(Ordering::Acquire) {
                            break;
                        }
                    }
                }
            }
        });

        Self {
            base_url,
            shutdown,
            request_count,
        }
    }

    pub fn url(&self) -> &str {
        &self.base_url
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

impl Drop for MockHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

/// Canonical synonym dictionary matching daemon/brain/extraction/heuristics.py
pub const SYNONYM_MAP: &[(&str, &str, &str, &str)] = &[
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

/// Normalizes an entity text to (id, label, type) following Python oracle rules.
pub fn normalize_entity_oracle(text: &str) -> (String, String, String) {
    let text_clean = text.trim();
    let text_lower = text_clean.to_lowercase();

    for (syn, id, label, typ) in SYNONYM_MAP {
        if &text_lower == syn {
            return (id.to_string(), label.to_string(), typ.to_string());
        }
    }

    if text_clean.contains('.') && !text_clean.contains(' ') {
        return (
            text_clean.to_lowercase(),
            text_clean.to_string(),
            "file".to_string(),
        );
    }

    let clean_id = text_lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    (clean_id, text_clean.to_string(), "concept".to_string())
}

/// Reference Heuristic Extractor implementing the deterministic oracle rules and traits.
#[derive(Debug, Default, Clone)]
pub struct ReferenceHeuristicExtractor;

impl ReferenceHeuristicExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Core extraction logic over text.
    pub fn extract_entities_and_relations(&self, text: &str) -> (Vec<NodeDTO>, Vec<EdgeDTO>) {
        if text.trim().is_empty() {
            return (vec![], vec![]);
        }

        let mut nodes_map: HashMap<String, NodeDTO> = HashMap::new();
        let mut edges: Vec<EdgeDTO> = Vec::new();
        let text_lower = text.to_lowercase();

        // 1. Scan for synonyms
        for (syn, id, label, typ) in SYNONYM_MAP {
            if text_lower.contains(syn) {
                nodes_map.entry(id.to_string()).or_insert_with(|| {
                    NodeDTO::new(
                        id.to_string(),
                        label.to_string(),
                        typ.to_string(),
                        serde_json::json!({}),
                    )
                });
            }
        }

        // 2. Scan for file patterns (e.g. config.toml, .env)
        for word in text.split_whitespace() {
            let clean = word
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-');
            if clean.contains('.') && clean.len() > 3 {
                let id = clean.to_lowercase();
                nodes_map.entry(id.clone()).or_insert_with(|| {
                    NodeDTO::new(
                        id,
                        clean.to_string(),
                        "file".to_string(),
                        serde_json::json!({}),
                    )
                });
            }
        }

        // 3. Scan for proper nouns
        for word in text.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            if !clean.is_empty() && clean.chars().next().unwrap().is_uppercase() {
                let (id, label, typ) = normalize_entity_oracle(clean);
                nodes_map
                    .entry(id.clone())
                    .or_insert_with(|| NodeDTO::new(id, label, typ, serde_json::json!({})));
            }
        }

        // 4. Relation patterns
        // - develops: (person) develops (project)
        if text_lower.contains("develops")
            && nodes_map.contains_key("ritikpathania")
            && nodes_map.contains_key("brain")
        {
            edges.push(EdgeDTO::new(
                "ritikpathania".to_string(),
                "brain".to_string(),
                "develops".to_string(),
                1.0,
            ));
        }

        // - configures / stored_in
        if nodes_map.contains_key("db-config") && nodes_map.contains_key("sqlite") {
            edges.push(EdgeDTO::new(
                "db-config".to_string(),
                "sqlite".to_string(),
                "configures".to_string(),
                1.0,
            ));
        }
        if nodes_map.contains_key("api-key") && nodes_map.contains_key("env-vars") {
            edges.push(EdgeDTO::new(
                "api-key".to_string(),
                "env-vars".to_string(),
                "stored_in".to_string(),
                1.0,
            ));
        }
        if nodes_map.contains_key("sqlite") && nodes_map.contains_key("config.toml") {
            edges.push(EdgeDTO::new(
                "sqlite".to_string(),
                "config.toml".to_string(),
                "stored_in".to_string(),
                1.0,
            ));
        }
        if nodes_map.contains_key("brain") && nodes_map.contains_key("docker") {
            edges.push(EdgeDTO::new(
                "brain".to_string(),
                "docker".to_string(),
                "runs_on".to_string(),
                1.0,
            ));
        }
        if nodes_map.contains_key("brain") && nodes_map.contains_key("python") {
            edges.push(EdgeDTO::new(
                "brain".to_string(),
                "python".to_string(),
                "depends_on".to_string(),
                1.0,
            ));
        }
        if nodes_map.contains_key("rust") && nodes_map.contains_key("uds") {
            edges.push(EdgeDTO::new(
                "rust".to_string(),
                "uds".to_string(),
                "communicates_via".to_string(),
                1.0,
            ));
        }

        // 5. Sequential fallback if no explicit edges found and >= 2 nodes
        if edges.is_empty() && nodes_map.len() >= 2 {
            let keys: Vec<String> = nodes_map.keys().cloned().collect();
            for i in 0..keys.len() - 1 {
                if keys[i] != keys[i + 1] {
                    edges.push(EdgeDTO::new(
                        keys[i].clone(),
                        keys[i + 1].clone(),
                        "associated_with".to_string(),
                        0.5,
                    ));
                }
            }
        }

        // Deduplicate edges and eliminate self-loops
        let mut unique_edges: Vec<EdgeDTO> = Vec::new();
        for edge in edges {
            if edge.source == edge.target {
                continue; // Suppress self-loop
            }
            if !unique_edges.iter().any(|e| {
                e.source == edge.source && e.target == edge.target && e.relation == edge.relation
            }) {
                unique_edges.push(edge);
            }
        }

        let nodes: Vec<NodeDTO> = nodes_map.into_values().collect();
        (nodes, unique_edges)
    }
}

impl ExtractionAgent for ReferenceHeuristicExtractor {
    fn name(&self) -> &str {
        "ReferenceHeuristicExtractor"
    }

    fn extract_graph(&self, text: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        Ok(self.extract_entities_and_relations(text))
    }
}

impl MemoryExtractor for ReferenceHeuristicExtractor {
    fn extract(&self, request: ExtractionRequest) -> Result<ExtractionResult, BrainError> {
        let (node_dtos, edge_dtos) = self.extract_entities_and_relations(&request.raw_content);

        let nodes: Vec<Node> = node_dtos
            .into_iter()
            .map(|d| {
                let node_id = NodeId::new();
                Node::new(node_id, d.label, NodeType::Concept)
            })
            .collect();

        let edges: Vec<Edge> = edge_dtos
            .into_iter()
            .map(|_e| {
                let src_id = NodeId::new();
                let dst_id = NodeId::new();
                Edge::new(src_id, dst_id, RelationKind::AssociatedWith, 1.0)
            })
            .collect();

        Ok(ExtractionResult {
            nodes,
            edges,
            provenance: GraphProvenance::default(),
            graph_version: GraphVersion::V1,
        })
    }
}

/// Reference LLM Memory Extractor parsing markdown code-fence stripped JSON.
#[derive(Debug, Clone)]
pub struct ReferenceLlmMemoryExtractor {
    provider_name: String,
    simulated_response: String,
}

impl ReferenceLlmMemoryExtractor {
    pub fn new(provider_name: &str, simulated_response: &str) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            simulated_response: simulated_response.to_string(),
        }
    }

    pub fn parse_llm_json(
        &self,
        raw_response: &str,
    ) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        let mut cleaned = raw_response.trim();
        if cleaned.starts_with("```json") {
            cleaned = &cleaned[7..];
        } else if cleaned.starts_with("```") {
            cleaned = &cleaned[3..];
        }
        if cleaned.ends_with("```") {
            cleaned = &cleaned[..cleaned.len() - 3];
        }
        let cleaned = cleaned.trim();

        #[derive(serde::Deserialize)]
        struct RawNode {
            id: String,
            label: String,
            #[serde(rename = "type")]
            node_type: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct RawEdge {
            source: String,
            target: String,
            relation: String,
            weight: Option<f64>,
        }

        #[derive(serde::Deserialize)]
        struct GraphSchema {
            nodes: Option<Vec<RawNode>>,
            edges: Option<Vec<RawEdge>>,
        }

        let parsed: GraphSchema =
            serde_json::from_str(cleaned).map_err(|e| BrainError::Validation {
                message: format!("LLM JSON parsing failed: {}. Raw: {}", e, raw_response),
            })?;

        let nodes: Vec<NodeDTO> = parsed
            .nodes
            .unwrap_or_default()
            .into_iter()
            .map(|n| {
                NodeDTO::new(
                    n.id,
                    n.label,
                    n.node_type.unwrap_or_else(|| "concept".to_string()),
                    serde_json::json!({}),
                )
            })
            .collect();

        let edges: Vec<EdgeDTO> = parsed
            .edges
            .unwrap_or_default()
            .into_iter()
            .map(|e| EdgeDTO::new(e.source, e.target, e.relation, e.weight.unwrap_or(1.0)))
            .collect();

        Ok((nodes, edges))
    }
}

impl ExtractionAgent for ReferenceLlmMemoryExtractor {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn extract_graph(&self, _text: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        self.parse_llm_json(&self.simulated_response)
    }
}

/// Mock ChatAgent for testing chat agent interface contracts.
pub struct MockChatAgent {
    pub agent_name: String,
    pub canned_response: String,
    pub fail_with_auth_error: bool,
}

impl MockChatAgent {
    pub fn new(name: &str, response: &str) -> Self {
        Self {
            agent_name: name.to_string(),
            canned_response: response.to_string(),
            fail_with_auth_error: false,
        }
    }
}

impl ChatAgent for MockChatAgent {
    fn name(&self) -> &str {
        &self.agent_name
    }

    fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
        if self.fail_with_auth_error {
            return Err(BrainError::Authorization {
                message: "Authentication failure: invalid API key".to_string(),
            });
        }
        Ok(self.canned_response.clone())
    }
}

/// Mock EmbeddingProvider for testing embedding provider interface contracts.
pub struct MockEmbeddingProvider {
    pub provider_name: &'static str,
    pub dim: usize,
    pub fail_with_auth_error: bool,
}

impl MockEmbeddingProvider {
    pub fn new(name: &'static str, dim: usize) -> Self {
        Self {
            provider_name: name,
            dim,
            fail_with_auth_error: false,
        }
    }
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn name(&self) -> &'static str {
        self.provider_name
    }

    fn embed(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
        if self.fail_with_auth_error {
            return Err(BrainError::Authorization {
                message: "Authentication failed for embedding service".to_string(),
            });
        }
        Ok(vec![0.1f32; self.dim])
    }
}

impl EmbeddingAgent for MockEmbeddingProvider {
    fn name(&self) -> &str {
        self.provider_name
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        self.embed(text)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}
