use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::EmbeddingProvider;
use brain_domain::{Node, NodeId, NodeType, Edge, RelationKind, TemporalEdge, TimePoint, temporal::TemporalValidity};
use brain_storage::SqliteStorage;
use brain_services::retrieval::eval_harness::{
    QueryCorpus, QueryItem, GroundTruthCorpus, CorpusNode, GroundTruthItem,
    HybridRetriever, FeatureProvider, FtsRetriever, SemanticRetriever,
};

/// Deterministic hashing-based embedding provider to construct realistic semantic spaces in tests.
#[derive(Debug)]
pub struct HashingEmbeddingProvider;

impl EmbeddingProvider for HashingEmbeddingProvider {
    fn name(&self) -> &'static str {
        "hashing-embedding-provider"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let mut v = vec![0.0f32; 384];
        let stop_words: HashSet<&str> = [
            "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in", "on",
            "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it", "its",
            "you", "your", "my", "up", "down", "out", "off", "how", "do", "i", "by", "specifying",
        ].iter().cloned().collect();

        let tokens: Vec<String> = text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && !stop_words.contains(s))
            .map(|s| match s {
                "olap" => "analytics".to_string(),
                "dashboard" => "metrics".to_string(),
                "instrumentation" => "telemetry".to_string(),
                "reporter" => "export".to_string(),
                "termination" => "sigterm".to_string(),
                "graceful" => "draining".to_string(),
                "shutdown" => "sigterm".to_string(),
                "routine" => "signals".to_string(),
                other => other.to_string(),
            })
            .collect();

        if tokens.is_empty() {
            return Ok(v);
        }

        for tok in &tokens {
            let mut h: u32 = 5381;
            for c in tok.bytes() {
                h = h.wrapping_shl(5).wrapping_add(h).wrapping_add(c as u32);
            }
            let idx = (h % 384) as usize;
            v[idx] += 1.0;
        }

        let mut norm_sq = 0.0f32;
        for &val in &v {
            norm_sq += val * val;
        }
        let norm = norm_sq.sqrt();
        if norm > 0.0 {
            for val in v.iter_mut() {
                *val /= norm;
            }
        }
        Ok(v)
    }
}

/// Exposes all components loaded in the production corpus environment.
pub struct ProductionCorpus {
    /// In-memory temporary SQLite database populated with corpus nodes, edges, and feedback history.
    pub _storage: Arc<SqliteStorage>,
    /// The queries definition corpus.
    pub queries: QueryCorpus,
    /// The expected retrieval ground truth mapping.
    pub ground_truth: GroundTruthCorpus,
    /// Loaded RRF hybrid retrieval driver.
    pub retriever: HybridRetriever<FtsRetriever, SemanticRetriever>,
    /// Diagnostic feature context provider.
    pub feature_provider: FeatureProvider,
}

const TOPICS: &[(&str, &str)] = &[
    ("TypeScript UDS client transport socket path configuration", "TypeScript"),
    ("Python dynamic plugin loader pyo3 module instantiation scripts", "Python"),
    ("CLI subcommand parser arguments run start status daemon", "CLI"),
    ("SIGTERM SIGINT graceful worker draining lifecycle signals run", "Lifecycle"),
    ("DaemonCleanupGuard Drop remove PID socket files startup", "Cleanup"),
    ("SQLite storage migration connection pooling driver initialization", "Database"),
    ("Stored events EventLogRepository transaction safety log repository", "EventLog"),
    ("ProjectionCheckpointRepository state manager persistent indexing checkpoint", "Checkpoint"),
    ("JobReadModel jobs SQLite schema mapping write model repository", "Jobs"),
    ("SessionCacheManager session memory hydration cache manager", "Session"),
    ("MemoryPipelineBuilder registry traversal directionality pipeline builder", "Pipeline"),
    ("FtsRetriever SQLite match score ranking lexical search", "FTS"),
    ("SemanticRetriever embedding search query provider lookup index", "Semantic"),
    ("HybridRetriever RRF ranker score fusion candidates strategy", "Hybrid"),
    ("FeatureProvider access frequency recency temporal decay context", "Features"),
    ("CalibrationEngine grid search weights optimize objective function", "Calibration"),
    ("SensitivityAnalysis feature impact gradient baseline diagnostics weights", "Sensitivity"),
    ("CorrelationEngine feature redundancy Pearson Spearman matrix correlation", "Correlation"),
    ("PruningExperimentRunner prune nodes degradation delta composite score", "Pruning"),
    ("LogisticRegressionModel trained weights intercept bias classification loss", "Logistic"),
    ("Web application frontend React styling TailwindCSS aesthetics responsive", "UI"),
    ("Next.js framework server side rendering API routes routing backend", "UI"),
    ("Vite bundler development server hot module replacement build pipeline", "Tooling"),
    ("Docker container orchestration service configuration compose ports host", "DevOps"),
    ("Kubernetes cluster deployment pods scaling load balancer ingress", "DevOps"),
    ("GitHub Actions CI CD workflow runner job steps checkout", "CI"),
    ("Unit tests integration test code coverage reports validation", "Testing"),
    ("Performance profiling peak RSS frame draw latency CPU cycle", "Profiling"),
    ("Memory leak detection valgrind instruments profiling heap analyzer", "Profiling"),
    ("Asynchronous runtime Tokio event loop spawn task join set", "Async"),
    ("Thread safety Arc Mutex RwLock atomic variables synchronization concurrent", "Sync"),
    ("REST API endpoints HTTP methods status codes JSON payload parser", "API"),
    ("gRPC protocol buffers service definition serialization multiplexing HTTP2", "API"),
    ("WebSocket persistent connection duplex communication handshake frame socket", "API"),
    ("Redis in memory database key value cache eviction policies TTL", "Cache"),
    ("Elasticsearch search engine inverted index tokenization analyzer clustering", "Search"),
    ("Kafka message broker topics partitions replication offset consumer group", "Queue"),
    ("RabbitMQ message broker exchange routing key binding queues ack", "Queue"),
    ("GraphQL query mutation subscription schema types resolver database", "API"),
    ("OpenID Connect OAuth authorization code flow access token JWT scope", "Security"),
    ("SSL TLS handshake certificate authority private key decryption cipher", "Security"),
    ("Git version control merge rebase cherry pick stash branch log", "Git"),
    ("Linux bash shell scripts environment variables piping redirect permissions", "OS"),
    ("MacOS system command line developer tools python pip virtualenv env", "OS"),
    ("Rust compiler rustc cargo packaging documentation rustdoc traits", "Rust"),
    ("Rust borrow checker lifetimes references mutability compiler checks", "Rust"),
    ("Error handling Result Option panic trace propagating unwrapping catch", "Rust"),
    ("Functional programming pattern matching closures map filter fold pure", "FP"),
    ("Object oriented design inheritance polymorphism encapsulation interface design", "OOP"),
    ("Software architecture microservices monorepos clean architecture DDD layers", "Arch"),
];

