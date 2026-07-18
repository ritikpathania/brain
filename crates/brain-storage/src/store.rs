use crate::connection::init_pool;
use crate::event_log::EventLogRepository;
use crate::migrations::run_migrations;
use brain_core::errors::BrainError;
use brain_core::repositories::{
    ConfigRepository, EdgeRepository, EmbeddingRepository, NodeRepository, RepositorySet,
    SessionRepository, Storage, StorageTransaction,
};
use brain_domain::{
    Edge, EdgeId, Embedding, Node, NodeId, NodeKind, NodeType, RelationKind, Session, SessionId,
};
use brain_integrations::IngestionEnvelope;
use std::collections::HashMap;

thread_local! {
    static TRANSACTION_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// SQLite database storage backend implementing all domain repositories.
#[derive(Clone)]
pub struct SqliteStorage {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteStorage {
    /// Initializes the SQLite connection pool and runs database schema migrations.
    pub fn new(path: &str, pool_size: u32, enable_wal: bool) -> Result<Self, BrainError> {
        let pool = init_pool(path, pool_size, enable_wal)?;
        let mut conn = pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection for migrations: {}", e),
            source: Some(Box::new(e)),
        })?;
        run_migrations(&mut conn)?;
        Ok(Self { pool })
    }

    /// Constructs a storage instance directly from an existing pool.
    pub fn from_pool(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Exposes the underlying connection pool.
    pub fn pool(&self) -> &r2d2::Pool<crate::connection::SqliteConnectionManager> {
        &self.pool
    }

    /// Executes the given closure in a single transaction.
    /// Commits on success, rolls back on failure or panic.
    pub fn run_transaction<F, R>(&self, f: F) -> Result<R, BrainError>
    where
        F: FnOnce(&dyn StorageTransaction) -> Result<R, BrainError>,
    {
        if TRANSACTION_ACTIVE.with(|active| active.get()) {
            return Err(BrainError::Storage {
                message: "Nested transactions are not supported".to_string(),
                source: None,
            });
        }

        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection for transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        // SQLite does not support nested transactions natively.
        // If autocommit is false, a transaction is already active.
        if !conn.is_autocommit() {
            return Err(BrainError::Storage {
                message: "Nested transactions are not supported".to_string(),
                source: None,
            });
        }

        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to begin transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        // Wrap the transaction reference in SqliteTransactionRef.
        // The lifetime 'a of the wrapped &Transaction is tied to this block.
        let active = ActiveConnection::new(&tx);
        let tx_ref = SqliteTransactionRef {
            repos: active,
            _marker: std::marker::PhantomData,
        };

        TRANSACTION_ACTIVE.with(|active| active.set(true));
        struct TxGuard;
        impl Drop for TxGuard {
            fn drop(&mut self) {
                TRANSACTION_ACTIVE.with(|active| active.set(false));
            }
        }
        let _guard = TxGuard;

        let result = f(&tx_ref);

        match result {
            Ok(val) => {
                tx.commit().map_err(|e| BrainError::Storage {
                    message: format!("Failed to commit transaction: {}", e),
                    source: Some(Box::new(e)),
                })?;
                Ok(val)
            }
            Err(err) => {
                let _ = tx.rollback();
                Err(err)
            }
        }
    }
}

impl Storage for SqliteStorage {
    fn run_transaction(
        &self,
        f: &mut dyn FnMut(&dyn StorageTransaction) -> Result<(), BrainError>,
    ) -> Result<(), BrainError> {
        self.run_transaction(|tx| f(tx))
    }
}

impl SqliteStorage {
    /// Inserts an edge into the archived_edges partition.
    pub fn archive_edge(
        &self,
        source: &str,
        target: &str,
        relation: &str,
        weight: f64,
        updated_at: u64,
    ) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection for archival: {}", e),
            source: Some(Box::new(e)),
        })?;
        let archived_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        conn.execute(
            "INSERT OR REPLACE INTO archived_edges (source, target, relation, weight, updated_at, archived_at) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![source, target, relation, weight, updated_at, archived_at],
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to insert into archived_edges: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(())
    }

    /// Checks if an edge exists in the archived_edges partition.
    pub fn is_edge_archived(
        &self,
        source: &str,
        target: &str,
        relation: &str,
    ) -> Result<bool, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM archived_edges WHERE source = ? AND target = ? AND relation = ?",
            rusqlite::params![source, target, relation],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(count > 0)
    }

    /// Saves a single `TemporalEdge` including its temporal metadata (validity and observed_at).
    pub fn save_temporal_edge(
        &self,
        temp_edge: &brain_domain::TemporalEdge,
    ) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection to save temporal edge: {}", e),
            source: Some(Box::new(e)),
        })?;

        // Serialize validity (TemporalValidity) as JSON string
        let validity_json =
            serde_json::to_string(&temp_edge.validity).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize validity: {}", e),
                source: Some(Box::new(e)),
            })?;

        conn.execute(
            "INSERT OR REPLACE INTO edges (source, target, relation, weight, updated_at, observed_at, validity) VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                temp_edge.edge.source.to_string(),
                temp_edge.edge.target.to_string(),
                temp_edge.edge.relation.id().as_str(),
                temp_edge.edge.weight,
                temp_edge.edge.updated_at,
                temp_edge.observed_at.unix_seconds(),
                validity_json
            ],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to execute save temporal edge: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Lists all edges from the database, reconstructed as `TemporalEdge` with their metadata.
    pub fn list_all_temporal_edges(&self) -> Result<Vec<brain_domain::TemporalEdge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn.prepare(
            "SELECT source, target, relation, weight, updated_at, observed_at, validity FROM edges"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

        let rows = stmt
            .query_map([], |row| {
                let source_str: String = row.get(0)?;
                let target_str: String = row.get(1)?;
                let relation_str: String = row.get(2)?;
                let weight: f64 = row.get(3)?;
                let updated_at: u64 = row.get(4)?;
                let observed_at_sec: u64 = row.get(5)?;
                let validity_json: String = row.get(6)?;

                Ok((
                    source_str,
                    target_str,
                    relation_str,
                    weight,
                    updated_at,
                    observed_at_sec,
                    validity_json,
                ))
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query temporal edges: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut temp_edges = Vec::new();
        for r in rows {
            let (
                source_str,
                target_str,
                relation_str,
                weight,
                updated_at,
                observed_at_sec,
                validity_json,
            ) = r.map_err(|e| BrainError::Storage {
                message: format!("Failed to read database row: {}", e),
                source: Some(Box::new(e)),
            })?;

            let source = uuid::Uuid::parse_str(&source_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to parse source NodeId: {}", e),
                source: Some(Box::new(e)),
            })?;
            let target = uuid::Uuid::parse_str(&target_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to parse target NodeId: {}", e),
                source: Some(Box::new(e)),
            })?;
            let relation: brain_domain::RelationKind =
                std::str::FromStr::from_str(&relation_str).unwrap();

            let mut edge = brain_domain::Edge::new(
                brain_domain::NodeId(source),
                brain_domain::NodeId(target),
                relation,
                weight,
            );
            edge.updated_at = updated_at;

            let validity: brain_domain::TemporalValidity =
                if validity_json.is_empty() || validity_json == "[]" {
                    brain_domain::TemporalValidity::new(Vec::new())
                } else {
                    serde_json::from_str(&validity_json)
                        .unwrap_or_else(|_| brain_domain::TemporalValidity::new(Vec::new()))
                };

            let observed_at = brain_domain::TimePoint::from_unix_seconds(observed_at_sec);

            temp_edges.push(brain_domain::TemporalEdge {
                edge,
                validity,
                observed_at,
            });
        }

        Ok(temp_edges)
    }

    /// Saves a single `WeightSnapshot` to the database.
    pub fn save_weight_snapshot(
        &self,
        snapshot: &brain_domain::retrieval::models::WeightSnapshot,
    ) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection to save weight snapshot: {}", e),
            source: Some(Box::new(e)),
        })?;

        let metadata_json = serde_json::to_string(&snapshot.metadata.calibration_metadata)
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize calibration metadata: {}", e),
                source: Some(Box::new(e)),
            })?;

        conn.execute(
            "INSERT OR REPLACE INTO weight_snapshots (version, created_at, semantic_weight, graph_weight, recency_weight, temporal_weight, calibration_metadata) VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                snapshot.metadata.version.value(),
                snapshot.metadata.created_at.unix_seconds(),
                snapshot.weights.semantic().value(),
                snapshot.weights.graph().value(),
                snapshot.weights.recency().value(),
                snapshot.weights.temporal().value(),
                metadata_json
            ],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to execute save weight snapshot: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Retrieves a single `WeightSnapshot` by version.
    pub fn get_weight_snapshot(
        &self,
        version: brain_domain::retrieval::models::SnapshotVersion,
    ) -> Result<Option<brain_domain::retrieval::models::WeightSnapshot>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn.prepare(
            "SELECT version, created_at, semantic_weight, graph_weight, recency_weight, temporal_weight, calibration_metadata FROM weight_snapshots WHERE version = ?"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare statement to get weight snapshot: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut rows = stmt
            .query(rusqlite::params![version.value()])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query weight snapshot: {}", e),
                source: Some(Box::new(e)),
            })?;

        if let Some(row) = rows.next().map_err(|e| BrainError::Storage {
            message: format!("Failed to fetch next weight snapshot row: {}", e),
            source: Some(Box::new(e)),
        })? {
            let version_val: u64 = row.get(0).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;
            let created_at_val: u64 = row.get(1).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;
            let sem_w: f64 = row.get(2).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;
            let gr_w: f64 = row.get(3).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;
            let rec_w: f64 = row.get(4).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;
            let temp_w: f64 = row.get(5).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;
            let metadata_str: String = row.get(6).map_err(|e| BrainError::Storage {
                message: e.to_string(),
                source: None,
            })?;

            let cal_meta: brain_domain::retrieval::models::CalibrationMetadata =
                serde_json::from_str(&metadata_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize calibration metadata: {}", e),
                    source: Some(Box::new(e)),
                })?;

            let weights = brain_domain::retrieval::models::RankingWeights::new(
                brain_domain::retrieval::models::RankingWeight::new(sem_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
                brain_domain::retrieval::models::RankingWeight::new(gr_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
                brain_domain::retrieval::models::RankingWeight::new(rec_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
                brain_domain::retrieval::models::RankingWeight::new(temp_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
            );

            Ok(Some(brain_domain::retrieval::models::WeightSnapshot {
                metadata: brain_domain::retrieval::models::SnapshotMetadata {
                    version: brain_domain::retrieval::models::SnapshotVersion::new(version_val),
                    created_at: brain_domain::temporal::TimePoint::from_unix_seconds(
                        created_at_val,
                    ),
                    calibration_metadata: cal_meta,
                },
                weights,
            }))
        } else {
            Ok(None)
        }
    }

    /// Lists all stored `WeightSnapshot` records in ascending order of version.
    pub fn list_all_weight_snapshots(
        &self,
    ) -> Result<Vec<brain_domain::retrieval::models::WeightSnapshot>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn.prepare(
            "SELECT version, created_at, semantic_weight, graph_weight, recency_weight, temporal_weight, calibration_metadata FROM weight_snapshots ORDER BY version ASC"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare list snapshots statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let rows = stmt
            .query_map([], |row| {
                let version_val: u64 = row.get(0)?;
                let created_at_val: u64 = row.get(1)?;
                let sem_w: f64 = row.get(2)?;
                let gr_w: f64 = row.get(3)?;
                let rec_w: f64 = row.get(4)?;
                let temp_w: f64 = row.get(5)?;
                let metadata_str: String = row.get(6)?;
                Ok((
                    version_val,
                    created_at_val,
                    sem_w,
                    gr_w,
                    rec_w,
                    temp_w,
                    metadata_str,
                ))
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query snapshots: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for r in rows {
            let (version_val, created_at_val, sem_w, gr_w, rec_w, temp_w, metadata_str) = r
                .map_err(|e| BrainError::Storage {
                    message: format!("Failed to read snapshot row: {}", e),
                    source: Some(Box::new(e)),
                })?;

            let cal_meta: brain_domain::retrieval::models::CalibrationMetadata =
                serde_json::from_str(&metadata_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize metadata: {}", e),
                    source: Some(Box::new(e)),
                })?;

            let weights = brain_domain::retrieval::models::RankingWeights::new(
                brain_domain::retrieval::models::RankingWeight::new(sem_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
                brain_domain::retrieval::models::RankingWeight::new(gr_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
                brain_domain::retrieval::models::RankingWeight::new(rec_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
                brain_domain::retrieval::models::RankingWeight::new(temp_w).map_err(|e| {
                    BrainError::Storage {
                        message: format!("{:?}", e),
                        source: None,
                    }
                })?,
            );

            results.push(brain_domain::retrieval::models::WeightSnapshot {
                metadata: brain_domain::retrieval::models::SnapshotMetadata {
                    version: brain_domain::retrieval::models::SnapshotVersion::new(version_val),
                    created_at: brain_domain::temporal::TimePoint::from_unix_seconds(
                        created_at_val,
                    ),
                    calibration_metadata: cal_meta,
                },
                weights,
            });
        }

        Ok(results)
    }

    /// Saves a single `FeedbackEvent` to the database.
    pub fn save_feedback_event(
        &self,
        event: &brain_domain::retrieval::models::FeedbackEvent,
    ) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection to save feedback event: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO feedback_events (id, schema_version, query, node_id, selected, timestamp, ranking_position, context) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                event.id,
                event.schema_version,
                event.query,
                event.node_id.to_string(),
                if event.selected { 1 } else { 0 },
                event.timestamp,
                event.ranking_position,
                event.context
            ],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to execute save feedback event: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Lists all feedback events from the database.
    pub fn list_all_feedback_events(
        &self,
    ) -> Result<Vec<brain_domain::retrieval::models::FeedbackEvent>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, schema_version, query, node_id, selected, timestamp, ranking_position, context FROM feedback_events"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare select feedback statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let schema_version: u32 = row.get(1)?;
                let query: String = row.get(2)?;
                let node_id_str: String = row.get(3)?;
                let selected_val: i32 = row.get(4)?;
                let timestamp: u64 = row.get(5)?;
                let ranking_position: usize = row.get(6)?;
                let context: String = row.get(7)?;

                let u = uuid::Uuid::parse_str(&node_id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(brain_domain::retrieval::models::FeedbackEvent {
                    id,
                    schema_version,
                    query,
                    node_id: NodeId(u),
                    selected: selected_val != 0,
                    timestamp,
                    ranking_position,
                    context,
                })
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query feedback events: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r.map_err(|e| BrainError::Storage {
                message: format!("Failed to read feedback row: {}", e),
                source: Some(Box::new(e)),
            })?);
        }

        Ok(results)
    }

    /// Evaluates and applies memory consolidation rules inside a single database transaction.
    /// Returns the list of actions executed.
    pub fn consolidate_memories(
        &self,
        policy: brain_domain::ConsolidationPolicy,
    ) -> Result<Vec<brain_domain::ConsolidationAction>, BrainError> {
        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection for consolidation: {}", e),
            source: Some(Box::new(e)),
        })?;

        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to begin consolidation transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let active = ActiveConnection::new(&tx);

        // 1. Load active nodes and edges
        let nodes = active.nodes().list_all()?;
        let edges = active.edges().list_all()?;

        let mut graph = brain_domain::KnowledgeGraph::new();
        for n in nodes {
            graph.nodes.insert(n.id, n);
        }
        for e in edges {
            let edge_id = brain_domain::EdgeId::new(e.source, e.target, e.relation.id());
            graph.edges.insert(edge_id, e);
        }

        // 2. Analyze and plan
        let consolidator = brain_domain::Consolidator::new(policy);
        let analysis = consolidator.analyze(&graph);
        let actions = consolidator.plan(analysis);

        // 3. Apply actions transactionally
        for action in &actions {
            match &action.action {
                brain_domain::ConsolidationActionType::PromoteToSemantic { edge_id } => {
                    if let Some(mut edge) = active.edges().find_by_id(edge_id)? {
                        edge.weight = 1.0;
                        edge.updated_at = current_time;
                        active.edges().save(&edge)?;
                    }
                }
                brain_domain::ConsolidationActionType::MergeNodes {
                    canonical_node_id,
                    redundant_node_ids,
                    merged_label: _,
                } => {
                    if let Some(mut canonical) = active.nodes().find_by_id(canonical_node_id)? {
                        for red_id in redundant_node_ids {
                            if let Some(red_node) = active.nodes().find_by_id(red_id)? {
                                canonical.merge_with(&red_node);
                                active.nodes().delete(red_id)?;

                                // Redirect all edges connected to red_id
                                let connections = active.edges().get_connections(red_id)?;
                                for mut edge in connections {
                                    let old_id = brain_domain::EdgeId::new(
                                        edge.source,
                                        edge.target,
                                        edge.relation.id(),
                                    );
                                    active.edges().delete(&old_id)?;
                                    if edge.source == *red_id {
                                        edge.source = *canonical_node_id;
                                    }
                                    if edge.target == *red_id {
                                        edge.target = *canonical_node_id;
                                    }
                                    active.edges().save(&edge)?;
                                }
                            }
                        }
                        active.nodes().save(&canonical)?;
                    }
                }
                brain_domain::ConsolidationActionType::ArchiveEdge { edge_id } => {
                    if let Some(edge) = active.edges().find_by_id(edge_id)? {
                        active.edges().delete(edge_id)?;
                        let relation_str = edge.relation.to_string();
                        let source_str = edge.source.to_string();
                        let target_str = edge.target.to_string();
                        tx.execute(
                            "INSERT OR REPLACE INTO archived_edges (source, target, relation, weight, updated_at, archived_at) VALUES (?, ?, ?, ?, ?, ?)",
                            rusqlite::params![source_str, target_str, relation_str, edge.weight, edge.updated_at, current_time],
                        ).map_err(|e| BrainError::Storage {
                            message: format!("Failed to insert archived edge: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                    }
                }
                brain_domain::ConsolidationActionType::PruneEdge { edge_id } => {
                    active.edges().delete(edge_id)?;
                }
            }
        }

        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit consolidation transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(actions)
    }
}

/// Sealed wrapper around a SQLite connection reference to restrict raw DB access.
pub(crate) struct ActiveConnection<'a> {
    conn: &'a rusqlite::Connection,
}

unsafe impl<'a> Send for ActiveConnection<'a> {}
unsafe impl<'a> Sync for ActiveConnection<'a> {}

impl<'a> ActiveConnection<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn execute<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<usize, rusqlite::Error> {
        self.conn.execute(sql, params)
    }

    pub fn prepare(&self, sql: &str) -> Result<rusqlite::Statement<'_>, rusqlite::Error> {
        self.conn.prepare(sql)
    }

    pub fn prepare_cached(&self, sql: &str) -> Result<rusqlite::CachedStatement<'_>, rusqlite::Error> {
        self.conn.prepare_cached(sql)
    }

    #[allow(dead_code)]
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T, rusqlite::Error>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> Result<T, rusqlite::Error>,
    {
        self.conn.query_row(sql, params, f)
    }
}

