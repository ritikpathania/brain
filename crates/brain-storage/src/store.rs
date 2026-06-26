use crate::connection::init_pool;
use crate::migrations::run_migrations;
use brain_core::errors::BrainError;
use brain_core::repositories::{EdgeRepository, NodeRepository};
use brain_domain::{Edge, EdgeId, Node, NodeId, NodeType};
use std::collections::HashMap;

/// SQLite database storage backend implementing all domain repositories.
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
}

impl NodeRepository for SqliteStorage {
    fn save(&self, node: &Node) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let node_type_str = serde_json::to_string(&node.node_type).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize node type: {}", e),
            source: Some(Box::new(e)),
        })?;
        let properties_str = serde_json::to_string(&node.properties).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize properties: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
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

    fn save_batch(&self, nodes: &[Node]) -> Result<(), BrainError> {
        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to start transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)"
            ).map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare statement: {}", e),
                source: Some(Box::new(e)),
            })?;

            for node in nodes {
                let node_type_str = serde_json::to_string(&node.node_type).map_err(|e| BrainError::Storage {
                    message: format!("Failed to serialize node type: {}", e),
                    source: Some(Box::new(e)),
                })?;
                let properties_str = serde_json::to_string(&node.properties).map_err(|e| BrainError::Storage {
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
        }

        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit transaction: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(())
    }

    fn find_by_id(&self, id: &NodeId) -> Result<Option<Node>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn
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
                let node_type: NodeType = serde_json::from_str(&node_type_str).map_err(|e| BrainError::Storage {
                    message: format!("Failed to deserialize node type: {}", e),
                    source: Some(Box::new(e)),
                })?;
                let properties: HashMap<String, serde_json::Value> = serde_json::from_str(&properties_str).map_err(|e| BrainError::Storage {
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

    fn delete(&self, id: &NodeId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        conn.execute("DELETE FROM nodes WHERE id = ?", [id.to_string()])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to delete node {}: {}", id, e),
                source: Some(Box::new(e)),
            })?;
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Node>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let mut stmt = conn
            .prepare("SELECT id, label, node_type, properties, updated_at FROM nodes")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare statement: {}", e),
                source: Some(Box::new(e)),
            })?;

        let node_iter = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let label: String = row.get(1)?;
            let node_type_str: String = row.get(2)?;
            let properties_str: String = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((id_str, label, node_type_str, properties_str, updated_at))
        }).map_err(|e| BrainError::Storage {
            message: format!("Query execution failed: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut nodes = Vec::new();
        for item in node_iter {
            let (id_str, label, node_type_str, properties_str, updated_at) = item.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse query row: {}", e),
                source: Some(Box::new(e)),
            })?;
            let id = uuid::Uuid::parse_str(&id_str)
                .map(NodeId)
                .map_err(|e| BrainError::Storage {
                    message: format!("Invalid UUID in storage: {}", e),
                    source: Some(Box::new(e)),
                })?;
            let node_type: NodeType = serde_json::from_str(&node_type_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize node type: {}", e),
                source: Some(Box::new(e)),
            })?;
            let properties: HashMap<String, serde_json::Value> = serde_json::from_str(&properties_str).map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize properties: {}", e),
                source: Some(Box::new(e)),
            })?;
            let mut node = Node::new(id, label, node_type).with_properties(properties);
            node.updated_at = updated_at;
            nodes.push(node);
        }

        Ok(nodes)
    }
}

impl EdgeRepository for SqliteStorage {
    fn save(&self, edge: &Edge) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        conn.execute(
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

    fn save_batch(&self, edges: &[Edge]) -> Result<(), BrainError> {
        let mut conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        let tx = conn.transaction().map_err(|e| BrainError::Storage {
            message: format!("Failed to start transaction: {}", e),
            source: Some(Box::new(e)),
        })?;

        {
            let mut stmt = tx.prepare(
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
        }

        tx.commit().map_err(|e| BrainError::Storage {
            message: format!("Failed to commit transaction: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(())
    }

    fn find_by_id(&self, id: &EdgeId) -> Result<Option<Edge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn
            .prepare("SELECT weight, updated_at FROM edges WHERE source = ? AND target = ? AND relation = ?")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let res = stmt.query_row((id.source.to_string(), id.target.to_string(), &id.relation), |row| {
            let weight: f64 = row.get(0)?;
            let updated_at: u64 = row.get(1)?;
            Ok((weight, updated_at))
        });

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

    fn delete(&self, id: &EdgeId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        conn.execute(
            "DELETE FROM edges WHERE source = ? AND target = ? AND relation = ?",
            (id.source.to_string(), id.target.to_string(), &id.relation),
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete edge: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(())
    }

    fn get_connections(&self, node_id: &NodeId) -> Result<Vec<Edge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn
            .prepare("SELECT source, target, relation, weight, updated_at FROM edges WHERE source = ? OR target = ?")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare connections query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let edge_iter = stmt.query_map([node_id.to_string(), node_id.to_string()], |row| {
            let src_str: String = row.get(0)?;
            let tgt_str: String = row.get(1)?;
            let relation: String = row.get(2)?;
            let weight: f64 = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((src_str, tgt_str, relation, weight, updated_at))
        }).map_err(|e| BrainError::Storage {
            message: format!("Failed to query connections: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut edges = Vec::new();
        for item in edge_iter {
            let (src_str, tgt_str, relation, weight, updated_at) = item.map_err(|e| BrainError::Storage {
                message: format!("Failed parsing connection row: {}", e),
                source: Some(Box::new(e)),
            })?;
            let source = uuid::Uuid::parse_str(&src_str).map(NodeId).map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID in storage: {}", e),
                source: Some(Box::new(e)),
            })?;
            let target = uuid::Uuid::parse_str(&tgt_str).map(NodeId).map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID in storage: {}", e),
                source: Some(Box::new(e)),
            })?;
            let mut edge = Edge::new(source, target, relation, weight);
            edge.updated_at = updated_at;
            edges.push(edge);
        }
        Ok(edges)
    }

    fn list_all(&self) -> Result<Vec<Edge>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn
            .prepare("SELECT source, target, relation, weight, updated_at FROM edges")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let edge_iter = stmt.query_map([], |row| {
            let src_str: String = row.get(0)?;
            let tgt_str: String = row.get(1)?;
            let relation: String = row.get(2)?;
            let weight: f64 = row.get(3)?;
            let updated_at: u64 = row.get(4)?;
            Ok((src_str, tgt_str, relation, weight, updated_at))
        }).map_err(|e| BrainError::Storage {
            message: format!("Failed to query edges list: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut edges = Vec::new();
        for item in edge_iter {
            let (src_str, tgt_str, relation, weight, updated_at) = item.map_err(|e| BrainError::Storage {
                message: format!("Failed parsing edge row: {}", e),
                source: Some(Box::new(e)),
            })?;
            let source = uuid::Uuid::parse_str(&src_str).map(NodeId).map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID in storage: {}", e),
                source: Some(Box::new(e)),
            })?;
            let target = uuid::Uuid::parse_str(&tgt_str).map(NodeId).map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID in storage: {}", e),
                source: Some(Box::new(e)),
            })?;
            let mut edge = Edge::new(source, target, relation, weight);
            edge.updated_at = updated_at;
            edges.push(edge);
        }
        Ok(edges)
    }
}