const QUERY_DEFS: &[(&str, &[usize])] = &[
    ("How do I configure TypeScript client UDS transport socket?", &[0]),
    ("TypeScript client socket test suite", &[1]),
    ("Python pyo3 dynamic plugin loader modules", &[2]),
    ("pyo3 plugin loader integration tests", &[3]),
    ("Graceful shutdown worker draining lifecycle SIGTERM", &[6]),
    ("Drop clean stale socket PID files cleanup", &[8]),
    ("Sqlite database connection pool driver migration", &[10]),
    ("EventLogRepository transaction safe stored events", &[12]),
    ("ProjectionCheckpointRepository state index manager checkpoint", &[14]),
    ("JobReadModel SQLite schema mapping jobs", &[16]),
    ("SessionCacheManager memory cache hydration on hit", &[18]),
    ("RRF ranking hybrid retriever score fusion candidates", &[26]),
    ("FeatureProvider recency access frequency temporal decay", &[28]),
    ("Grid search calibration optimization weights objective", &[30]),
    ("SensitivityAnalysis feature impact baseline diagnostics", &[32]),
    ("CorrelationEngine feature redundancy Pearson Spearman matrix", &[34]),
    ("PruningExperimentRunner prune node composite delta degradation", &[36]),
    ("LogisticRegressionModel L2 gradient descent weights intercept", &[38]),
    ("React frontend TailwindCSS web application styling design", &[40]),
    ("Next.js framework server side rendering routing API", &[42]),
    ("Docker container orchestration compose ports services", &[46]),
    ("Kubernetes cluster pods deployment scaling ingress", &[48]),
    ("GitHub Actions CI CD steps runner checkout workflow", &[50]),
    ("Performance profiling peak RSS latency frames CPU", &[54]),
    ("Tokio async runtime spawn event loop task join set", &[58]),
    ("Mutex RwLock thread safety Arc synchronization lock", &[60]),
    ("REST gRPC websocket HTTP protocol duplex connection", &[62, 64, 66]),
    ("Redis cache eviction TTL in-memory DB", &[68]),
    ("Elasticsearch inverted index tokenization analyzer search", &[70]),
    ("Kafka topic partition offset consumer group replication", &[72]),
];

/// Helper builder for constructing the production evaluation corpus.
pub struct ProductionCorpusBuilder;

