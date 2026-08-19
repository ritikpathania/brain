use crate::projections::{ProjectionId, StateReducer};
use brain_core::errors::BrainError;
use brain_events::{DomainEvent, EventEnvelope};
use brain_storage::{JobReadModel, SqliteJobReadModelRepository};
use std::sync::Arc;

/// Reducer implementing state reductions for background job state changes onto SQLite read models.
pub struct JobProjectionReducer {
    repo: Arc<SqliteJobReadModelRepository>,
}

impl JobProjectionReducer {
    /// Creates a new `JobProjectionReducer` instance.
    pub fn new(repo: Arc<SqliteJobReadModelRepository>) -> Self {
        Self { repo }
    }
}

impl StateReducer for JobProjectionReducer {
    fn id(&self) -> ProjectionId {
        ProjectionId::Jobs
    }

    fn version(&self) -> u32 {
        1
    }

    fn reduce(
        &self,
        conn: &brain_storage::Connection,
        envelope: &EventEnvelope,
    ) -> Result<(), BrainError> {
        let seq = envelope.sequence.ok_or_else(|| BrainError::Storage {
            message: "Sequence missing on event envelope during jobs reduction".to_string(),
            source: None,
        })?;

        // Extract core domain event
        let domain_event = match &envelope.payload {
            DomainEvent::Core(e) => e,
            _ => return Ok(()), // Ignore non-core events
        };

        match domain_event {
            brain_domain::DomainEvent::JobCreated {
                job_id,
                kind,
                priority,
                owner,
            } => {
                let owner_str = match owner {
                    brain_domain::jobs::JobOwner::System => "system".to_string(),
                    brain_domain::jobs::JobOwner::User { username } => format!("user:{}", username),
                    brain_domain::jobs::JobOwner::Session { session_id } => {
                        format!("session:{}", session_id.0)
                    }
                };
                let kind_str = match kind {
                    brain_domain::jobs::JobKind::Tool => "tool".to_string(),
                    brain_domain::jobs::JobKind::Retrieval => "retrieval".to_string(),
                    brain_domain::jobs::JobKind::Indexing => "indexing".to_string(),
                    brain_domain::jobs::JobKind::Sync => "sync".to_string(),
                    brain_domain::jobs::JobKind::Compilation => "compilation".to_string(),
                    brain_domain::jobs::JobKind::Maintenance => "maintenance".to_string(),
                };
                let priority_val = match priority {
                    brain_domain::jobs::JobPriority::Critical => 0,
                    brain_domain::jobs::JobPriority::High => 1,
                    brain_domain::jobs::JobPriority::Normal => 2,
                    brain_domain::jobs::JobPriority::Low => 3,
                };

                let model = JobReadModel {
                    job_id: job_id.0,
                    kind: kind_str,
                    owner: owner_str,
                    state: "pending".to_string(),
                    priority: priority_val,
                    progress: 0,
                    started_at: None,
                    completed_at: None,
                    failure_reason: None,
                    updated_sequence: seq,
                };
                self.repo.save_conn(conn, &model)?;
            }
            brain_domain::DomainEvent::JobStarted { job_id, timestamp } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, &job_id.0)? {
                    model.state = "running".to_string();
                    model.started_at = Some(timestamp.0);
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::JobProgressed { job_id, progress } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, &job_id.0)? {
                    let progress_pct = match progress {
                        brain_domain::jobs::JobProgress::Determinate {
                            completed, total, ..
                        } => {
                            if *total > 0 {
                                (completed * 100 / total) as u32
                            } else {
                                0
                            }
                        }
                        brain_domain::jobs::JobProgress::Indeterminate => 0,
                    };
                    model.progress = progress_pct;
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::JobWaiting { job_id, .. } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, &job_id.0)? {
                    model.state = "waiting".to_string();
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::JobCompleted { job_id, timestamp } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, &job_id.0)? {
                    model.state = "completed".to_string();
                    model.completed_at = Some(timestamp.0);
                    model.progress = 100;
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::JobFailed {
                job_id,
                reason,
                timestamp,
            } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, &job_id.0)? {
                    model.state = "failed".to_string();
                    model.completed_at = Some(timestamp.0);
                    model.failure_reason = Some(reason.0.clone());
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::JobCancelled { job_id, timestamp } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, &job_id.0)? {
                    model.state = "cancelled".to_string();
                    model.completed_at = Some(timestamp.0);
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn reset(&self, conn: &brain_storage::Connection) -> Result<(), BrainError> {
        self.repo.clear_all_conn(conn)
    }
}
