use crate::retrieval::eval_harness::FeatureContext;
use brain_core::errors::BrainError;
use brain_domain::NodeId;
use brain_storage::connection::SqliteConnectionManager;
use brain_storage::r2d2::Pool;
use std::collections::HashMap;

/// Immutable database snapshot context loader for ranking candidates.
pub struct FeatureProvider {
    pool: Pool<SqliteConnectionManager>,
}

impl FeatureProvider {
    /// Instantiates a new FeatureProvider.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Loads the `FeatureContext` for a batch of `NodeId`s in a single pass.
    pub fn load_contexts(
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

        // Format placeholders for IN clause (e.g. ?1, ?2, ?3...)
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

        let params =
            brain_storage::rusqlite::params_from_iter(node_ids.iter().map(|id| id.0.to_string()));

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

        for row_res in rows {
            let (id_str, updated_at, properties_json, graph_degree, access_count, last_observed_at) =
                row_res.map_err(|e| BrainError::Storage {
                    message: format!("Failed to parse batch metadata row: {}", e),
                    source: Some(Box::new(e)),
                })?;

            let uuid = uuid::Uuid::parse_str(&id_str).map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID in retrieved batch metadata: {}", e),
                source: Some(Box::new(e)),
            })?;
            let node_id = NodeId(uuid);

            let mut importance = None;
            let mut pinned = false;
            let mut provenance_confidence = None;
            if let Ok(props) =
                serde_json::from_str::<HashMap<String, serde_json::Value>>(&properties_json)
            {
                if let Some(imp) = props.get("importance").and_then(|v| v.as_f64()) {
                    importance = Some(imp);
                }
                if let Some(pin) = props.get("pinned").and_then(|v| v.as_bool()) {
                    pinned = pin;
                }
                if let Some(conf) = props.get("provenance_confidence").and_then(|v| v.as_f64()) {
                    provenance_confidence = Some(conf);
                }
            }

            contexts.insert(
                node_id,
                FeatureContext {
                    updated_at: Some(updated_at),
                    importance,
                    pinned,
                    provenance_confidence,
                    graph_degree: Some(graph_degree),
                    access_count: Some(access_count),
                    last_observed_at: Some(last_observed_at),
                },
            );
        }

        // For any nodes not found in DB, default to empty FeatureContext
        for id in node_ids {
            contexts.entry(*id).or_insert(FeatureContext {
                updated_at: None,
                importance: None,
                pinned: false,
                provenance_confidence: None,
                graph_degree: None,
                access_count: None,
                last_observed_at: None,
            });
        }

        Ok(contexts)
    }
}
