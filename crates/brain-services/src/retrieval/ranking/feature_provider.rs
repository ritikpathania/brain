//! Feature provider module.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use brain_core::errors::BrainError;
use brain_domain::NodeId;
use r2d2::Pool;
use brain_storage::connection::SqliteConnectionManager;

/// The current feature schema version.
///
/// This version must be incremented whenever the set of features in
/// [`FeatureVector`] changes (fields added, removed, or semantically redefined).
/// It is embedded as both the fingerprint version prefix byte and in
/// [`ModelMetadata`](crate::retrieval::model_loader::ModelMetadata) to enable
/// compatibility checks before loading a trained model.
pub const FEATURE_SCHEMA_VERSION: u32 = 1;

/// Representation of extracted features for a candidate retrieved memory node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureVector {
    /// Score from lexical Full-Text Search, if discovered via FTS.
    pub lexical_similarity: Option<f64>,
    /// Score from vector semantic similarity, if discovered via Semantic search.
    pub semantic_similarity: Option<f64>,
    /// Temporal recency score (decayed updated_at delta).
    pub recency: Option<f64>,
    /// Combined static importance and pinning flag.
    pub importance: Option<f64>,
    /// Confidence of the source ingestion provenance.
    pub provenance_confidence: Option<f64>,
    /// Log-scaled graph degree.
    pub graph_degree: Option<f64>,
    /// Log-scaled access frequency.
    pub access_frequency: Option<f64>,
    /// Freshness decay (decayed last_observed_at delta).
    pub freshness_decay: Option<f64>,
}

impl FeatureVector {
    /// Computes a portable, versioned SHA-256 fingerprint for this FeatureVector.
    pub fn fingerprint(&self) -> String {
        // Serialize to a deterministic JSON byte array representation
        let json_bytes = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        // Version prefix: must match FEATURE_SCHEMA_VERSION
        hasher.update(&[FEATURE_SCHEMA_VERSION as u8]);
        hasher.update(&json_bytes);
        let result = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for byte in result {
            use std::fmt::Write;
            write!(&mut hex, "{:02x}", byte).unwrap();
        }
        hex
    }
}

/// Half-life configuration parameters for exponential time decays.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RankingDecay {
    /// Half-life for updated_at recency decay in days.
    pub recency_half_life_days: f64,
    /// Half-life for freshness decay (based on last edge observation) in days.
    pub freshness_half_life_days: f64,
}

impl Default for RankingDecay {
    fn default() -> Self {
        Self {
            recency_half_life_days: 7.0,   // 1 week half-life
            freshness_half_life_days: 1.0, // 1 day half-life
        }
    }
}

/// Immutable database snapshot context used to construct a feature vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureContext {
    /// Timestamp when the node was updated.
    pub updated_at: Option<u64>,
    /// Importance score assigned to the node.
    pub importance: Option<f64>,
    /// Pinned indicator.
    pub pinned: bool,
    /// Provenance confidence score.
    pub provenance_confidence: Option<f64>,
    /// Number of edges connected to this node in the graph.
    pub graph_degree: Option<u32>,
    /// Ingestion/Interaction selection count.
    pub access_count: Option<u64>,
    /// Most recent observed timestamp across all connecting edges.
    pub last_observed_at: Option<u64>,
}

/// First-class runtime contract for retrieving candidate metadata.
pub trait FeatureProvider: Send + Sync {
    /// Loads the `FeatureContext` for a batch of `NodeId`s.
    fn load_contexts(&self, node_ids: &[NodeId]) -> Result<HashMap<NodeId, FeatureContext>, BrainError>;
}

/// Pure translation layer extracting raw features from database context and similarity scores.
pub struct FeatureExtractor {
    /// Reference time point used to calculate age deltas.
    pub reference_time: u64,
    /// Exponential decay parameters.
    pub decay: RankingDecay,
}

impl FeatureExtractor {
    /// Instantiates a new FeatureExtractor with reference time and decay parameters.
    pub fn new(reference_time: u64, decay: RankingDecay) -> Self {
        Self { reference_time, decay }
    }

