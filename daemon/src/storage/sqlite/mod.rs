use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use chrono::Utc;

use crate::plugins::StorageBackend;
use crate::storage::{ExtractedEdge, ExtractedNode};

pub mod graph;
pub mod schema;
pub mod stm;

pub struct LtmDatabase {
    pub conn: Arc<Mutex<Connection>>,
}

impl LtmDatabase {
    /// Initialize the SQLite database and create schemas/indexes.
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;

        // Enable Write-Ahead Logging (WAL) for high concurrency, fallback to DELETE if I/O error occurs
        if let Err(e) = conn.pragma_update(None, "journal_mode", "WAL") {
            println!(
                "[LTM Warning] Failed to enable WAL mode, falling back to DELETE: {}",
                e
            );
            conn.pragma_update(None, "journal_mode", "DELETE")?;
        }

        // Enable Foreign Keys enforcement
        conn.pragma_update(None, "foreign_keys", "ON")?;

        schema::initialize_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Expose the underlying Connection to run custom operations safely
    pub fn with_connection<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Connection) -> R,
    {
        let conn_guard = self.conn.lock().unwrap();
        f(&conn_guard)
    }

    /// Inserts an ingestion event into the SQLite event_log table.
    /// Performs deduplication by checking event_id. If duplicate, returns Ok(existing_sequence).
    pub fn insert_event(&self, envelope: &brain_integrations::IngestionEnvelope) -> Result<u64, rusqlite::Error> {
        let conn_guard = self.conn.lock().unwrap();

        // 1. Check for duplicates
        let event_id_str = envelope.identity.event_id.to_string();
        let mut check_stmt = conn_guard.prepare("SELECT sequence FROM event_log WHERE event_id = ?1")?;
        let mut rows = check_stmt.query(params![event_id_str])?;
        if let Some(row) = rows.next()? {
            let seq: i64 = row.get(0)?;
            return Ok(seq as u64);
        }

        // 2. Format columns
        let adapter_id_str = envelope.identity.adapter_id.to_string();
        let client_id_str = envelope.identity.client_id.to_string();
        let session_id_str = envelope.identity.session_id.to_string();
        let workspace_id_str = envelope.identity.workspace_id.to_string();
        let conversation_id_str = envelope.identity.conversation_id.map(|id| id.to_string());
        let event_model_version = envelope.event_model_version.clone();
        let event_type = serde_json::to_value(&envelope.event.kind())
            .map(|v| v.as_str().unwrap_or("unknown").to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let payload = brain_integrations::to_canonical_json(envelope)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let timestamp_str = envelope.identity.timestamp.to_rfc3339();
        let received_at_str = Utc::now().to_rfc3339();

        // 3. Insert and retrieve auto-increment sequence ID
        conn_guard.execute(
            "INSERT INTO event_log (event_id, adapter_id, client_id, session_id, workspace_id, conversation_id, event_model_version, event_type, payload, timestamp, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                event_id_str,
                adapter_id_str,
                client_id_str,
                session_id_str,
                workspace_id_str,
                conversation_id_str,
                event_model_version,
                event_type,
                payload,
                timestamp_str,
                received_at_str
            ],
        )?;

        let sequence = conn_guard.last_insert_rowid() as u64;
        Ok(sequence)
    }

    /// Checks if the event_id already exists in the log.
    pub fn is_duplicate_event(&self, event_id: &brain_domain::EventId) -> Result<bool, rusqlite::Error> {
        let conn_guard = self.conn.lock().unwrap();
        let mut stmt = conn_guard.prepare("SELECT 1 FROM event_log WHERE event_id = ?1")?;
        let exists = stmt.exists(params![event_id.to_string()])?;
        Ok(exists)
    }

    /// Replays events starting after the given sequence number.
    pub fn get_events_after(&self, sequence: u64) -> Result<Vec<brain_integrations::IngestionEnvelope>, rusqlite::Error> {
        let conn_guard = self.conn.lock().unwrap();
        let mut stmt = conn_guard.prepare("SELECT payload FROM event_log WHERE sequence > ?1 ORDER BY sequence ASC")?;
        
        let rows = stmt.query_map(params![sequence], |row| {
            let payload_str: String = row.get(0)?;
            Ok(payload_str)
        })?;

        let mut envelopes = Vec::new();
        for row in rows {
            let payload_str = row?;
            let envelope: brain_integrations::IngestionEnvelope = serde_json::from_str(&payload_str)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }
}

