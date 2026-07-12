use std::sync::Arc;
use brain_core::errors::BrainError;
use brain_core::repositories::SessionRepository;
use brain_domain::{SessionId, SessionTitle, MessageTimestamp};
use brain_storage::{SqliteSessionReadModelRepository, SessionReadModel};
use crate::query::dto::{SessionSummary, SessionDetails, MessageDTO};
use crate::query::filters::SessionQuery;
use crate::query::traits::SessionQueryService;

/// Concrete implementation of `SessionQueryService` backing by Sqlite projection read models and core session store.
pub struct SqliteSessionQueryService {
    projection_repo: Arc<SqliteSessionReadModelRepository>,
    session_repo: Arc<dyn SessionRepository>,
}

impl SqliteSessionQueryService {
    /// Creates a new `SqliteSessionQueryService` instance.
    pub fn new(
        projection_repo: Arc<SqliteSessionReadModelRepository>,
        session_repo: Arc<dyn SessionRepository>,
    ) -> Self {
        Self {
            projection_repo,
            session_repo,
        }
    }
}

// Module-local mapper functions to map database projection models to Query DTOs.
fn map_to_summary(row: SessionReadModel) -> SessionSummary {
    SessionSummary {
        session_id: row.session_id,
        title: SessionTitle(row.title),
        is_archived: row.is_archived,
        is_pinned: row.is_pinned,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

impl SessionQueryService for SqliteSessionQueryService {
    fn list_sessions(&self, query: SessionQuery) -> Result<Vec<SessionSummary>, BrainError> {
        let (limit, offset) = match query.pagination {
            Some(pag) => (pag.limit, pag.offset),
            None => (None, None),
        };

        let rows = self.projection_repo.query(
            query.is_archived,
            query.is_pinned,
            limit,
            offset,
        )?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(map_to_summary(row));
        }

        Ok(summaries)
    }

    fn get_session(&self, id: &SessionId) -> Result<Option<SessionDetails>, BrainError> {
        // Query read model for general session state
        let read_model = match self.projection_repo.find_by_id(id)? {
            Some(rm) => rm,
            None => return Ok(None),
        };

        // Retrieve messages from the core session repository (abstraction over long term storage)
        let messages = match self.session_repo.load_session(id)? {
            Some(session) => session
                .messages
                .into_iter()
                .map(|msg| MessageDTO {
                    id: msg.id,
                    role: msg.role,
                    content: msg.content,
                    timestamp: MessageTimestamp(msg.timestamp),
                })
                .collect(),
            None => Vec::new(),
        };

        Ok(Some(SessionDetails {
            session_id: read_model.session_id,
            title: SessionTitle(read_model.title),
            is_archived: read_model.is_archived,
            is_pinned: read_model.is_pinned,
            created_at: read_model.created_at,
            updated_at: read_model.updated_at,
            messages,
        }))
    }
}
