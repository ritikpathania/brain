//! SQLite-backed repository for tracking session read model projections.

use brain_core::errors::BrainError;
use brain_domain::{SessionId, SessionTimestamp};
use rusqlite::params;

/// Generic interface for read model repositories.
pub trait ReadModelRepository<T, K>: Send + Sync {
    /// Saves a read model (insert or update).
    fn save(&self, model: &T) -> Result<(), BrainError>;
    /// Deletes a read model by its key.
    fn delete(&self, id: &K) -> Result<(), BrainError>;
    /// Clears all read models of this type.
    fn clear_all(&self) -> Result<(), BrainError>;
}

/// Read model state representing a simplified view of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReadModel {
    /// Session identity.
    pub session_id: SessionId,
    /// Title description.
    pub title: String,
    /// Is the session archived.
    pub is_archived: bool,
    /// Is the session pinned.
    pub is_pinned: bool,
    /// Creation timestamp.
    pub created_at: SessionTimestamp,
    /// Updation timestamp.
    pub updated_at: SessionTimestamp,
    /// Last sequence sequence.
    pub updated_sequence: u64,
}

/// SQLite repository interface for mutating and querying sessions projection read models.
pub struct SqliteSessionReadModelRepository {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteSessionReadModelRepository {
    /// Creates a new `SqliteSessionReadModelRepository` instance.
    pub fn new(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Finds a session read model state by ID.
    pub fn find_by_id(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionReadModel>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let res: Result<SessionReadModel, rusqlite::Error> = conn.query_row(
            "SELECT session_id, title, is_archived, is_pinned, created_at, updated_at, updated_sequence
             FROM sessions_projection
             WHERE session_id = ?1",
            params![session_id.to_string()],
            |row| {
                let id_str: String = row.get(0)?;
                let title: String = row.get(1)?;
                let is_archived: bool = row.get(2)?;
                let is_pinned: bool = row.get(3)?;
                let created_at: i64 = row.get(4)?;
                let updated_at: i64 = row.get(5)?;
                let updated_sequence: i64 = row.get(6)?;

                let parsed_id = SessionId(
                    ulid::Ulid::from_string(&id_str)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?
                );

                Ok(SessionReadModel {
                    session_id: parsed_id,
                    title,
                    is_archived,
                    is_pinned,
                    created_at: SessionTimestamp(created_at as u64),
                    updated_at: SessionTimestamp(updated_at as u64),
                    updated_sequence: updated_sequence as u64,
                })
            },
        );

        match res {
            Ok(model) => Ok(Some(model)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Storage {
                message: format!("Failed to query session read model: {}", e),
                source: Some(Box::new(e)),
            }),
        }
    }

    /// Lists all session read models ordered by pinned status, update timestamp, and session ID.
    pub fn list_all(&self) -> Result<Vec<SessionReadModel>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut stmt = conn.prepare(
            "SELECT session_id, title, is_archived, is_pinned, created_at, updated_at, updated_sequence
             FROM sessions_projection
             ORDER BY is_pinned DESC, updated_at DESC, session_id ASC"
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare list sessions query: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mapped = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let title: String = row.get(1)?;
                let is_archived: bool = row.get(2)?;
                let is_pinned: bool = row.get(3)?;
                let created_at: i64 = row.get(4)?;
                let updated_at: i64 = row.get(5)?;
                let updated_sequence: i64 = row.get(6)?;

                let parsed_id = SessionId(ulid::Ulid::from_string(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?);

                Ok(SessionReadModel {
                    session_id: parsed_id,
                    title,
                    is_archived,
                    is_pinned,
                    created_at: SessionTimestamp(created_at as u64),
                    updated_at: SessionTimestamp(updated_at as u64),
                    updated_sequence: updated_sequence as u64,
                })
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute list sessions query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for item in mapped {
            results.push(item.map_err(|e| BrainError::Storage {
                message: format!("Failed to map session read model row: {}", e),
                source: Some(Box::new(e)),
            })?);
        }

        Ok(results)
    }

    /// Queries session read models matching optional filters and pagination specifications.
    pub fn query(
        &self,
        is_archived: Option<bool>,
        is_pinned: Option<bool>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<SessionReadModel>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut sql = "SELECT session_id, title, is_archived, is_pinned, created_at, updated_at, updated_sequence FROM sessions_projection".to_string();
        let mut filters = Vec::new();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
        let mut param_index = 1;

        if let Some(archived) = is_archived {
            filters.push(format!("is_archived = ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Integer(if archived {
                1
            } else {
                0
            }));
            param_index += 1;
        }

        if let Some(pinned) = is_pinned {
            filters.push(format!("is_pinned = ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Integer(if pinned { 1 } else { 0 }));
            param_index += 1;
        }

        if !filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&filters.join(" AND "));
        }

        sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC, session_id ASC");

        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Integer(l as i64));
            param_index += 1;
        }

        if let Some(o) = offset {
            sql.push_str(&format!(" OFFSET ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Integer(o as i64));
        }

        let mut stmt = conn.prepare(&sql).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare query sessions statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|v| v as &dyn rusqlite::ToSql)
            .collect();

        let mapped = stmt
            .query_map(&params_ref[..], |row| {
                let id_str: String = row.get(0)?;
                let title: String = row.get(1)?;
                let is_archived: bool = row.get(2)?;
                let is_pinned: bool = row.get(3)?;
                let created_at: i64 = row.get(4)?;
                let updated_at: i64 = row.get(5)?;
                let updated_sequence: i64 = row.get(6)?;

                let parsed_id = SessionId(ulid::Ulid::from_string(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?);

                Ok(SessionReadModel {
                    session_id: parsed_id,
                    title,
                    is_archived,
                    is_pinned,
                    created_at: SessionTimestamp(created_at as u64),
                    updated_at: SessionTimestamp(updated_at as u64),
                    updated_sequence: updated_sequence as u64,
                })
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute query sessions query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for item in mapped {
            results.push(item.map_err(|e| BrainError::Storage {
                message: format!("Failed to map session read model row: {}", e),
                source: Some(Box::new(e)),
            })?);
        }

        Ok(results)
    }
}

impl ReadModelRepository<SessionReadModel, SessionId> for SqliteSessionReadModelRepository {
    fn save(&self, model: &SessionReadModel) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "INSERT INTO sessions_projection (session_id, title, is_archived, is_pinned, created_at, updated_at, updated_sequence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET
                 title = excluded.title,
                 is_archived = excluded.is_archived,
                 is_pinned = excluded.is_pinned,
                 updated_at = excluded.updated_at,
                 updated_sequence = excluded.updated_sequence",
            params![
                model.session_id.to_string(),
                model.title,
                if model.is_archived { 1 } else { 0 },
                if model.is_pinned { 1 } else { 0 },
                model.created_at.0 as i64,
                model.updated_at.0 as i64,
                model.updated_sequence as i64,
            ],
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to save session read model: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    fn delete(&self, id: &SessionId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "DELETE FROM sessions_projection WHERE session_id = ?1",
            params![id.to_string()],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete session read model: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    fn clear_all(&self) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute("DELETE FROM sessions_projection", [])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to clear sessions projection table: {}", e),
                source: Some(Box::new(e)),
            })?;

        Ok(())
    }
}