/// Implementation of StorageTransaction wrapping a SQLite transaction reference.
pub struct SqliteTransactionRef<'a, 'b> {
    repos: ActiveConnection<'a>,
    _marker: std::marker::PhantomData<&'b rusqlite::Transaction<'b>>,
}

unsafe impl<'a, 'b> Send for SqliteTransactionRef<'a, 'b> {}
unsafe impl<'a, 'b> Sync for SqliteTransactionRef<'a, 'b> {}

impl<'a, 'b> StorageTransaction for SqliteTransactionRef<'a, 'b> {
    fn repositories(&self) -> &dyn RepositorySet {
        &self.repos
    }
}

impl<'a> RepositorySet for ActiveConnection<'a> {
    fn nodes(&self) -> &dyn NodeRepository {
        self
    }
    fn edges(&self) -> &dyn EdgeRepository {
        self
    }
    fn embeddings(&self) -> &dyn EmbeddingRepository {
        self
    }
    fn sessions(&self) -> &dyn SessionRepository {
        self
    }
    fn configs(&self) -> &dyn ConfigRepository {
        self
    }
}

impl RepositorySet for SqliteStorage {
    fn nodes(&self) -> &dyn NodeRepository {
        self
    }
    fn edges(&self) -> &dyn EdgeRepository {
        self
    }
    fn embeddings(&self) -> &dyn EmbeddingRepository {
        self
    }
    fn sessions(&self) -> &dyn SessionRepository {
        self
    }
    fn configs(&self) -> &dyn ConfigRepository {
        self
    }
}

