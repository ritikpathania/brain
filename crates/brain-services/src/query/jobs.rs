use std::sync::Arc;
use brain_core::errors::BrainError;
use brain_domain::jobs::{JobId, JobKind, JobOwner, JobState, JobPriority, JobProgress, JobTimestamp, JobFailureReason};
use brain_storage::{SqliteJobReadModelRepository, JobReadModel};
use crate::query::dto::{JobSummary, JobDetails};
use crate::query::filters::JobQuery;
use crate::query::traits::JobQueryService;

/// Concrete implementation of `JobQueryService` backing by Sqlite projection read models.
pub struct SqliteJobQueryService {
    repo: Arc<SqliteJobReadModelRepository>,
}

impl SqliteJobQueryService {
    /// Creates a new `SqliteJobQueryService` instance.
    pub fn new(repo: Arc<SqliteJobReadModelRepository>) -> Self {
        Self { repo }
    }
}

// Module-local mapper functions to map database projection models to Query DTOs.
fn map_to_summary(row: JobReadModel) -> Result<JobSummary, BrainError> {
    let kind: JobKind = serde_json::from_str(&format!("\"{}\"", row.kind))
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to deserialize job kind: {}", e),
            source: Some(Box::new(e)),
        })?;

    let owner = if row.owner == "system" {
        JobOwner::System
    } else if let Some(username) = row.owner.strip_prefix("user:") {
        JobOwner::User {
            username: username.to_string(),
        }
    } else if let Some(session_str) = row.owner.strip_prefix("session:") {
        let ulid = ulid::Ulid::from_string(session_str)
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to parse session ID in job owner: {}", e),
                source: Some(Box::new(e)),
            })?;
        JobOwner::Session {
            session_id: brain_domain::SessionId(ulid),
        }
    } else {
        return Err(BrainError::Storage {
            message: format!("Unknown job owner format: {}", row.owner),
            source: None,
        });
    };

    let state: JobState = serde_json::from_str(&format!("\"{}\"", row.state))
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to deserialize job state: {}", e),
            source: Some(Box::new(e)),
        })?;

    // Determine priority and progress
    let priority = match row.priority {
        0 => JobPriority::Low,
        1 => JobPriority::Normal,
        2 => JobPriority::High,
        3 => JobPriority::Critical,
        _ => JobPriority::Normal,
    };

    let progress = if row.progress == u32::MAX {
        JobProgress::Indeterminate
    } else {
        JobProgress::Determinate {
            completed: row.progress as u64,
            total: 100,
            unit: brain_domain::jobs::ProgressUnit::Items,
        }
    };

    Ok(JobSummary {
        job_id: JobId(row.job_id),
        kind,
        owner,
        state,
        priority,
        progress,
    })
}

fn map_to_details(row: JobReadModel) -> Result<JobDetails, BrainError> {
    let summary = map_to_summary(row.clone())?;

    Ok(JobDetails {
        job_id: summary.job_id,
        kind: summary.kind,
        owner: summary.owner,
        state: summary.state,
        priority: summary.priority,
        progress: summary.progress,
        started_at: row.started_at.map(JobTimestamp),
        completed_at: row.completed_at.map(JobTimestamp),
        failure_reason: row.failure_reason.map(JobFailureReason),
    })
}

impl JobQueryService for SqliteJobQueryService {
    fn list_jobs(&self, query: JobQuery) -> Result<Vec<JobSummary>, BrainError> {
        let owner_str = query.owner.map(|o| match o {
            JobOwner::System => "system".to_string(),
            JobOwner::User { username } => format!("user:{}", username),
            JobOwner::Session { session_id } => format!("session:{}", session_id.0),
        });

        let state_str = query.state.map(|s| {
            serde_json::to_string(&s)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string()
        });

        let (limit, offset) = match query.pagination {
            Some(pag) => (pag.limit, pag.offset),
            None => (None, None),
        };

        let rows = self.repo.query(
            owner_str.as_deref(),
            state_str.as_deref(),
            limit,
            offset,
        )?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(map_to_summary(row)?);
        }

        Ok(summaries)
    }

    fn get_job(&self, id: &JobId) -> Result<Option<JobDetails>, BrainError> {
        match self.repo.find_by_id(&id.0)? {
            Some(row) => Ok(Some(map_to_details(row)?)),
            None => Ok(None),
        }
    }
}