    /// Purely extracts a FeatureVector from similarity scores and database context.
    pub fn extract(
        &self,
        lexical_similarity: Option<f64>,
        semantic_similarity: Option<f64>,
        context: &FeatureContext,
    ) -> FeatureVector {
        // 1. Recency Decay
        let recency = context.updated_at.map(|updated_at| {
            let dt = (self.reference_time.saturating_sub(updated_at)) as f64;
            let half_life_sec = self.decay.recency_half_life_days * 86400.0;
            if half_life_sec <= 0.0 {
                1.0
            } else {
                let tau = half_life_sec / 2.0f64.ln();
                (-dt / tau).exp()
            }
        });

        // 2. Importance
        let importance = if context.pinned {
            Some(1.0)
        } else {
            context.importance.or(Some(0.0))
        };

        // 3. Provenance Confidence
        let provenance_confidence = context.provenance_confidence.or(Some(1.0));

        // 4. Log-scaled Graph Degree
        let graph_degree = context.graph_degree.map(|degree| (degree as f64 + 1.0).ln());

        // 5. Log-scaled Access Frequency
        let access_frequency = context.access_count.map(|count| (count as f64 + 1.0).ln());

        // 6. Freshness Decay
        let freshness_decay = context.last_observed_at.map(|last_observed| {
            let dt = (self.reference_time.saturating_sub(last_observed)) as f64;
            let half_life_sec = self.decay.freshness_half_life_days * 86400.0;
            if half_life_sec <= 0.0 {
                1.0
            } else {
                let tau = half_life_sec / 2.0f64.ln();
                (-dt / tau).exp()
            }
        });

        FeatureVector {
            lexical_similarity,
            semantic_similarity,
            recency,
            importance,
            provenance_confidence,
            graph_degree,
            access_frequency,
            freshness_decay,
        }
    }
}

/// Durable SQLite connection pool implementation of the FeatureProvider trait.
pub struct SqliteFeatureProvider {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteFeatureProvider {
    /// Instantiates a new SqliteFeatureProvider with a connection pool.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }
}

impl FeatureProvider for SqliteFeatureProvider {
    fn load_contexts(
        &self,
        node_ids: &[NodeId],
    ) -> Result<HashMap<NodeId, FeatureContext>, BrainError> {
        let mut contexts = HashMap::new();
        if node_ids.is_empty() {
            return Ok(contexts);
        }

        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to acquire connection for feature provider: {}", e),
            source: Some(Box::new(e)),
        })?;

        let placeholders: Vec<String> = (1..=node_ids.len()).map(|i| format!("?{}", i)).collect();
        let query_str = format!(
            "SELECT n.id, n.updated_at, n.properties, \
             (SELECT COUNT(*) FROM edges e WHERE e.source = n.id OR e.target = n.id) AS graph_degree, \
             (SELECT COUNT(*) FROM feedback_events f WHERE f.node_id = n.id) AS access_count, \
             (SELECT COALESCE(MAX(e2.observed_at), 0) FROM edges e2 WHERE e2.source = n.id OR e2.target = n.id) AS last_observed_at \
             FROM nodes n WHERE n.id IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&query_str).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare batch metadata statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let params = rusqlite::params_from_iter(node_ids.iter().map(|id| id.0.to_string()));

        let rows = stmt
            .query_map(params, |row| {
                let id_str: String = row.get(0)?;
                let updated_at: u64 = row.get(1)?;
                let properties_json: String = row.get(2)?;
                let graph_degree: u32 = row.get(3)?;
                let access_count: u64 = row.get(4)?;
                let last_observed_at: u64 = row.get(5)?;

                Ok((
                    id_str,
                    updated_at,
                    properties_json,
                    graph_degree,
                    access_count,
                    last_observed_at,
                ))
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query batch metadata: {}", e),
                source: Some(Box::new(e)),
            })?;

        for row in rows {
            if let Ok((
                id_str,
                updated_at,
                properties_json,
                graph_degree,
                access_count,
                last_observed_at,
            )) = row
            {
                if let Ok(uuid) = uuid::Uuid::parse_str(&id_str) {
                    let node_id = NodeId(uuid);

                    let mut pinned = false;
                    let mut importance = None;
                    if let Ok(props) =
                        serde_json::from_str::<serde_json::Value>(&properties_json)
                    {
                        pinned = props
                            .get("pinned")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        importance = props.get("importance").and_then(|v| v.as_f64());
                    }

                    let ctx = FeatureContext {
                        updated_at: Some(updated_at),
                        importance,
                        pinned,
                        provenance_confidence: None,
                        graph_degree: Some(graph_degree),
                        access_count: Some(access_count),
                        last_observed_at: Some(last_observed_at),
                    };
                    contexts.insert(node_id, ctx);
                }
            }
        }

        Ok(contexts)
    }
}