// =========================================================================
// Node Repository implementations and helpers
// =========================================================================

fn is_stub(node_type: &NodeType) -> bool {
    matches!(node_type, NodeKind::Unknown)
}

fn save_node_conn(db: &ActiveConnection<'_>, node: &Node) -> Result<(), BrainError> {
    let existing: Option<(NodeType, HashMap<String, serde_json::Value>)> = {
        let mut stmt = db
            .prepare_cached("SELECT node_type, properties FROM nodes WHERE id = ?")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare select statement: {}", e),
                source: Some(Box::new(e)),
            })?;
        let res = stmt.query_row([node.id.to_string()], |row| {
            let t: String = row.get(0)?;
            let p: String = row.get(1)?;
            Ok((t, p))
        });
        match res {
            Ok((t_str, p_str)) => {
                let t: NodeType =
                    serde_json::from_str(&t_str).map_err(|e| BrainError::Storage {
                        message: format!("Failed to deserialize node type: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                let p: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&p_str).map_err(|e| BrainError::Storage {
                        message: format!("Failed to deserialize properties: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                Some((t, p))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                return Err(BrainError::Storage {
                    message: format!("Failed to query node for check: {}", e),
                    source: Some(Box::new(e)),
                })
            }
        }
    };

    if let Some((existing_type, mut existing_props)) = existing {
        let final_type = if is_stub(&existing_type) {
            node.node_type.clone()
        } else {
            existing_type
        };
        for (k, v) in &node.properties {
            existing_props.insert(k.clone(), v.clone());
        }
        let node_type_str =
            serde_json::to_string(&final_type).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize node type: {}", e),
                source: Some(Box::new(e)),
            })?;
        let properties_str =
            serde_json::to_string(&existing_props).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize properties: {}", e),
                source: Some(Box::new(e)),
            })?;
        db.prepare_cached("UPDATE nodes SET label = ?, node_type = ?, properties = ?, updated_at = ? WHERE id = ?")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare node update statement: {}", e),
            source: Some(Box::new(e)),
        })?
        .execute((
            &node.label,
            node_type_str,
            properties_str,
            node.updated_at,
            node.id.to_string(),
        ))
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to update node {}: {}", node.id, e),
            source: Some(Box::new(e)),
        })?;
    } else {
        let node_type_str =
            serde_json::to_string(&node.node_type).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize node type: {}", e),
                source: Some(Box::new(e)),
            })?;
        let properties_str =
            serde_json::to_string(&node.properties).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize properties: {}", e),
                source: Some(Box::new(e)),
            })?;
        db.prepare_cached("INSERT INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare node insert statement: {}", e),
            source: Some(Box::new(e)),
        })?
        .execute((
            node.id.to_string(),
            &node.label,
            node_type_str,
            properties_str,
            node.updated_at,
        ))
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to insert node {}: {}", node.id, e),
            source: Some(Box::new(e)),
        })?;
    }
    Ok(())
}

