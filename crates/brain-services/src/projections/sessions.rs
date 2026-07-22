use brain_core::errors::BrainError;
use std::sync::Arc;

use crate::projections::{ProjectionId, StateReducer};
use brain_events::{DomainEvent, EventEnvelope};
use brain_storage::{SessionReadModel, SqliteSessionReadModelRepository};

/// Reducer implementing state reductions for session lifecycle events onto SQLite read models.
pub struct SessionProjectionReducer {
    repo: Arc<SqliteSessionReadModelRepository>,
}

impl SessionProjectionReducer {
    /// Creates a new `SessionProjectionReducer` instance.
    pub fn new(repo: Arc<SqliteSessionReadModelRepository>) -> Self {
        Self { repo }
    }
}

impl StateReducer for SessionProjectionReducer {
    fn id(&self) -> ProjectionId {
        ProjectionId::Sessions
    }

    fn version(&self) -> u32 {
        1
    }

    fn reduce(
        &self,
        conn: &rusqlite::Connection,
        envelope: &EventEnvelope,
    ) -> Result<(), BrainError> {
        let seq = envelope.sequence.ok_or_else(|| BrainError::Storage {
            message: "Sequence missing on event envelope during sessions reduction".to_string(),
            source: None,
        })?;

        // Extract core domain event
        let domain_event = match &envelope.payload {
            DomainEvent::Core(e) => e,
            _ => return Ok(()), // Ignore non-core events
        };

        match domain_event {
            brain_domain::DomainEvent::SessionCreated {
                session_id,
                title,
                created_at,
            } => {
                let model = SessionReadModel {
                    session_id: *session_id,
                    title: title.0.clone(),
                    is_archived: false,
                    is_pinned: false,
                    created_at: *created_at,
                    updated_at: *created_at,
                    updated_sequence: seq,
                };
                self.repo.save_conn(conn, &model)?;
            }
            brain_domain::DomainEvent::SessionRenamed {
                session_id,
                title,
                updated_at,
            } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, session_id)? {
                    model.title = title.0.clone();
                    model.updated_at = *updated_at;
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::SessionPinnedChanged {
                session_id,
                pinned,
                updated_at,
            } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, session_id)? {
                    model.is_pinned = *pinned;
                    model.updated_at = *updated_at;
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::SessionArchived {
                session_id,
                updated_at,
            } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, session_id)? {
                    model.is_archived = true;
                    model.updated_at = *updated_at;
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::SessionRestored {
                session_id,
                updated_at,
            } => {
                if let Some(mut model) = self.repo.find_by_id_conn(conn, session_id)? {
                    model.is_archived = false;
                    model.updated_at = *updated_at;
                    model.updated_sequence = seq;
                    self.repo.save_conn(conn, &model)?;
                }
            }
            brain_domain::DomainEvent::SessionDeleted { session_id } => {
                // This is a projection policy to reflect active/live sessions
                self.repo.delete_conn(conn, session_id)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn reset(&self, conn: &rusqlite::Connection) -> Result<(), BrainError> {
        self.repo.clear_all_conn(conn)
    }
}
