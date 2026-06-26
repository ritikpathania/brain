use crate::connection::init_pool;
use crate::migrations::run_migrations;
use brain_core::errors::BrainError;
use brain_core::repositories::{
    ConfigRepository, EdgeRepository, EmbeddingRepository, NodeRepository, RepositorySet,
    SessionRepository, StorageTransaction,
};
use brain_domain::{Conversation, Edge, EdgeId, Embedding, Node, NodeId, NodeType, SessionId};
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

fn save_node_conn(db: &ActiveConnection<'_>, node: &Node) -> Result<(), BrainError> {
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

    db.execute(
        "INSERT OR REPLACE INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)",
        (
            node.id.to_string(),
            &node.label,
            node_type_str,
            properties_str,
            node.updated_at,
        ),
    )
    .map_err(|e| BrainError::Storage {
        message: format!("Failed to save node {}: {}", node.id, e),
        source: Some(Box::new(e)),
    })?;
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
        let mut stmt = db.prepare(
            "INSERT OR REPLACE INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        for node in nodes {
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
            stmt.execute((
                node.id.to_string(),
                &node.label,
                node_type_str,
                properties_str,
                node.updated_at,
            ))
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute save node: {}", e),
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
            &edge.relation,
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
                &edge.relation,
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
        (id.source.to_string(), id.target.to_string(), &id.relation),
        |row| {
            let weight: f64 = row.get(0)?;
            let updated_at: u64 = row.get(1)?;
            Ok((weight, updated_at))
        },
    );

    match res {
        Ok((weight, updated_at)) => {
            let mut edge = Edge::new(id.source, id.target, id.relation.clone(), weight);
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
        (id.source.to_string(), id.target.to_string(), &id.relation),
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
        let mut edge = Edge::new(source, target, relation, weight);
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
        let mut edge = Edge::new(source, target, relation, weight);
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