fn save_nodes_batch_conn(db: &ActiveConnection<'_>, nodes: &[Node]) -> Result<(), BrainError> {
    let needs_tx = db.conn.is_autocommit();
    if needs_tx {
        db.execute("BEGIN TRANSACTION", [])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to begin transaction for batch: {}", e),
                source: Some(Box::new(e)),
            })?;
    }

    let result = (|| {
        let mut select_stmt = db
            .prepare("SELECT node_type, properties FROM nodes WHERE id = ?")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare select statement: {}", e),
                source: Some(Box::new(e)),
            })?;
        let mut update_stmt = db.prepare(
            "UPDATE nodes SET label = ?, node_type = ?, properties = ?, updated_at = ? WHERE id = ?"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare update statement: {}", e),
            source: Some(Box::new(e)),
        })?;
        let mut insert_stmt = db.prepare(
            "INSERT INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare insert statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        for node in nodes {
            let existing: Option<(NodeType, HashMap<String, serde_json::Value>)> = {
                let res = select_stmt.query_row([node.id.to_string()], |row| {
                    let t: String = row.get(0)?;
                    let p: String = row.get(1)?;
                    Ok((t, p))
                });
                match res {
                    Ok((t_str, p_str)) => {
                        let t: NodeType =
                            serde_json::from_str(&t_str).map_err(|e| BrainError::Storage {
                                message: format!("Failed to deserialize node type: {}", e),
                                source: Some(Box::new(e)),
                            })?;
                        let p: HashMap<String, serde_json::Value> = serde_json::from_str(&p_str)
                            .map_err(|e| BrainError::Storage {
                                message: format!("Failed to deserialize properties: {}", e),
                                source: Some(Box::new(e)),
                            })?;
                        Some((t, p))
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => None,
                    Err(e) => {
                        return Err(BrainError::Storage {
                            message: format!("Failed to query node for check: {}", e),
                            source: Some(Box::new(e)),
                        })
                    }
                }
            };

            if let Some((existing_type, mut existing_props)) = existing {
                let final_type = if is_stub(&existing_type) {
                    node.node_type.clone()
                } else {
                    existing_type
                };
                for (k, v) in &node.properties {
                    existing_props.insert(k.clone(), v.clone());
                }
                let node_type_str =
                    serde_json::to_string(&final_type).map_err(|e| BrainError::Storage {
                        message: format!("Failed to serialize node type: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                let properties_str =
                    serde_json::to_string(&existing_props).map_err(|e| BrainError::Storage {
                        message: format!("Failed to serialize properties: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                update_stmt
                    .execute((
                        &node.label,
                        node_type_str,
                        properties_str,
                        node.updated_at,
                        node.id.to_string(),
                    ))
                    .map_err(|e| BrainError::Storage {
                        message: format!("Failed to execute update node: {}", e),
                        source: Some(Box::new(e)),
                    })?;
            } else {
                let node_type_str =
                    serde_json::to_string(&node.node_type).map_err(|e| BrainError::Storage {
                        message: format!("Failed to serialize node type: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                let properties_str =
                    serde_json::to_string(&node.properties).map_err(|e| BrainError::Storage {
                        message: format!("Failed to serialize properties: {}", e),
                        source: Some(Box::new(e)),
                    })?;
                insert_stmt
                    .execute((
                        node.id.to_string(),
                        &node.label,
                        node_type_str,
                        properties_str,
                        node.updated_at,
                    ))
                    .map_err(|e| BrainError::Storage {
                        message: format!("Failed to execute insert node: {}", e),
                        source: Some(Box::new(e)),
                    })?;
            }
        }
        Ok(())
    })();

    if needs_tx {
        match result {
            Ok(()) => {
                db.execute("COMMIT", []).map_err(|e| BrainError::Storage {
                    message: format!("Failed to commit transaction for batch: {}", e),
                    source: Some(Box::new(e)),
                })?;
            }
            Err(err) => {
                let _ = db.execute("ROLLBACK", []);
                return Err(err);
            }
        }
    }

    result
}

fn find_node_by_id_conn(
    db: &ActiveConnection<'_>,
    id: &NodeId,
) -> Result<Option<Node>, BrainError> {
    let mut stmt = db
        .prepare("SELECT label, node_type, properties, updated_at FROM nodes WHERE id = ?")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let res = stmt.query_row([id.to_string()], |row| {
        let label: String = row.get(0)?;
        let node_type_str: String = row.get(1)?;
        let properties_str: String = row.get(2)?;
        let updated_at: u64 = row.get(3)?;
        Ok((label, node_type_str, properties_str, updated_at))
    });

    match res {
        Ok((label, node_type_str, properties_str, updated_at)) => {
            let node_type: NodeType =
                serde_json::from_str(&node_type_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize node type: {}", e),
                    source: Some(Box::new(e)),
                })?;
            let properties: HashMap<String, serde_json::Value> =
                serde_json::from_str(&properties_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize properties: {}", e),
                    source: Some(Box::new(e)),
                })?;
            let mut node = Node::new(*id, label, node_type).with_properties(properties);
            node.updated_at = updated_at;
            Ok(Some(node))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BrainError::Storage {
            message: format!("Failed to query node: {}", e),
            source: Some(Box::new(e)),
        }),
    }
}

fn delete_node_conn(db: &ActiveConnection<'_>, id: &NodeId) -> Result<(), BrainError> {
    db.execute("DELETE FROM nodes WHERE id = ?", [id.to_string()])
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete node {}: {}", id, e),
            source: Some(Box::new(e)),
        })?;
    Ok(())
}

fn list_all_nodes_conn(db: &ActiveConnection<'_>) -> Result<Vec<Node>, BrainError> {
    let mut stmt = db
        .prepare("SELECT id, label, node_type, properties, updated_at FROM nodes")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare statement: {}", e),
            source: Some(Box::new(e)),
        })?;

    let node_iter = stmt
        .query_map([], |row| {
            let id_str: String = row.get(0)?;
            let label: String = row.get(1)?;
            let node_type_str: String = row.get(2)?;
            let properties_str: String = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((id_str, label, node_type_str, properties_str, updated_at))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("Query execution failed: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut nodes = Vec::new();
    for item in node_iter {
        let (id_str, label, node_type_str, properties_str, updated_at) =
            item.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse query row: {}", e),
                source: Some(Box::new(e)),
            })?;
        let id = uuid::Uuid::parse_str(&id_str)
            .map(NodeId)
            .map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID in storage: {}", e),
                source: Some(Box::new(e)),
            })?;
        let node_type: NodeType =
            serde_json::from_str(&node_type_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize node type: {}", e),
                source: Some(Box::new(e)),
            })?;
        let properties: HashMap<String, serde_json::Value> = serde_json::from_str(&properties_str)
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize properties: {}", e),
                source: Some(Box::new(e)),
            })?;
        let mut node = Node::new(id, label, node_type).with_properties(properties);
        node.updated_at = updated_at;
        nodes.push(node);
    }

    Ok(nodes)
}

fn find_nodes_by_tokens_conn(
    db: &ActiveConnection<'_>,
    tokens: &[String],
) -> Result<Vec<Node>, BrainError> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let mut clauses = Vec::with_capacity(tokens.len());
    for i in 0..tokens.len() {
        clauses.push(format!(
            "(LOWER(label) LIKE ?{0} ESCAPE '\\' OR LOWER(properties) LIKE ?{0} ESCAPE '\\')",
            i + 1
        ));
    }
    let query_str = format!(
        "SELECT id, label, node_type, properties, updated_at FROM nodes WHERE {}",
        clauses.join(" OR ")
    );

    let mut stmt = db.prepare(&query_str).map_err(|e| BrainError::Storage {
        message: format!("Failed to prepare search query: {}", e),
        source: Some(Box::new(e)),
    })?;

    let params: Vec<String> = tokens
        .iter()
        .map(|t| {
            let escaped = t
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            format!("%{}%", escaped.to_lowercase())
        })
        .collect();

    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

    let iter = stmt
        .query_map(&*param_refs, |row| {
            let id_str: String = row.get(0)?;
            let label: String = row.get(1)?;
            let node_type_str: String = row.get(2)?;
            let properties_str: String = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((id_str, label, node_type_str, properties_str, updated_at))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("Search query execution failed: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut nodes = Vec::new();
    for item in iter {
        let (id_str, label, node_type_str, properties_str, updated_at) =
            item.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse search row: {}", e),
                source: Some(Box::new(e)),
            })?;

        let id = uuid::Uuid::parse_str(&id_str)
            .map(NodeId)
            .map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID: {}", e),
                source: Some(Box::new(e)),
            })?;

        let node_type: NodeType =
            serde_json::from_str(&node_type_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize node type: {}", e),
                source: Some(Box::new(e)),
            })?;

        let properties: HashMap<String, serde_json::Value> = serde_json::from_str(&properties_str)
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize properties: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut node = Node::new(id, label, node_type).with_properties(properties);
        node.updated_at = updated_at;
        nodes.push(node);
    }

    Ok(nodes)
}