impl ProductionCorpusBuilder {
    /// Builds and returns the complete deterministic production evaluation corpus.
    pub fn build() -> Result<ProductionCorpus, BrainError> {
        let test_storage = brain_storage::TestStorage::new();
        let storage = Arc::new(test_storage.storage().clone());

        let mut corpus_nodes = Vec::new();
        let mut expected_map = HashMap::new();

        // 1. Generate 100 nodes programmatically (50 topics * 2: Implementation vs Test)
        let mut node_ids = Vec::new();
        for (i, &(content, cat)) in TOPICS.iter().enumerate() {
            // Hex-padded deterministic UUID strings
            let impl_id_str = format!("00000000-0000-0000-0000-000000000{:03x}", i * 2);
            let test_id_str = format!("00000000-0000-0000-0000-000000000{:03x}", i * 2 + 1);

            let impl_id = NodeId(uuid::Uuid::parse_str(&impl_id_str).unwrap());
            let test_id = NodeId(uuid::Uuid::parse_str(&test_id_str).unwrap());

            node_ids.push(impl_id);
            node_ids.push(test_id);

            // Create implementation node (high importance)
            let mut impl_node = Node::new(
                impl_id,
                format!("{} implementation logic for {} system.", content, cat),
                NodeType::Concept,
            );
            impl_node.updated_at = 1600000000 + (i as u64) * 3600;
            impl_node.properties.insert("importance".to_string(), serde_json::json!(0.7 + (i % 4) as f64 * 0.1));
            impl_node.properties.insert("pinned".to_string(), serde_json::json!(i % 12 == 0));
            impl_node.properties.insert("provenance_confidence".to_string(), serde_json::json!(0.8 + (i % 3) as f64 * 0.1));

            // Create test node (lower importance)
            let mut test_node = Node::new(
                test_id,
                format!("{} integration test verification suite.", content),
                NodeType::Concept,
            );
            test_node.updated_at = 1600000000 + (i as u64) * 3600 + 1800;
            test_node.properties.insert("importance".to_string(), serde_json::json!(0.3 + (i % 3) as f64 * 0.1));
            test_node.properties.insert("pinned".to_string(), serde_json::json!(false));
            test_node.properties.insert("provenance_confidence".to_string(), serde_json::json!(0.6 + (i % 3) as f64 * 0.1));

            // Save to storage
            storage.nodes().save(&impl_node)?;
            storage.nodes().save(&test_node)?;

            // Save embeddings using the deterministic provider
            let impl_vector = HashingEmbeddingProvider.embed(&impl_node.label)?;
            let test_vector = HashingEmbeddingProvider.embed(&test_node.label)?;
            storage.embeddings().save(&brain_domain::Embedding::new(impl_id, impl_vector))?;
            storage.embeddings().save(&brain_domain::Embedding::new(test_id, test_vector))?;

            // Track for corpus JSON representation
            corpus_nodes.push(CorpusNode {
                node_id: impl_id_str,
                content: impl_node.label,
                node_type: "Concept".to_string(),
            });
            corpus_nodes.push(CorpusNode {
                node_id: test_id_str,
                content: test_node.label,
                node_type: "Concept".to_string(),
            });

            // Save "test-of" edges connecting tests to implementations
            let edge = Edge::new(test_id, impl_id, RelationKind::AssociatedWith, 1.0);
            storage.save_temporal_edge(&TemporalEdge {
                edge,
                observed_at: TimePoint::from_unix_seconds(1600000000 + (i as u64) * 3600),
                validity: TemporalValidity::new(vec![]),
            })?;

            // Access logs: frequently accessed implementation nodes
            if i % 3 == 0 {
                let access_count = (i % 6) + 1;
                let conn = storage.pool().get().map_err(|e| BrainError::Internal { message: e.to_string() })?;
                for log_idx in 0..access_count {
                    conn.execute(
                        "INSERT INTO feedback_events (id, schema_version, query, node_id, selected, timestamp, ranking_position, context) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        rusqlite::params![
                            format!("prod_event_{}_{}", i, log_idx),
                            1,
                            "realistic query search",
                            impl_id.to_string(),
                            1,
                            1600000000 + (i as u64) * 3600 + (log_idx as u64) * 600,
                            0,
                            "{}"
                        ],
                    ).map_err(|e| BrainError::Internal { message: e.to_string() })?;
                }
            }
        }

        // 2. Build queries corpus and ground truth
        let mut query_items = Vec::new();
        for (q_idx, &(query_text, matched_indices)) in QUERY_DEFS.iter().enumerate() {
            let query_id = format!("q_p{:03}", q_idx + 1);
            let embedding = HashingEmbeddingProvider.embed(query_text).ok();

            query_items.push(QueryItem {
                query_id: query_id.clone(),
                text: query_text.to_string(),
                tags: vec!["prod_test".to_string()],
                embedding,
            });

            let mut expected_node_ids = Vec::new();
            for &idx in matched_indices {
                if idx < node_ids.len() {
                    expected_node_ids.push(node_ids[idx].to_string());
                }
            }

            expected_map.insert(query_id, GroundTruthItem {
                expected_node_ids,
                acceptable_alternatives: vec![],
                minimum_rank: HashMap::new(),
                feature_under_test: None,
            });
        }

        let queries = QueryCorpus {
            version: 1,
            queries: query_items,
        };

        let ground_truth = GroundTruthCorpus {
            version: 1,
            nodes: corpus_nodes,
            ground_truth: expected_map,
        };

        // 3. Build RRF Hybrid Retriever
        let fts = FtsRetriever::new(storage.pool().clone());
        let semantic = SemanticRetriever::new(storage.pool().clone(), Arc::new(HashingEmbeddingProvider));
        let retriever = HybridRetriever::new(fts, semantic);
        let feature_provider = FeatureProvider::new(storage.pool().clone());

        Ok(ProductionCorpus {
            _storage: storage,
            queries,
            ground_truth,
            retriever,
            feature_provider,
        })
    }
}
