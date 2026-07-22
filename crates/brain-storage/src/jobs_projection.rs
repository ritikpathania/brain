//! SQLite-backed repository for tracking job read model projections.

use brain_core::errors::BrainError;
use rusqlite::params;
use uuid::Uuid;

/// Read model state representing a simplified view of a background job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobReadModel {
    /// Unique job identifier.
    pub job_id: Uuid,
    /// Categorical task type.
    pub kind: String,
    /// Scope context owner (e.g. system, user:username, session:session_id).
    pub owner: String,
    /// Lifecycle state.
    pub state: String,
    /// Integer-mapped precedence priority tier.
    pub priority: u32,
    /// Metric percentage completed.
    pub progress: u32,
    /// Start execution timestamp.
    pub started_at: Option<u64>,
    /// Ending timestamp.
    pub completed_at: Option<u64>,
    /// Optional failure reason.
    pub failure_reason: Option<String>,
    /// Last sequence number that updated this row.
    pub updated_sequence: u64,
}

/// SQLite repository interface for mutating and querying jobs projection read models.
pub struct SqliteJobReadModelRepository {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteJobReadModelRepository {
    /// Creates a new `SqliteJobReadModelRepository` instance.
    pub fn new(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Saves a job read model state (insert or update).
    pub fn save(&self, model: &JobReadModel) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.save_conn(&conn, model)
    }

    /// Saves a job read model state using a shared transaction connection.
    pub fn save_conn(
        &self,
        conn: &rusqlite::Connection,
        model: &JobReadModel,
    ) -> Result<(), BrainError> {
        conn.execute(
            "INSERT INTO jobs_projection (job_id, kind, owner, state, priority, progress, started_at, completed_at, failure_reason, updated_sequence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(job_id) DO UPDATE SET
                 state = excluded.state,
                 progress = excluded.progress,
                 started_at = COALESCE(excluded.started_at, jobs_projection.started_at),
                 completed_at = COALESCE(excluded.completed_at, jobs_projection.completed_at),
                 failure_reason = COALESCE(excluded.failure_reason, jobs_projection.failure_reason),
                 updated_sequence = excluded.updated_sequence",
            params![
                model.job_id.to_string(),
                model.kind,
                model.owner,
                model.state,
                model.priority as i64,
                model.progress as i64,
                model.started_at.map(|t| t as i64),
                model.completed_at.map(|t| t as i64),
                model.failure_reason,
                model.updated_sequence as i64,
            ],
        ).map_err(|e| BrainError::Storage {
            message: format!("Failed to save job read model: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(())
    }

    /// Finds a job read model state by ID.
    pub fn find_by_id(&self, job_id: &Uuid) -> Result<Option<JobReadModel>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.find_by_id_conn(&conn, job_id)
    }

    /// Finds a job read model state by ID using a shared transaction connection.
    pub fn find_by_id_conn(
        &self,
        conn: &rusqlite::Connection,
        job_id: &Uuid,
    ) -> Result<Option<JobReadModel>, BrainError> {
        let res: Result<JobReadModel, rusqlite::Error> = conn.query_row(
            "SELECT job_id, kind, owner, state, priority, progress, started_at, completed_at, failure_reason, updated_sequence
             FROM jobs_projection
             WHERE job_id = ?1",
            params![job_id.to_string()],
            |row| {
                let id_str: String = row.get(0)?;
                let kind: String = row.get(1)?;
                let owner: String = row.get(2)?;
                let state: String = row.get(3)?;
                let priority: i64 = row.get(4)?;
                let progress: i64 = row.get(5)?;
                let started_at: Option<i64> = row.get(6)?;
                let completed_at: Option<i64> = row.get(7)?;
                let failure_reason: Option<String> = row.get(8)?;
                let updated_sequence: i64 = row.get(9)?;

                let parsed_id = Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

                Ok(JobReadModel {
                    job_id: parsed_id,
                    kind,
                    owner,
                    state,
                    priority: priority as u32,
                    progress: progress as u32,
                    started_at: started_at.map(|t| t as u64),
                    completed_at: completed_at.map(|t| t as u64),
                    failure_reason,
                    updated_sequence: updated_sequence as u64,
                })
            },
        );

        match res {
            Ok(model) => Ok(Some(model)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Storage {
                message: format!("Failed to query job read model: {}", e),
                source: Some(Box::new(e)),
            }),
        }
    }

    /// Deletes a job read model state.
    pub fn delete(&self, job_id: &Uuid) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        conn.execute(
            "DELETE FROM jobs_projection WHERE job_id = ?1",
            params![job_id.to_string()],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete job read model: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Clears all job read model states (used for rebuilds).
    pub fn clear_all(&self) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.clear_all_conn(&conn)
    }

    /// Clears all job read model states using a shared transaction connection.
    pub fn clear_all_conn(&self, conn: &rusqlite::Connection) -> Result<(), BrainError> {
        conn.execute("DELETE FROM jobs_projection", [])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to clear jobs projection table: {}", e),
                source: Some(Box::new(e)),
            })?;

        Ok(())
    }

    /// Queries job read models matching optional filters and pagination specifications.
    pub fn query(
        &self,
        owner: Option<&str>,
        state: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<JobReadModel>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let mut sql = "SELECT job_id, kind, owner, state, priority, progress, started_at, completed_at, failure_reason, updated_sequence FROM jobs_projection".to_string();
        let mut filters = Vec::new();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
        let mut param_index = 1;

        if let Some(owner_str) = owner {
            filters.push(format!("owner = ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Text(owner_str.to_string()));
            param_index += 1;
        }

        if let Some(state_str) = state {
            filters.push(format!("state = ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Text(state_str.to_string()));
            param_index += 1;
        }

        if !filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&filters.join(" AND "));
        }

        sql.push_str(" ORDER BY updated_sequence DESC, job_id ASC");

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
            message: format!("Failed to prepare query jobs statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|v| v as &dyn rusqlite::ToSql)
            .collect();

        let mapped = stmt
            .query_map(&params_ref[..], |row| {
                let id_str: String = row.get(0)?;
                let kind: String = row.get(1)?;
                let owner: String = row.get(2)?;
                let state: String = row.get(3)?;
                let priority: i64 = row.get(4)?;
                let progress: i64 = row.get(5)?;
                let started_at: Option<i64> = row.get(6)?;
                let completed_at: Option<i64> = row.get(7)?;
                let failure_reason: Option<String> = row.get(8)?;
                let updated_sequence: i64 = row.get(9)?;

                let parsed_id = Uuid::parse_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(JobReadModel {
                    job_id: parsed_id,
                    kind,
                    owner,
                    state,
                    priority: priority as u32,
                    progress: progress as u32,
                    started_at: started_at.map(|t| t as u64),
                    completed_at: completed_at.map(|t| t as u64),
                    failure_reason,
                    updated_sequence: updated_sequence as u64,
                })
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute query jobs query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for item in mapped {
            results.push(item.map_err(|e| BrainError::Storage {
                message: format!("Failed to map job read model row: {}", e),
                source: Some(Box::new(e)),
            })?);
        }

        Ok(results)
    }
}

impl crate::sessions_projection::ReadModelRepository<JobReadModel, Uuid>
    for SqliteJobReadModelRepository
{
    fn save(&self, model: &JobReadModel) -> Result<(), BrainError> {
        self.save(model)
    }

    fn delete(&self, id: &Uuid) -> Result<(), BrainError> {
        self.delete(id)
    }

    fn clear_all(&self) -> Result<(), BrainError> {
        self.clear_all()
    }
}