fn find_nodes_by_fts_conn(
    db: &ActiveConnection<'_>,
    query: &str,
) -> Result<Vec<(Node, f64)>, BrainError> {
    let terms: Vec<String> = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let sanitized = terms.join(" OR ");
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = db
        .prepare(
            "SELECT n.id, n.label, n.node_type, n.properties, n.updated_at, bm25(node_search) \
             FROM nodes n \
             JOIN node_search ns ON n.rowid = ns.rowid \
             WHERE node_search MATCH ?1 \
             ORDER BY bm25(node_search) ASC",
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare FTS search statement: {}", e),
            source: Some(Box::new(e)),
        })?;

    let iter = stmt
        .query_map([&sanitized], |row| {
            let id_str: String = row.get(0)?;
            let label: String = row.get(1)?;
            let node_type_str: String = row.get(2)?;
            let properties_str: String = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            let bm25_score: f64 = row.get(5)?;
            Ok((
                id_str,
                label,
                node_type_str,
                properties_str,
                updated_at,
                bm25_score,
            ))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("FTS query execution failed: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut results = Vec::new();
    for item in iter {
        let (id_str, label, node_type_str, properties_str, updated_at, bm25_score) =
            item.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse FTS search row: {}", e),
                source: Some(Box::new(e)),
            })?;

        let id = uuid::Uuid::parse_str(&id_str)
            .map(NodeId)
            .map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID: {}", e),
                source: Some(Box::new(e)),
            })?;

        let node_type: NodeType =
            serde_json::from_str(&node_type_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize node type: {}", e),
                source: Some(Box::new(e)),
            })?;

        let properties: HashMap<String, serde_json::Value> = serde_json::from_str(&properties_str)
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize properties: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut node = Node::new(id, label, node_type).with_properties(properties);
        node.updated_at = updated_at;
        results.push((node, -bm25_score));
    }

    Ok(results)
}

impl NodeRepository for SqliteStorage {
    fn save(&self, node: &Node) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_node_conn(&active, node)
    }

    fn save_batch(&self, nodes: &[Node]) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_nodes_batch_conn(&active, nodes)
    }

    fn find_by_id(&self, id: &NodeId) -> Result<Option<Node>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        find_node_by_id_conn(&active, id)
    }

    fn delete(&self, id: &NodeId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        delete_node_conn(&active, id)
    }

    fn list_all(&self) -> Result<Vec<Node>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        list_all_nodes_conn(&active)
    }

    fn find_by_tokens(&self, tokens: &[String]) -> Result<Vec<Node>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        find_nodes_by_tokens_conn(&active, tokens)
    }

    fn find_by_fts(&self, query: &str) -> Result<Vec<(Node, f64)>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        find_nodes_by_fts_conn(&active, query)
    }
}

impl<'a> NodeRepository for ActiveConnection<'a> {
    fn save(&self, node: &Node) -> Result<(), BrainError> {
        save_node_conn(self, node)
    }

    fn save_batch(&self, nodes: &[Node]) -> Result<(), BrainError> {
        save_nodes_batch_conn(self, nodes)
    }

    fn find_by_id(&self, id: &NodeId) -> Result<Option<Node>, BrainError> {
        find_node_by_id_conn(self, id)
    }

    fn delete(&self, id: &NodeId) -> Result<(), BrainError> {
        delete_node_conn(self, id)
    }

    fn list_all(&self) -> Result<Vec<Node>, BrainError> {
        list_all_nodes_conn(self)
    }

    fn find_by_tokens(&self, tokens: &[String]) -> Result<Vec<Node>, BrainError> {
        find_nodes_by_tokens_conn(self, tokens)
    }

    fn find_by_fts(&self, query: &str) -> Result<Vec<(Node, f64)>, BrainError> {
        find_nodes_by_fts_conn(self, query)
    }
}

// =========================================================================
// Edge Repository implementations and helpers
// =========================================================================

fn save_edge_conn(db: &ActiveConnection<'_>, edge: &Edge) -> Result<(), BrainError> {
    db.execute(
        "INSERT OR REPLACE INTO edges (source, target, relation, weight, updated_at) VALUES (?, ?, ?, ?, ?)",
        (
            edge.source.to_string(),
            edge.target.to_string(),
            edge.relation.to_string(),
            edge.weight,
            edge.updated_at,
        ),
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to save edge: {}", e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn save_edges_batch_conn(db: &ActiveConnection<'_>, edges: &[Edge]) -> Result<(), BrainError> {
    let needs_tx = db.conn.is_autocommit();
    if needs_tx {
        db.execute("BEGIN TRANSACTION", [])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to begin transaction for batch: {}", e),
                source: Some(Box::new(e)),
            })?;
    }

    let result = (|| {
        let mut stmt = db.prepare(
            "INSERT OR REPLACE INTO edges (source, target, relation, weight, updated_at) VALUES (?, ?, ?, ?, ?)"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        for edge in edges {
            stmt.execute((
                edge.source.to_string(),
                edge.target.to_string(),
                edge.relation.to_string(),
                edge.weight,
                edge.updated_at,
            ))
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute save edge: {}", e),
                source: Some(Box::new(e)),
            })?;
        }
        Ok(())
    })();

    if needs_tx {
        match result {
            Ok(()) => {
                db.execute("COMMIT", []).map_err(|e| BrainError::Storage {
                    message: format!("Failed to commit transaction for batch: {}", e),
                    source: Some(Box::new(e)),
                })?;
            }
            Err(err) => {
                let _ = db.execute("ROLLBACK", []);
                return Err(err);
            }
        }
    }

    result
}

