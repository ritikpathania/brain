use crate::projections::{ProjectionId, StateReducer};
use brain_core::errors::BrainError;
use brain_domain::{SearchDocument, SearchDocumentId, SearchDocumentKind, SearchMetadata};
use brain_events::{DomainEvent, EventEnvelope};
use brain_storage::SqliteSearchRepository;
use std::sync::Arc;

/// Stateful FTS5 search projection reducer translating session and message lifecycles to indexed documents.
pub struct SearchProjectionReducer {
    repo: Arc<SqliteSearchRepository>,
}

impl SearchProjectionReducer {
    /// Creates a new `SearchProjectionReducer` instance.
    pub fn new(repo: Arc<SqliteSearchRepository>) -> Self {
        Self { repo }
    }
}

impl StateReducer for SearchProjectionReducer {
    fn id(&self) -> ProjectionId {
        ProjectionId::Search
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
            message: "Sequence missing on event envelope during search reduction".to_string(),
            source: None,
        })?;

        match &envelope.payload {
            DomainEvent::Core(brain_domain::DomainEvent::SessionCreated {
                session_id,
                title,
                ..
            }) => {
                let doc_id = SearchDocumentId::new(format!("session:{}", session_id));
                let doc = SearchDocument::new(
                    doc_id,
                    SearchDocumentKind::Session,
                    title.0.clone(),
                    "".to_string(),
                    SearchMetadata::Session {
                        archived: false,
                        pinned: false,
                    },
                );
                self.repo.save_conn(conn, &doc, seq)?;
            }
            DomainEvent::Core(brain_domain::DomainEvent::SessionRenamed {
                session_id,
                title,
                ..
            }) => {
                let doc_id = SearchDocumentId::new(format!("session:{}", session_id));
                let (archived, pinned) =
                    if let Some(existing) = self.repo.find_by_id_conn(conn, &doc_id)? {
                        match existing.metadata {
                            SearchMetadata::Session { archived, pinned } => (archived, pinned),
                            _ => (false, false),
                        }
                    } else {
                        (false, false)
                    };
                let doc = SearchDocument::new(
                    doc_id,
                    SearchDocumentKind::Session,
                    title.0.clone(),
                    "".to_string(),
                    SearchMetadata::Session { archived, pinned },
                );
                self.repo.save_conn(conn, &doc, seq)?;
            }
            DomainEvent::Core(brain_domain::DomainEvent::SessionPinnedChanged {
                session_id,
                pinned,
                ..
            }) => {
                let doc_id = SearchDocumentId::new(format!("session:{}", session_id));
                let (archived, title) =
                    if let Some(existing) = self.repo.find_by_id_conn(conn, &doc_id)? {
                        let archived = match existing.metadata {
                            SearchMetadata::Session { archived, .. } => archived,
                            _ => false,
                        };
                        (archived, existing.title)
                    } else {
                        (false, "".to_string())
                    };
                let doc = SearchDocument::new(
                    doc_id,
                    SearchDocumentKind::Session,
                    title,
                    "".to_string(),
                    SearchMetadata::Session {
                        archived,
                        pinned: *pinned,
                    },
                );
                self.repo.save_conn(conn, &doc, seq)?;
            }
            DomainEvent::Core(brain_domain::DomainEvent::SessionArchived {
                session_id, ..
            }) => {
                let doc_id = SearchDocumentId::new(format!("session:{}", session_id));
                let (pinned, title) =
                    if let Some(existing) = self.repo.find_by_id_conn(conn, &doc_id)? {
                        let pinned = match existing.metadata {
                            SearchMetadata::Session { pinned, .. } => pinned,
                            _ => false,
                        };
                        (pinned, existing.title)
                    } else {
                        (false, "".to_string())
                    };
                let doc = SearchDocument::new(
                    doc_id,
                    SearchDocumentKind::Session,
                    title,
                    "".to_string(),
                    SearchMetadata::Session {
                        archived: true,
                        pinned,
                    },
                );
                self.repo.save_conn(conn, &doc, seq)?;
            }
            DomainEvent::Core(brain_domain::DomainEvent::SessionRestored {
                session_id, ..
            }) => {
                let doc_id = SearchDocumentId::new(format!("session:{}", session_id));
                let (pinned, title) =
                    if let Some(existing) = self.repo.find_by_id_conn(conn, &doc_id)? {
                        let pinned = match existing.metadata {
                            SearchMetadata::Session { pinned, .. } => pinned,
                            _ => false,
                        };
                        (pinned, existing.title)
                    } else {
                        (false, "".to_string())
                    };
                let doc = SearchDocument::new(
                    doc_id,
                    SearchDocumentKind::Session,
                    title,
                    "".to_string(),
                    SearchMetadata::Session {
                        archived: false,
                        pinned,
                    },
                );
                self.repo.save_conn(conn, &doc, seq)?;
            }
            DomainEvent::Core(brain_domain::DomainEvent::SessionDeleted { session_id }) => {
                let doc_id = SearchDocumentId::new(format!("session:{}", session_id));
                self.repo.delete_conn(conn, &doc_id)?;
                self.repo.delete_by_session_id_conn(conn, session_id)?;
            }
            DomainEvent::Core(brain_domain::DomainEvent::MessageAdded {
                session_id,
                message,
            }) => {
                let doc_id = SearchDocumentId::new(format!("message:{}", message.id));
                let doc = SearchDocument::new(
                    doc_id,
                    SearchDocumentKind::Message,
                    "".to_string(),
                    message.content.clone(),
                    SearchMetadata::Message {
                        session_id: *session_id,
                        role: message.role,
                    },
                );
                self.repo.save_conn(conn, &doc, seq)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reset(&self, conn: &brain_storage::Connection) -> Result<(), BrainError> {
        self.repo.clear_all_conn(conn)
    }
}
