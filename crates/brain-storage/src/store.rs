use crate::connection::init_pool;
use crate::migrations::run_migrations;
use brain_core::errors::BrainError;
use brain_core::repositories::{
    ConfigRepository, EdgeRepository, EmbeddingRepository, NodeRepository, RepositorySet,
    SessionRepository, StorageTransaction,
};
use brain_domain::{Conversation, Edge, EdgeId, Embedding, Node, NodeId, NodeType, SessionId, RelationKind, NodeKind};
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

    /// Inserts an edge into the archived_edges partition.
    pub fn archive_edge(&self, source: &str, target: &str, relation: &str, weight: f64, updated_at: u64) -> Result<(), BrainError> {
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
    pub fn is_edge_archived(&self, source: &str, target: &str, relation: &str) -> Result<bool, BrainError> {
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
    pub fn save_temporal_edge(&self, temp_edge: &brain_domain::TemporalEdge) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection to save temporal edge: {}", e),
            source: Some(Box::new(e)),
        })?;

        // Serialize validity (TemporalValidity) as JSON string
        let validity_json = serde_json::to_string(&temp_edge.validity).map_err(|e| BrainError::Storage {
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

        let rows = stmt.query_map([], |row| {
            let source_str: String = row.get(0)?;
            let target_str: String = row.get(1)?;
            let relation_str: String = row.get(2)?;
            let weight: f64 = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            let observed_at_sec: u64 = row.get(5)?;
            let validity_json: String = row.get(6)?;

            Ok((source_str, target_str, relation_str, weight, updated_at, observed_at_sec, validity_json))
        }).map_err(|e| BrainError::Storage {
            message: format!("Failed to query temporal edges: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut temp_edges = Vec::new();
        for r in rows {
            let (source_str, target_str, relation_str, weight, updated_at, observed_at_sec, validity_json) = r.map_err(|e| BrainError::Storage {
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
            let relation: brain_domain::RelationKind = std::str::FromStr::from_str(&relation_str).unwrap();

            let mut edge = brain_domain::Edge::new(
                brain_domain::NodeId(source),
                brain_domain::NodeId(target),
                relation,
                weight,
            );
            edge.updated_at = updated_at;

            let validity: brain_domain::TemporalValidity = if validity_json.is_empty() || validity_json == "[]" {
                brain_domain::TemporalValidity::new(Vec::new())
            } else {
                serde_json::from_str(&validity_json).unwrap_or_else(|_| brain_domain::TemporalValidity::new(Vec::new()))
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

    /// Evaluates and applies memory consolidation rules inside a single database transaction.
    /// Returns the list of actions executed.
    pub fn consolidate_memories(&self, policy: brain_domain::ConsolidationPolicy) -> Result<Vec<brain_domain::ConsolidationAction>, BrainError> {
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
                                    let old_id = brain_domain::EdgeId::new(edge.source, edge.target, edge.relation.id());
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
        let mut stmt = db.prepare("SELECT node_type, properties FROM nodes WHERE id = ?").map_err(|e| BrainError::Storage {
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
                let t: NodeType = serde_json::from_str(&t_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize node type: {}", e),
                    source: Some(Box::new(e)),
                })?;
                let p: HashMap<String, serde_json::Value> = serde_json::from_str(&p_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize properties: {}", e),
                    source: Some(Box::new(e)),
                })?;
                Some((t, p))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(BrainError::Storage {
                message: format!("Failed to query node for check: {}", e),
                source: Some(Box::new(e)),
            }),
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
        let node_type_str = serde_json::to_string(&final_type).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize node type: {}", e),
            source: Some(Box::new(e)),
        })?;
        let properties_str = serde_json::to_string(&existing_props).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize properties: {}", e),
            source: Some(Box::new(e)),
        })?;
        db.execute(
            "UPDATE nodes SET label = ?, node_type = ?, properties = ?, updated_at = ? WHERE id = ?",
            (
                &node.label,
                node_type_str,
                properties_str,
                node.updated_at,
                node.id.to_string(),
            ),
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to update node {}: {}", node.id, e),
            source: Some(Box::new(e)),
        })?;
    } else {
        let node_type_str = serde_json::to_string(&node.node_type).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize node type: {}", e),
            source: Some(Box::new(e)),
        })?;
        let properties_str = serde_json::to_string(&node.properties).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize properties: {}", e),
            source: Some(Box::new(e)),
        })?;
        db.execute(
            "INSERT INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)",
            (
                node.id.to_string(),
                &node.label,
                node_type_str,
                properties_str,
                node.updated_at,
            ),
        )
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
        let mut select_stmt = db.prepare("SELECT node_type, properties FROM nodes WHERE id = ?").map_err(|e| BrainError::Storage {
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
                        let t: NodeType = serde_json::from_str(&t_str).map_err(|e| BrainError::Storage {
                            message: format!("Failed to deserialize node type: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                        let p: HashMap<String, serde_json::Value> = serde_json::from_str(&p_str).map_err(|e| BrainError::Storage {
                            message: format!("Failed to deserialize properties: {}", e),
                            source: Some(Box::new(e)),
                        })?;
                        Some((t, p))
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => None,
                    Err(e) => return Err(BrainError::Storage {
                        message: format!("Failed to query node for check: {}", e),
                        source: Some(Box::new(e)),
                    }),
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
                let node_type_str = serde_json::to_string(&final_type).map_err(|e| BrainError::Storage {
                    message: format!("Failed to serialize node type: {}", e),
                    source: Some(Box::new(e)),
                })?;
                let properties_str = serde_json::to_string(&existing_props).map_err(|e| BrainError::Storage {
                    message: format!("Failed to serialize properties: {}", e),
                    source: Some(Box::new(e)),
                })?;
                update_stmt.execute((
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
                let node_type_str = serde_json::to_string(&node.node_type).map_err(|e| BrainError::Storage {
                    message: format!("Failed to serialize node type: {}", e),
                    source: Some(Box::new(e)),
                })?;
                let properties_str = serde_json::to_string(&node.properties).map_err(|e| BrainError::Storage {
                    message: format!("Failed to serialize properties: {}", e),
                    source: Some(Box::new(e)),
                })?;
                insert_stmt.execute((
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
        (id.source.to_string(), id.target.to_string(), id.relation.as_str()),
        |row| {
            let weight: f64 = row.get(0)?;
            let updated_at: u64 = row.get(1)?;
            Ok((weight, updated_at))
        },
    );

    match res {
        Ok((weight, updated_at)) => {
            let rel = id.relation.as_str().parse().unwrap_or(RelationKind::Unknown);
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
        (id.source.to_string(), id.target.to_string(), id.relation.as_str()),
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

fn save_embedding_conn(db: &ActiveConnection<'_>, embedding: &Embedding) -> Result<(), BrainError> {
    let mut bytes = Vec::with_capacity(embedding.vector.len() * 4);
    for &val in &embedding.vector {
        bytes.extend_from_slice(&val.to_le_bytes());
    }

    db.execute(
        "INSERT OR REPLACE INTO embeddings (node_id, vector, dimension) VALUES (?, ?, ?)",
        (
            embedding.node_id.to_string(),
            bytes,
            embedding.dimension as i64,
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
}

// =========================================================================
// Session Repository implementations and helpers
// =========================================================================

fn save_session_conn(
    db: &ActiveConnection<'_>,
    id: &SessionId,
    history: &Conversation,
) -> Result<(), BrainError> {
    let history_str = serde_json::to_string(history).map_err(|e| BrainError::Storage {
        message: format!("Failed to serialize session history: {}", e),
        source: Some(Box::new(e)),
    })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    db.execute(
        "INSERT OR REPLACE INTO sessions (id, history, updated_at) VALUES (?, ?, ?)",
        (id.to_string(), history_str, now),
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
) -> Result<Option<Conversation>, BrainError> {
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
            let conversation: Conversation =
                serde_json::from_str(&history_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize conversation: {}", e),
                    source: Some(Box::new(e)),
                })?;
            Ok(Some(conversation))
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
    fn save_session(&self, id: &SessionId, history: &Conversation) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let active = ActiveConnection::new(&conn);
        save_session_conn(&active, id, history)
    }

    fn load_session(&self, id: &SessionId) -> Result<Option<Conversation>, BrainError> {
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
    fn save_session(&self, id: &SessionId, history: &Conversation) -> Result<(), BrainError> {
        save_session_conn(self, id, history)
    }

    fn load_session(&self, id: &SessionId) -> Result<Option<Conversation>, BrainError> {
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
    db.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES (?, ?)",
        (key, val),
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to save config key {}: {}", key, e),
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

fn get_config_key_conn(db: &ActiveConnection<'_>, key: &str) -> Result<Option<String>, BrainError> {
    let mut stmt = db
        .prepare("SELECT value FROM config WHERE key = ?")
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