fn find_edge_by_id_conn(
    db: &ActiveConnection<'_>,
    id: &EdgeId,
) -> Result<Option<Edge>, BrainError> {
    let mut stmt = db
        .prepare(
            "SELECT weight, updated_at FROM edges WHERE source = ? AND target = ? AND relation = ?",
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let res = stmt.query_row(
        (
            id.source.to_string(),
            id.target.to_string(),
            id.relation.as_str(),
        ),
        |row| {
            let weight: f64 = row.get(0)?;
            let updated_at: u64 = row.get(1)?;
            Ok((weight, updated_at))
        },
    );

    match res {
        Ok((weight, updated_at)) => {
            let rel = id
                .relation
                .as_str()
                .parse()
                .unwrap_or(RelationKind::Unknown);
            let mut edge = Edge::new(id.source, id.target, rel, weight);
            edge.updated_at = updated_at;
            Ok(Some(edge))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BrainError::Storage {
            message: format!("Failed to query edge: {}", e),
            source: Some(Box::new(e)),
        }),
    }
}

fn delete_edge_conn(db: &ActiveConnection<'_>, id: &EdgeId) -> Result<(), BrainError> {
    db.execute(
        "DELETE FROM edges WHERE source = ? AND target = ? AND relation = ?",
        (
            id.source.to_string(),
            id.target.to_string(),
            id.relation.as_str(),
        ),
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to delete edge: {}", e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn get_edge_connections_conn(
    db: &ActiveConnection<'_>,
    node_id: &NodeId,
) -> Result<Vec<Edge>, BrainError> {
    let mut stmt = db
        .prepare("SELECT source, target, relation, weight, updated_at FROM edges WHERE source = ? OR target = ?")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare connections query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let edge_iter = stmt
        .query_map([node_id.to_string(), node_id.to_string()], |row| {
            let src_str: String = row.get(0)?;
            let tgt_str: String = row.get(1)?;
            let relation: String = row.get(2)?;
            let weight: f64 = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((src_str, tgt_str, relation, weight, updated_at))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to query connections: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut edges = Vec::new();
    for item in edge_iter {
        let (src_str, tgt_str, relation, weight, updated_at) =
            item.map_err(|e| BrainError::Storage {
                message: format!("Failed parsing connection row: {}", e),
                source: Some(Box::new(e)),
            })?;
        let source =
            uuid::Uuid::parse_str(&src_str)
                .map(NodeId)
                .map_err(|e| BrainError::Storage {
                    message: format!("Invalid UUID in storage: {}", e),
                    source: Some(Box::new(e)),
                })?;
        let target =
            uuid::Uuid::parse_str(&tgt_str)
                .map(NodeId)
                .map_err(|e| BrainError::Storage {
                    message: format!("Invalid UUID in storage: {}", e),
                    source: Some(Box::new(e)),
                })?;
        let rel = relation.parse().unwrap_or(RelationKind::Unknown);
        let mut edge = Edge::new(source, target, rel, weight);
        edge.updated_at = updated_at;
        edges.push(edge);
    }
    Ok(edges)
}

fn list_all_edges_conn(db: &ActiveConnection<'_>) -> Result<Vec<Edge>, BrainError> {
    let mut stmt = db
        .prepare("SELECT source, target, relation, weight, updated_at FROM edges")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let edge_iter = stmt
        .query_map([], |row| {
            let src_str: String = row.get(0)?;
            let tgt_str: String = row.get(1)?;
            let relation: String = row.get(2)?;
            let weight: f64 = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((src_str, tgt_str, relation, weight, updated_at))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to query edges list: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut edges = Vec::new();
    for item in edge_iter {
        let (src_str, tgt_str, relation, weight, updated_at) =
            item.map_err(|e| BrainError::Storage {
                message: format!("Failed parsing edge row: {}", e),
                source: Some(Box::new(e)),
            })?;
        let source =
            uuid::Uuid::parse_str(&src_str)
                .map(NodeId)
                .map_err(|e| BrainError::Storage {
                    message: format!("Invalid UUID in storage: {}", e),
                    source: Some(Box::new(e)),
                })?;
        let target =
            uuid::Uuid::parse_str(&tgt_str)
                .map(NodeId)
                .map_err(|e| BrainError::Storage {
                    message: format!("Invalid UUID in storage: {}", e),
                    source: Some(Box::new(e)),
                })?;
        let rel = relation.parse().unwrap_or(RelationKind::Unknown);
        let mut edge = Edge::new(source, target, rel, weight);
        edge.updated_at = updated_at;
        edges.push(edge);
    }
    Ok(edges)
}

impl EdgeRepository for SqliteStorage {
    fn save(&self, edge: &Edge) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_edge_conn(&active, edge)
    }

    fn save_batch(&self, edges: &[Edge]) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_edges_batch_conn(&active, edges)
    }

    fn find_by_id(&self, id: &EdgeId) -> Result<Option<Edge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        find_edge_by_id_conn(&active, id)
    }

    fn delete(&self, id: &EdgeId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        delete_edge_conn(&active, id)
    }

    fn get_connections(&self, node_id: &NodeId) -> Result<Vec<Edge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        get_edge_connections_conn(&active, node_id)
    }

    fn list_all(&self) -> Result<Vec<Edge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        list_all_edges_conn(&active)
    }
}

impl<'a> EdgeRepository for ActiveConnection<'a> {
    fn save(&self, edge: &Edge) -> Result<(), BrainError> {
        save_edge_conn(self, edge)
    }

    fn save_batch(&self, edges: &[Edge]) -> Result<(), BrainError> {
        save_edges_batch_conn(self, edges)
    }

    fn find_by_id(&self, id: &EdgeId) -> Result<Option<Edge>, BrainError> {
        find_edge_by_id_conn(self, id)
    }

    fn delete(&self, id: &EdgeId) -> Result<(), BrainError> {
        delete_edge_conn(self, id)
    }

    fn get_connections(&self, node_id: &NodeId) -> Result<Vec<Edge>, BrainError> {
        get_edge_connections_conn(self, node_id)
    }

    fn list_all(&self) -> Result<Vec<Edge>, BrainError> {
        list_all_edges_conn(self)
    }
}

// =========================================================================
// Embedding Repository implementations and helpers
// =========================================================================

use std::sync::OnceLock;

fn get_predefined_centroids() -> &'static [Vec<f32>] {
    static CENTROIDS: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    CENTROIDS.get_or_init(|| {
        let mut centroids = Vec::with_capacity(8);
        for c in 0..8 {
            let mut v = vec![0.0f32; 384];
            let mut norm_sq = 0.0f32;
            for i in 0..384 {
                let val = ((2.0 * std::f64::consts::PI * (i + 1) as f64 * (c + 1) as f64) / 384.0)
                    .sin() as f32;
                v[i] = val;
                norm_sq += val * val;
            }
            let norm = norm_sq.sqrt();
            if norm > 0.0 {
                for val in v.iter_mut() {
                    *val /= norm;
                }
            }
            centroids.push(v);
        }
        centroids
    })
}

fn compute_closest_centroid(vector: &[f32]) -> i32 {
    let centroids = get_predefined_centroids();
    let mut best_centroid = 0;
    let mut max_similarity = f32::NEG_INFINITY;

    for (c, centroid) in centroids.iter().enumerate() {
        let mut dot_product = 0.0f32;
        let limit = std::cmp::min(vector.len(), centroid.len());
        for i in 0..limit {
            dot_product += vector[i] * centroid[i];
        }
        if dot_product > max_similarity {
            max_similarity = dot_product;
            best_centroid = c as i32;
        }
    }
    best_centroid
}