impl StorageBackend for LtmDatabase {
    fn name(&self) -> &str {
        "sqlite"
    }

    fn event_log(&self) -> Option<&dyn crate::plugins::traits::EventLogRepository> {
        Some(self)
    }

    fn write_graph(&self, nodes: &[ExtractedNode], edges: &[ExtractedEdge]) -> Result<(), String> {
        self.upsert_nodes_and_edges(nodes, edges)
            .map_err(|e| e.to_string())
    }

    fn query_graph(&self, query: &str) -> Result<Vec<(ExtractedNode, Vec<ExtractedEdge>)>, String> {
        self.query_ltm(query).map_err(|e| e.to_string())
    }

    fn get_updates_since(
        &self,
        timestamp: i64,
    ) -> Result<(Vec<ExtractedNode>, Vec<ExtractedEdge>, i64), String> {
        let conn_guard = self.conn.lock().unwrap();

        let mut node_stmt = conn_guard
            .prepare(
                "SELECT id, label, type, properties, updated_at FROM nodes WHERE updated_at > ?1",
            )
            .map_err(|e| e.to_string())?;

        let node_iter = node_stmt
            .query_map(params![timestamp], |row| {
                let props_str: String = row.get(3)?;
                let attributes = serde_json::from_str(&props_str)
                    .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
                Ok((
                    ExtractedNode {
                        id: row.get(0)?,
                        label: row.get(1)?,
                        node_type: row.get(2)?,
                        attributes,
                    },
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut nodes = Vec::new();
        let mut max_ts = timestamp;
        for node_res in node_iter {
            let (node, ts) = node_res.map_err(|e| e.to_string())?;
            if ts > max_ts {
                max_ts = ts;
            }
            nodes.push(node);
        }

        let mut edge_stmt = conn_guard
            .prepare("SELECT source, target, relation, updated_at FROM edges WHERE updated_at > ?1")
            .map_err(|e| e.to_string())?;

        let edge_iter = edge_stmt
            .query_map(params![timestamp], |row| {
                Ok((
                    ExtractedEdge {
                        source: row.get(0)?,
                        target: row.get(1)?,
                        relation: row.get(2)?,
                    },
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut edges = Vec::new();
        for edge_res in edge_iter {
            let (edge, ts) = edge_res.map_err(|e| e.to_string())?;
            if ts > max_ts {
                max_ts = ts;
            }
            edges.push(edge);
        }

        Ok((nodes, edges, max_ts))
    }

    fn decay_weights(&self, half_life_secs: f64, threshold: f64) -> Result<(), String> {
        self.decay_relationships(half_life_secs, threshold)
            .map_err(|e| e.to_string())
    }

    fn write_embeddings(&self, embeddings: &[(String, Vec<f32>)]) -> Result<(), String> {
        let mut conn_guard = self.conn.lock().unwrap();
        let tx = conn_guard.transaction().map_err(|e| e.to_string())?;
        let centroids = get_centroids();
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO node_embeddings (node_id, embedding, centroid_id)
                  VALUES (?1, ?2, ?3)
                  ON CONFLICT(node_id) DO UPDATE SET embedding = excluded.embedding, centroid_id = excluded.centroid_id",
                )
                .map_err(|e| e.to_string())?;

            for (node_id, emb) in embeddings {
                let bytes = embedding_to_bytes(emb);
                let mut best_centroid = 0;
                let mut best_sim = -2.0;
                for (c_idx, centroid) in centroids.iter().enumerate() {
                    let sim = cosine_similarity(emb, centroid);
                    if sim > best_sim {
                        best_sim = sim;
                        best_centroid = c_idx;
                    }
                }
                stmt.execute(params![node_id, bytes, best_centroid])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn query_nearest_neighbors(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        let conn_guard = self.conn.lock().unwrap();

        let total_count: i64 = conn_guard
            .query_row("SELECT COUNT(*) FROM node_embeddings", [], |row| row.get(0))
            .unwrap_or(0);

        let mut candidates = Vec::new();

        if total_count < 50 {
            let mut stmt = conn_guard
                .prepare("SELECT node_id, embedding FROM node_embeddings")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    let node_id: String = row.get(0)?;
                    let bytes: Vec<u8> = row.get(1)?;
                    Ok((node_id, bytes))
                })
                .map_err(|e| e.to_string())?;

            for r in rows {
                let (node_id, bytes) = r.map_err(|e| e.to_string())?;
                let emb = bytes_to_embedding(&bytes)?;
                if emb.len() == query_embedding.len() {
                    let sim = cosine_similarity(query_embedding, &emb);
                    candidates.push((node_id, sim));
                }
            }
        } else {
            let centroids = get_centroids();
            let mut centroid_sims: Vec<(usize, f32)> = centroids
                .iter()
                .enumerate()
                .map(|(idx, c)| (idx, cosine_similarity(query_embedding, c)))
                .collect();
            centroid_sims
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let c1 = centroid_sims[0].0 as i64;
            let c2 = centroid_sims[1].0 as i64;

            let mut stmt = conn_guard
                .prepare(
                    "SELECT node_id, embedding FROM node_embeddings WHERE centroid_id IN (?1, ?2)",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![c1, c2], |row| {
                    let node_id: String = row.get(0)?;
                    let bytes: Vec<u8> = row.get(1)?;
                    Ok((node_id, bytes))
                })
                .map_err(|e| e.to_string())?;

            for r in rows {
                let (node_id, bytes) = r.map_err(|e| e.to_string())?;
                let emb = bytes_to_embedding(&bytes)?;
                if emb.len() == query_embedding.len() {
                    let sim = cosine_similarity(query_embedding, &emb);
                    candidates.push((node_id, sim));
                }
            }
        }

        // Sort by similarity score descending
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(limit);
        Ok(candidates)
    }

    fn get_connections(&self, node_ids: &[String]) -> Result<Vec<ExtractedEdge>, String> {
        let conn_guard = self.conn.lock().unwrap();
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; node_ids.len()].join(", ");
        let query_str = format!(
            "SELECT source, target, relation FROM edges WHERE source IN ({}) OR target IN ({})",
            placeholders, placeholders
        );

        let mut stmt = conn_guard.prepare(&query_str).map_err(|e| e.to_string())?;

        let mut params: Vec<rusqlite::types::Value> = Vec::new();
        for id in node_ids {
            params.push(rusqlite::types::Value::Text(id.clone()));
        }
        for id in node_ids {
            params.push(rusqlite::types::Value::Text(id.clone()));
        }

        let edge_iter = stmt
            .query_map(rusqlite::params_from_iter(params), |row| {
                Ok(ExtractedEdge {
                    source: row.get(0)?,
                    target: row.get(1)?,
                    relation: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut edges = Vec::new();
        for edge_res in edge_iter {
            edges.push(edge_res.map_err(|e| e.to_string())?);
        }
        Ok(edges)
    }

    fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<ExtractedNode>, String> {
        let conn_guard = self.conn.lock().unwrap();
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; ids.len()].join(", ");
        let query_str = format!(
            "SELECT id, label, type, properties FROM nodes WHERE id IN ({})",
            placeholders
        );

        let mut stmt = conn_guard.prepare(&query_str).map_err(|e| e.to_string())?;

        let params: Vec<rusqlite::types::Value> = ids
            .iter()
            .map(|id| rusqlite::types::Value::Text(id.clone()))
            .collect();

        let node_iter = stmt
            .query_map(rusqlite::params_from_iter(params), |row| {
                let props_str: String = row.get(3)?;
                let attributes = serde_json::from_str(&props_str)
                    .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
                Ok(ExtractedNode {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    node_type: row.get(2)?,
                    attributes,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut nodes = Vec::new();
        for node_res in node_iter {
            nodes.push(node_res.map_err(|e| e.to_string())?);
        }
        Ok(nodes)
    }

    fn apply_kpp_ops(&self, ops: &[brain_domain::bkf::SqliteOp]) -> Result<(), String> {
        let mut conn_guard = self.conn.lock().unwrap();
        let tx = conn_guard.transaction().map_err(|e| e.to_string())?;

        for op in ops {
            match op {
                brain_domain::bkf::SqliteOp::Node(delta) => match delta {
                    brain_domain::bkf::ProjectionDelta::Insert(node) => {
                        let props_json = serde_json::to_string(&node.attributes).unwrap_or_default();
                        tx.execute(
                            "INSERT INTO nodes (id, label, type, properties, updated_at, lifecycle, validity, version_state) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                             ON CONFLICT(id) DO UPDATE SET \
                             label=excluded.label, \
                             type=excluded.type, \
                             properties=excluded.properties, \
                             updated_at=excluded.updated_at, \
                             lifecycle=excluded.lifecycle, \
                             validity=excluded.validity, \
                             version_state=excluded.version_state",
                            (
                                &node.id,
                                &node.label,
                                &node.entity_type,
                                &props_json,
                                Utc::now().timestamp(),
                                format!("{:?}", node.lifecycle),
                                format!("{:?}", node.validity),
                                format!("{:?}", node.version_state),
                            ),
                        ).map_err(|e| e.to_string())?;
                    }
                    brain_domain::bkf::ProjectionDelta::Update { id, changes } => {
                        let props_json = serde_json::to_string(&changes.attributes).unwrap_or_default();
                        tx.execute(
                            "UPDATE nodes SET label = ?1, type = ?2, properties = ?3, updated_at = ?4, lifecycle = ?5, validity = ?6, version_state = ?7 WHERE id = ?8",
                            (
                                &changes.label,
                                &changes.entity_type,
                                &props_json,
                                Utc::now().timestamp(),
                                format!("{:?}", changes.lifecycle),
                                format!("{:?}", changes.validity),
                                format!("{:?}", changes.version_state),
                                id,
                            ),
                        ).map_err(|e| e.to_string())?;
                    }
                    brain_domain::bkf::ProjectionDelta::Delete(id) => {
                        tx.execute("DELETE FROM nodes WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
                    }
                },
                brain_domain::bkf::SqliteOp::Edge(delta) => match delta {
                    brain_domain::bkf::ProjectionDelta::Insert(edge) => {
                        tx.execute(
                            "INSERT INTO edges (source, target, relation, weight, updated_at, lifecycle, version_state) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                             ON CONFLICT(source, target, relation) DO UPDATE SET \
                             weight=excluded.weight, \
                             lifecycle=excluded.lifecycle, \
                             version_state=excluded.version_state",
                            (
                                &edge.source,
                                &edge.target,
                                &edge.relation,
                                edge.weight,
                                Utc::now().timestamp(),
                                format!("{:?}", edge.lifecycle),
                                format!("{:?}", edge.version_state),
                            ),
                        ).map_err(|e| e.to_string())?;
                    }
                    brain_domain::bkf::ProjectionDelta::Update { id: _, changes } => {
                        tx.execute(
                            "UPDATE edges SET weight = ?1, lifecycle = ?2, version_state = ?3 WHERE source = ?4 AND target = ?5 AND relation = ?6",
                            (
                                changes.weight,
                                format!("{:?}", changes.lifecycle),
                                format!("{:?}", changes.version_state),
                                &changes.source,
                                &changes.target,
                                &changes.relation,
                            ),
                        ).map_err(|e| e.to_string())?;
                    }
                    brain_domain::bkf::ProjectionDelta::Delete(id) => {
                        tx.execute(
                            "DELETE FROM edges WHERE (source || '-' || target || '-' || LOWER(relation)) = ?1",
                            [id],
                        ).map_err(|e| e.to_string())?;
                    }
                },
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn log_kpp_event(&self, event: &brain_domain::DomainEvent) -> Result<(), String> {
        let event_id = uuid::Uuid::new_v4().to_string();
        let _correlation_id = uuid::Uuid::new_v4().to_string();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let payload_json = serde_json::to_string(event).map_err(|e| e.to_string())?;

        let conn_guard = self.conn.lock().unwrap();
        conn_guard.execute(
            "INSERT INTO event_log (event_id, adapter_id, client_id, session_id, workspace_id, conversation_id, event_model_version, event_type, payload, timestamp, received_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            (
                &event_id,
                "KPP",
                "KPP_CLIENT",
                "KPP_SESSION",
                "KPP_WORKSPACE",
                None::<String>,
                "1.0",
                "core",
                &payload_json,
                &timestamp_ms.to_string(),
                &timestamp_ms.to_string(),
            ),
        ).map_err(|e| e.to_string())?;

        Ok(())
    }
}

impl crate::plugins::traits::EventLogRepository for LtmDatabase {
    fn insert_event(&self, envelope: &brain_integrations::IngestionEnvelope) -> Result<u64, String> {
        self.insert_event(envelope).map_err(|e| e.to_string())
    }
    fn is_duplicate_event(&self, event_id: &brain_domain::EventId) -> Result<bool, String> {
        self.is_duplicate_event(event_id).map_err(|e| e.to_string())
    }
    fn get_events_after(&self, sequence: u64) -> Result<Vec<brain_integrations::IngestionEnvelope>, String> {
        self.get_events_after(sequence).map_err(|e| e.to_string())
    }
}

// ==========================================
// Vector & Embedding Math Helpers
// ==========================================

fn embedding_to_bytes(emb: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(emb.len() * 4);
    for &val in emb {
        bytes.extend_from_slice(&val.to_ne_bytes());
    }
    bytes
}

fn bytes_to_embedding(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("Invalid byte length for embedding".to_string());
    }
    let mut emb = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let array = chunk
            .try_into()
            .map_err(|_| "Failed to parse chunk".to_string())?;
        emb.push(f32::from_ne_bytes(array));
    }
    Ok(emb)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let rem_a = chunks_a.remainder();
    let rem_b = chunks_b.remainder();

    for (ca, cb) in chunks_a.zip(chunks_b) {
        dot += ca[0] * cb[0] + ca[1] * cb[1] + ca[2] * cb[2] + ca[3] * cb[3];
        norm_a += ca[0] * ca[0] + ca[1] * ca[1] + ca[2] * ca[2] + ca[3] * ca[3];
        norm_b += cb[0] * cb[0] + cb[1] * cb[1] + cb[2] * cb[2] + cb[3] * cb[3];
    }

    for (x, y) in rem_a.iter().zip(rem_b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

fn get_centroids() -> Vec<Vec<f32>> {
    let mut centroids = Vec::new();
    for c in 0..8 {
        let mut v = vec![0.0; 384];
        let mut norm = 0.0;
        for (j, val_ref) in v.iter_mut().enumerate() {
            let val = ((c as f32 + 1.0) * (j as f32 + 1.0)).sin();
            *val_ref = val;
            norm += val * val;
        }
        let sqrt_norm = norm.sqrt();
        if sqrt_norm > 0.0 {
            for val_ref in &mut v {
                *val_ref /= sqrt_norm;
            }
        }
        centroids.push(v);
    }
    centroids
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_ltm_setup_and_upsert() {
        let db = LtmDatabase::new_in_memory().unwrap();

        let node1 = ExtractedNode {
            id: "sqlite".to_string(),
            label: "SQLite".to_string(),
            node_type: "technology".to_string(),
            attributes: serde_json::json!({ "engine": "SQLite" }),
        };

        let node2 = ExtractedNode {
            id: "db-config".to_string(),
            label: "Database Configuration".to_string(),
            node_type: "configuration".to_string(),
            attributes: serde_json::json!({}),
        };

        let edge = ExtractedEdge {
            source: "db-config".to_string(),
            target: "sqlite".to_string(),
            relation: "configures".to_string(),
        };

        db.upsert_nodes_and_edges(&[node1, node2], &[edge]).unwrap();

        let results = db.query_ltm("sqlite").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "sqlite");
        assert_eq!(results[0].1.len(), 1);
        assert_eq!(results[0].1[0].source, "db-config");
        assert_eq!(results[0].1[0].target, "sqlite");
        assert_eq!(results[0].1[0].relation, "configures");
    }

    #[test]
    fn test_ltm_node_overwrite() {
        let db = LtmDatabase::new_in_memory().unwrap();

        let node_v1 = ExtractedNode {
            id: "rust".to_string(),
            label: "Rust Lang".to_string(),
            node_type: "language".to_string(),
            attributes: serde_json::json!({ "version": "1.70" }),
        };
        db.upsert_nodes_and_edges(&[node_v1], &[]).unwrap();

        let node_v2 = ExtractedNode {
            id: "rust".to_string(),
            label: "Rust".to_string(),
            node_type: "language".to_string(),
            attributes: serde_json::json!({ "version": "1.78", "speed": "fast" }),
        };
        db.upsert_nodes_and_edges(&[node_v2], &[]).unwrap();

        let results = db.query_ltm("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.label, "Rust");
        assert_eq!(results[0].0.attributes["version"], "1.78");
        assert_eq!(results[0].0.attributes["speed"], "fast");
    }

    #[test]
    fn test_edge_weight_decay_and_pruning() {
        let db = LtmDatabase::new_in_memory().unwrap();

        let node1 = ExtractedNode {
            id: "nodea".to_string(),
            label: "Node A".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        };

        let node2 = ExtractedNode {
            id: "nodeb".to_string(),
            label: "Node B".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        };

        let edge = ExtractedEdge {
            source: "nodea".to_string(),
            target: "nodeb".to_string(),
            relation: "linked_to".to_string(),
        };

        db.upsert_nodes_and_edges(&[node1.clone(), node2.clone()], std::slice::from_ref(&edge))
            .unwrap();

        db.with_connection(|conn| {
            let mut stmt = conn
                .prepare("SELECT weight FROM edges WHERE source='nodea'")
                .unwrap();
            let mut rows = stmt.query([]).unwrap();
            let w: f64 = rows.next().unwrap().unwrap().get(0).unwrap();
            assert!((w - 1.5).abs() < 0.001);
        });

        db.upsert_nodes_and_edges(&[], std::slice::from_ref(&edge))
            .unwrap();
        db.with_connection(|conn| {
            let mut stmt = conn
                .prepare("SELECT weight FROM edges WHERE source='nodea'")
                .unwrap();
            let mut rows = stmt.query([]).unwrap();
            let w: f64 = rows.next().unwrap().unwrap().get(0).unwrap();
            assert!((w - 2.0).abs() < 0.001);
        });

        db.with_connection(|conn| {
            let seven_days_ago = (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                - 604800) as i64;
            conn.execute(
                "UPDATE edges SET updated_at = ?1 WHERE source='nodea'",
                params![seven_days_ago],
            )
            .unwrap();
        });

        db.decay_relationships(604800.0, 0.1).unwrap();
        db.with_connection(|conn| {
            let mut stmt = conn
                .prepare("SELECT weight FROM edges WHERE source='nodea'")
                .unwrap();
            let mut rows = stmt.query([]).unwrap();
            let w: f64 = rows.next().unwrap().unwrap().get(0).unwrap();
            assert!((w - 1.0).abs() < 0.05);
        });

        db.with_connection(|conn| {
            let thirty_days_ago = (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                - 30 * 86400) as i64;
            conn.execute(
                "UPDATE edges SET updated_at = ?1 WHERE source='nodea'",
                params![thirty_days_ago],
            )
            .unwrap();
        });

        db.decay_relationships(604800.0, 0.1).unwrap();
        db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT COUNT(*) FROM edges").unwrap();
            let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
            assert_eq!(count, 0);
        });
    }

    #[test]
    fn test_sqlite_vector_search_and_graph_expansion() {
        let db = LtmDatabase::new_in_memory().unwrap();

        let node1 = ExtractedNode {
            id: "node1".to_string(),
            label: "Node 1".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        };
        let node2 = ExtractedNode {
            id: "node2".to_string(),
            label: "Node 2".to_string(),
            node_type: "concept".to_string(),
            attributes: serde_json::json!({}),
        };
        let edge = ExtractedEdge {
            source: "node1".to_string(),
            target: "node2".to_string(),
            relation: "rel".to_string(),
        };
        db.upsert_nodes_and_edges(&[node1, node2], &[edge]).unwrap();

        let emb1 = vec![1.0, 0.0, 0.0];
        let emb2 = vec![0.0, 1.0, 0.0];
        db.write_embeddings(&[
            ("node1".to_string(), emb1.clone()),
            ("node2".to_string(), emb2.clone()),
        ])
        .unwrap();

        // Query nearest neighbor for vec![0.9, 0.1, 0.0]
        let neighbors = db.query_nearest_neighbors(&[0.9, 0.1, 0.0], 5).unwrap();
        assert_eq!(neighbors.len(), 2);
        assert_eq!(neighbors[0].0, "node1");
        assert!(neighbors[0].1 > 0.8);

        // Test get_connections
        let connections = db.get_connections(&["node1".to_string()]).unwrap();
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].source, "node1");
        assert_eq!(connections[0].target, "node2");
    }

    #[test]
    fn test_sqlite_ivf_vector_search() {
        let db = LtmDatabase::new_in_memory().unwrap();

        // Ingest 60 mock nodes to cross the IVF threshold
        let mut nodes = Vec::new();
        let mut embeddings = Vec::new();
        for i in 0..60 {
            let id = format!("node_{}", i);
            nodes.push(ExtractedNode {
                id: id.clone(),
                label: format!("Node {}", i),
                node_type: "concept".to_string(),
                attributes: serde_json::json!({}),
            });
            let mut emb = vec![0.0; 384];
            emb[i % 384] = 1.0;
            embeddings.push((id, emb));
        }
        db.upsert_nodes_and_edges(&nodes, &[]).unwrap();
        db.write_embeddings(&embeddings).unwrap();

        // Query with a non-zero vector. It should execute the IVF branch (count >= 50) and return nearest neighbors
        let query = vec![1.0; 384];
        let neighbors = db.query_nearest_neighbors(&query, 5).unwrap();
        assert!(!neighbors.is_empty());
    }

    #[test]
    fn test_ltm_node_merge_stub() {
        let db = LtmDatabase::new_in_memory().unwrap();

        // 1. Insert a stub node (as if created by insert OR ignore of edges)
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO nodes (id, label, type, properties, updated_at) VALUES ('rust', 'rust', 'stub', '{\"initial\":true}', 100)",
                [],
            ).unwrap();
        });

        // 2. Upsert a real node over it
        let real_node = ExtractedNode {
            id: "rust".to_string(),
            label: "Rust".to_string(),
            node_type: "language".to_string(),
            attributes: serde_json::json!({ "version": "1.78" }),
        };
        db.upsert_nodes_and_edges(&[real_node], &[]).unwrap();

        let results = db.query_ltm("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.node_type, "language"); // Should be updated from stub
        assert_eq!(results[0].0.attributes["initial"], true); // Should be merged
        assert_eq!(results[0].0.attributes["version"], "1.78");
    }

    #[test]
    fn test_ltm_node_preserve_type() {
        let db = LtmDatabase::new_in_memory().unwrap();

        let node_v1 = ExtractedNode {
            id: "rust".to_string(),
            label: "Rust Lang".to_string(),
            node_type: "language".to_string(),
            attributes: serde_json::json!({}),
        };
        db.upsert_nodes_and_edges(&[node_v1], &[]).unwrap();

        let node_v2 = ExtractedNode {
            id: "rust".to_string(),
            label: "Rust".to_string(),
            node_type: "framework".to_string(),
            attributes: serde_json::json!({}),
        };
        db.upsert_nodes_and_edges(&[node_v2], &[]).unwrap();

        let results = db.query_ltm("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.node_type, "language"); // Should NOT be changed to framework
    }
}