fn save_embedding_conn(db: &ActiveConnection<'_>, embedding: &Embedding) -> Result<(), BrainError> {
    let mut bytes = Vec::with_capacity(embedding.vector.len() * 4);
    for &val in &embedding.vector {
        bytes.extend_from_slice(&val.to_le_bytes());
    }

    let centroid_id = compute_closest_centroid(&embedding.vector);

    db.execute(
        "INSERT OR REPLACE INTO embeddings (node_id, vector, dimension, centroid_id) VALUES (?, ?, ?, ?)",
        (
            embedding.node_id.to_string(),
            bytes,
            embedding.dimension as i64,
            centroid_id as i64,
        ),
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to save embedding: {}", e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn find_embedding_by_node_id_conn(
    db: &ActiveConnection<'_>,
    node_id: &NodeId,
) -> Result<Option<Embedding>, BrainError> {
    let mut stmt = db
        .prepare("SELECT vector, dimension FROM embeddings WHERE node_id = ?")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare embedding query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let res = stmt.query_row([node_id.to_string()], |row| {
        let bytes: Vec<u8> = row.get(0)?;
        let dimension: i64 = row.get(1)?;
        Ok((bytes, dimension))
    });

    match res {
        Ok((bytes, _dimension)) => {
            let mut vector = Vec::with_capacity(bytes.len() / 4);
            for chunk in bytes.chunks_exact(4) {
                let arr = chunk.try_into().unwrap_or([0u8; 4]);
                vector.push(f32::from_le_bytes(arr));
            }
            Ok(Some(Embedding::new(*node_id, vector)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BrainError::Storage {
            message: format!("Failed to query embedding: {}", e),
            source: Some(Box::new(e)),
        }),
    }
}

fn delete_embedding_conn(db: &ActiveConnection<'_>, node_id: &NodeId) -> Result<(), BrainError> {
    db.execute(
        "DELETE FROM embeddings WHERE node_id = ?",
        [node_id.to_string()],
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to delete embedding: {}", e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn list_all_embeddings_conn(db: &ActiveConnection<'_>) -> Result<Vec<Embedding>, BrainError> {
    let mut stmt = db
        .prepare("SELECT node_id, vector, dimension FROM embeddings")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let iter = stmt
        .query_map([], |row| {
            let node_id_str: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let dimension: i64 = row.get(2)?;
            Ok((node_id_str, bytes, dimension))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("Query execution failed: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut embeddings = Vec::new();
    for item in iter {
        let (node_id_str, bytes, _dimension) = item.map_err(|e| BrainError::Storage {
            message: format!("Failed to parse query row: {}", e),
            source: Some(Box::new(e)),
        })?;
        let node_id = uuid::Uuid::parse_str(&node_id_str)
            .map(NodeId)
            .map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID: {}", e),
                source: Some(Box::new(e)),
            })?;
        let mut vector = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks_exact(4) {
            let arr = chunk.try_into().unwrap_or([0u8; 4]);
            vector.push(f32::from_le_bytes(arr));
        }
        embeddings.push(Embedding::new(node_id, vector));
    }

    Ok(embeddings)
}

impl EmbeddingRepository for SqliteStorage {
    fn save(&self, embedding: &Embedding) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_embedding_conn(&active, embedding)
    }

    fn find_by_node_id(&self, node_id: &NodeId) -> Result<Option<Embedding>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        find_embedding_by_node_id_conn(&active, node_id)
    }

    fn delete(&self, node_id: &NodeId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        delete_embedding_conn(&active, node_id)
    }

    fn list_all_embeddings(&self) -> Result<Vec<Embedding>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        list_all_embeddings_conn(&active)
    }

    fn find_by_centroids(&self, centroid_ids: &[i32]) -> Result<Vec<Embedding>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        find_embeddings_by_centroids_conn(&active, centroid_ids)
    }
}

impl<'a> EmbeddingRepository for ActiveConnection<'a> {
    fn save(&self, embedding: &Embedding) -> Result<(), BrainError> {
        save_embedding_conn(self, embedding)
    }

    fn find_by_node_id(&self, node_id: &NodeId) -> Result<Option<Embedding>, BrainError> {
        find_embedding_by_node_id_conn(self, node_id)
    }

    fn delete(&self, node_id: &NodeId) -> Result<(), BrainError> {
        delete_embedding_conn(self, node_id)
    }

    fn list_all_embeddings(&self) -> Result<Vec<Embedding>, BrainError> {
        list_all_embeddings_conn(self)
    }

    fn find_by_centroids(&self, centroid_ids: &[i32]) -> Result<Vec<Embedding>, BrainError> {
        find_embeddings_by_centroids_conn(self, centroid_ids)
    }
}

fn find_embeddings_by_centroids_conn(
    db: &ActiveConnection<'_>,
    centroid_ids: &[i32],
) -> Result<Vec<Embedding>, BrainError> {
    if centroid_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders: Vec<String> = (0..centroid_ids.len()).map(|_| "?".to_string()).collect();
    let query_str = format!(
        "SELECT node_id, vector, dimension FROM embeddings WHERE centroid_id IN ({})",
        placeholders.join(", ")
    );

    let mut stmt = db.prepare(&query_str).map_err(|e| BrainError::Storage {
        message: format!("Failed to prepare query: {}", e),
        source: Some(Box::new(e)),
    })?;

    let params: Vec<rusqlite::types::Value> = centroid_ids
        .iter()
        .map(|&c| rusqlite::types::Value::Integer(c as i64))
        .collect();

    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

    let iter = stmt
        .query_map(&*param_refs, |row| {
            let node_id_str: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let dimension: i64 = row.get(2)?;
            Ok((node_id_str, bytes, dimension))
        })
        .map_err(|e| BrainError::Storage {
            message: format!("Query execution failed: {}", e),
            source: Some(Box::new(e)),
        })?;

    let mut embeddings = Vec::new();
    for item in iter {
        let (node_id_str, bytes, _dimension) = item.map_err(|e| BrainError::Storage {
            message: format!("Failed to parse query row: {}", e),
            source: Some(Box::new(e)),
        })?;
        let node_id = uuid::Uuid::parse_str(&node_id_str)
            .map(NodeId)
            .map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID: {}", e),
                source: Some(Box::new(e)),
            })?;
        let mut vector = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks_exact(4) {
            let arr = chunk.try_into().unwrap_or([0u8; 4]);
            vector.push(f32::from_le_bytes(arr));
        }
        embeddings.push(Embedding::new(node_id, vector));
    }

    Ok(embeddings)
}

// =========================================================================
// Session Repository implementations and helpers
// =========================================================================

fn save_session_conn(
    db: &ActiveConnection<'_>,
    id: &SessionId,
    session: &Session,
) -> Result<(), BrainError> {
    let session_str = serde_json::to_string(session).map_err(|e| BrainError::Storage {
        message: format!("Failed to serialize session: {}", e),
        source: Some(Box::new(e)),
    })?;
    let now = session.updated_at.0;

    db.execute(
        "INSERT OR REPLACE INTO sessions (id, history, updated_at) VALUES (?, ?, ?)",
        (id.to_string(), session_str, now),
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to save session {}: {}", id, e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn load_session_conn(
    db: &ActiveConnection<'_>,
    id: &SessionId,
) -> Result<Option<Session>, BrainError> {
    let mut stmt = db
        .prepare("SELECT history FROM sessions WHERE id = ?")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let res = stmt.query_row([id.to_string()], |row| {
        let history: String = row.get(0)?;
        Ok(history)
    });

    match res {
        Ok(history_str) => {
            let session: Session =
                serde_json::from_str(&history_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize session: {}", e),
                    source: Some(Box::new(e)),
                })?;
            Ok(Some(session))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BrainError::Storage {
            message: format!("Failed to query session: {}", e),
            source: Some(Box::new(e)),
        }),
    }
}

fn delete_session_conn(db: &ActiveConnection<'_>, id: &SessionId) -> Result<(), BrainError> {
    db.execute("DELETE FROM sessions WHERE id = ?", [id.to_string()])
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete session: {}", e),
            source: Some(Box::new(e)),
        })?;
    Ok(())
}

impl SessionRepository for SqliteStorage {
    fn save_session(&self, id: &SessionId, session: &Session) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_session_conn(&active, id, session)
    }

    fn load_session(&self, id: &SessionId) -> Result<Option<Session>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        load_session_conn(&active, id)
    }

    fn delete_session(&self, id: &SessionId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        delete_session_conn(&active, id)
    }
}

impl<'a> SessionRepository for ActiveConnection<'a> {
    fn save_session(&self, id: &SessionId, session: &Session) -> Result<(), BrainError> {
        save_session_conn(self, id, session)
    }

    fn load_session(&self, id: &SessionId) -> Result<Option<Session>, BrainError> {
        load_session_conn(self, id)
    }

    fn delete_session(&self, id: &SessionId) -> Result<(), BrainError> {
        delete_session_conn(self, id)
    }
}

// =========================================================================
// Config Repository implementations and helpers
// =========================================================================

fn save_config_key_conn(db: &ActiveConnection<'_>, key: &str, val: &str) -> Result<(), BrainError> {
    db.prepare_cached("INSERT OR REPLACE INTO config (key, value) VALUES (?, ?)")
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to prepare config save statement: {}", e),
        source: Some(Box::new(e)),
    })?
    .execute((key, val))
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to save config key {}: {}", key, e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn get_config_key_conn(db: &ActiveConnection<'_>, key: &str) -> Result<Option<String>, BrainError> {
    let mut stmt = db
        .prepare_cached("SELECT value FROM config WHERE key = ?")
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query: {}", e),
            source: Some(Box::new(e)),
        })?;

    let res = stmt.query_row([key], |row| {
        let value: String = row.get(0)?;
        Ok(value)
    });

    match res {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(BrainError::Storage {
            message: format!("Failed to query config key: {}", e),
            source: Some(Box::new(e)),
        }),
    }
}

impl ConfigRepository for SqliteStorage {
    fn save_key(&self, key: &str, val: &str) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_config_key_conn(&active, key, val)
    }

    fn get_key(&self, key: &str) -> Result<Option<String>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        get_config_key_conn(&active, key)
    }
}

impl<'a> ConfigRepository for ActiveConnection<'a> {
    fn save_key(&self, key: &str, val: &str) -> Result<(), BrainError> {
        save_config_key_conn(self, key, val)
    }

    fn get_key(&self, key: &str) -> Result<Option<String>, BrainError> {
        get_config_key_conn(self, key)
    }
}

impl SqliteStorage {
    /// Applies KPP SQLite deltas transactionally to the database.
    pub fn apply_kpp_ops(&self, ops: &[brain_domain::bkf::SqliteOp]) -> Result<(), BrainError> {
        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection for KPP ops: {}", e),
            source: Some(Box::new(e)),
        })?;

        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to start transaction for KPP ops: {}", e),
            source: Some(Box::new(e)),
        })?;

        for op in ops {
            match op {
                brain_domain::bkf::SqliteOp::Node(delta) => match delta {
                    brain_domain::bkf::ProjectionDelta::Insert(node) => {
                        tx.execute(
                            "INSERT INTO nodes (id, label, node_type, properties, updated_at, lifecycle, validity, version_state) \
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
                             ON CONFLICT(id) DO UPDATE SET \
                             label=excluded.label, \
                             node_type=excluded.node_type, \
                             properties=excluded.properties, \
                             updated_at=excluded.updated_at, \
                             lifecycle=excluded.lifecycle, \
                             validity=excluded.validity, \
                             version_state=excluded.version_state",
                            (
                                &node.id,
                                &node.label,
                                &node.entity_type,
                                &node.attributes,
                                1700000000i64,
                                &node.lifecycle,
                                &node.validity,
                                &node.version_state,
                            ),
                        ).map_err(|e| BrainError::Storage {
                            message: format!("Failed to insert/update KPP node: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                    }
                    brain_domain::bkf::ProjectionDelta::Update { id, changes } => {
                        tx.execute(
                            "UPDATE nodes SET label = ?, node_type = ?, properties = ?, lifecycle = ?, validity = ?, version_state = ? WHERE id = ?",
                            (
                                &changes.label,
                                &changes.entity_type,
                                &changes.attributes,
                                &changes.lifecycle,
                                &changes.validity,
                                &changes.version_state,
                                id,
                            ),
                        ).map_err(|e| BrainError::Storage {
                            message: format!("Failed to update KPP node: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                    }
                    brain_domain::bkf::ProjectionDelta::Delete(id) => {
                        tx.execute("DELETE FROM nodes WHERE id = ?", [id])
                            .map_err(|e| BrainError::Storage {
                                message: format!("Failed to delete KPP node: {}", e),
                                source: Some(Box::new(e)),
                            })?;
                    }
                },
                brain_domain::bkf::SqliteOp::Edge(delta) => match delta {
                    brain_domain::bkf::ProjectionDelta::Insert(edge) => {
                        tx.execute(
                            "INSERT INTO edges (source, target, relation, weight, updated_at, lifecycle, version_state) \
                             VALUES (?, ?, ?, ?, ?, ?, ?) \
                             ON CONFLICT(source, target, relation) DO UPDATE SET \
                             weight=excluded.weight, \
                             lifecycle=excluded.lifecycle, \
                             version_state=excluded.version_state",
                            (
                                &edge.source,
                                &edge.target,
                                &edge.relation,
                                edge.weight,
                                1700000000i64,
                                &edge.lifecycle,
                                &edge.version_state,
                            ),
                        ).map_err(|e| BrainError::Storage {
                            message: format!("Failed to insert/update KPP edge: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                    }
                    brain_domain::bkf::ProjectionDelta::Update { id: _, changes } => {
                        tx.execute(
                            "UPDATE edges SET weight = ?, lifecycle = ?, version_state = ? WHERE source = ? AND target = ? AND relation = ?",
                            (
                                changes.weight,
                                &changes.lifecycle,
                                &changes.version_state,
                                &changes.source,
                                &changes.target,
                                &changes.relation,
                            ),
                        ).map_err(|e| BrainError::Storage {
                            message: format!("Failed to update KPP edge: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                    }
                    brain_domain::bkf::ProjectionDelta::Delete(id) => {
                        tx.execute(
                            "DELETE FROM edges WHERE (source || '-' || target || '-' || LOWER(relation)) = ?",
                            [id],
                        ).map_err(|e| BrainError::Storage {
                            message: format!("Failed to delete KPP edge: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                    }
                },
            }
        }

        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit KPP operations: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Logs a KPP pipeline event to the system_event_log database table.
    pub fn log_kpp_event(&self, event: &brain_domain::DomainEvent) -> Result<(), BrainError> {
        let event_id = uuid::Uuid::new_v4();
        let correlation_id = uuid::Uuid::new_v4();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let payload_json = serde_json::to_string(event).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize KPP event payload: {}", e),
            source: Some(Box::new(e)),
        })?;

        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection for event logging: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "INSERT INTO system_event_log (event_id, correlation_id, timestamp_ms, version, source, topic, payload) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                event_id.to_string(),
                correlation_id.to_string(),
                timestamp_ms as i64,
                "1.0",
                "KPP",
                "core",
                payload_json,
            ),
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to insert system_event_log row: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }
}

impl EventLogRepository for SqliteStorage {
    fn insert_event(&self, envelope: &IngestionEnvelope) -> Result<u64, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        // 1. Check for duplicate event_id
        let event_id_str = envelope.identity.event_id.to_string();
        let mut check_stmt = conn
            .prepare("SELECT sequence FROM event_log WHERE event_id = ?1")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare duplicate check statement: {}", e),
                source: Some(Box::new(e)),
            })?;
        let mut rows = check_stmt
            .query(rusqlite::params![event_id_str])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query duplicate events: {}", e),
                source: Some(Box::new(e)),
            })?;
        if let Some(row) = rows.next().map_err(|e| BrainError::Storage {
            message: format!("Failed to fetch duplicate event row: {}", e),
            source: Some(Box::new(e)),
        })? {
            let seq: i64 = row.get(0).map_err(|e| BrainError::Storage {
                message: format!("Failed to get sequence ID: {}", e),
                source: Some(Box::new(e)),
            })?;
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
        let payload =
            brain_integrations::to_canonical_json(envelope).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize canonical envelope payload: {}", e),
                source: Some(Box::new(e)),
            })?;
        let timestamp_str = envelope.identity.timestamp.to_rfc3339();
        let received_at_str = chrono::Utc::now().to_rfc3339();

        // 3. Insert and retrieve sequence ID
        conn.execute(
            "INSERT INTO event_log (event_id, adapter_id, client_id, session_id, workspace_id, conversation_id, event_model_version, event_type, payload, timestamp, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
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
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to insert ingestion event: {}", e),
            source: Some(Box::new(e)),
        })?;

        let sequence = conn.last_insert_rowid() as u64;
        Ok(sequence)
    }

    fn is_duplicate_event(&self, event_id: &brain_domain::EventId) -> Result<bool, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn
            .prepare("SELECT 1 FROM event_log WHERE event_id = ?1")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare duplicate check statement: {}", e),
                source: Some(Box::new(e)),
            })?;
        let exists = stmt
            .exists(rusqlite::params![event_id.0.to_string()])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to check event existence: {}", e),
                source: Some(Box::new(e)),
            })?;
        Ok(exists)
    }

    fn get_events_after(&self, sequence: u64) -> Result<Vec<IngestionEnvelope>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn
            .prepare("SELECT payload FROM event_log WHERE sequence > ?1 ORDER BY sequence ASC")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare select events query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let rows = stmt
            .query_map(rusqlite::params![sequence], |row| {
                let payload_str: String = row.get(0)?;
                Ok(payload_str)
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query events after sequence: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut envelopes = Vec::new();
        for row in rows {
            let payload_str = row.map_err(|e| BrainError::Storage {
                message: format!("Failed to fetch event row: {}", e),
                source: Some(Box::new(e)),
            })?;
            let envelope: IngestionEnvelope =
                serde_json::from_str(&payload_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize IngestionEnvelope payload: {}", e),
                    source: Some(Box::new(e)),
                })?;
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }
}
