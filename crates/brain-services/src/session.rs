use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::services::SessionService;
use brain_domain::{Session, SessionTitle, SessionTimestamp, Node, SessionId};
use brain_session::SessionCacheManager;
use std::sync::Arc;
use crate::jobs::publisher::DomainEventPublisher;

/// Concrete implementation of SessionService orchestrating persistence and volatile cache.
pub struct SessionServiceImpl {
    repos: Arc<dyn RepositorySet>,
    cache_manager: Arc<SessionCacheManager>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl SessionServiceImpl {
    /// Creates a new SessionServiceImpl.
    pub fn new(
        repos: Arc<dyn RepositorySet>,
        cache_manager: Arc<SessionCacheManager>,
        publisher: Arc<dyn DomainEventPublisher>,
    ) -> Self {
        Self {
            repos,
            cache_manager,
            publisher,
        }
    }
}

impl SessionService for SessionServiceImpl {
    fn create_session(&self) -> Result<SessionId, BrainError> {
        let session_id = SessionId::new();
        let timestamp = SessionTimestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        let mut session = Session::new(
            session_id,
            SessionTitle("New Session".to_string()),
            timestamp,
        );

        // Populate cache context
        self.cache_manager.get_or_create(session_id);

        // Persist session to database
        self.save_session(&session_id, &mut session)?;

        Ok(session_id)
    }

    fn session_exists(&self, id: &SessionId) -> Result<bool, BrainError> {
        if self.cache_manager.exists(id) {
            return Ok(true);
        }
        let session = self.repos.sessions().load_session(id)?;
        Ok(session.is_some())
    }

    fn load_session(&self, id: &SessionId) -> Result<Session, BrainError> {
        let session = self.repos.sessions().load_session(id)?;
        match session {
            Some(session_aggregate) => {
                // Ensure context is initialized in the cache
                self.cache_manager.get_or_create(*id);
                Ok(session_aggregate)
            }
            None => Err(BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            }),
        }
    }

    fn save_session(&self, id: &SessionId, session: &mut Session) -> Result<(), BrainError> {
        if !self.session_exists(id)? {
            return Err(BrainError::Session {
                session_id: *id,
                message: "Session does not exist".to_string(),
            });
        }
        self.repos.sessions().save_session(id, session)?;
        for event in session.drain_events() {
            self.publisher.publish(event);
        }
        Ok(())
    }

    fn ingest_node(&self, id: &SessionId, node: Node) -> Result<(), BrainError> {
        if !self.session_exists(id)? {
            return Err(BrainError::Session {
                session_id: *id,
                message: "Session does not exist".to_string(),
            });
        }
        // Save node to storage
        self.repos.nodes().save(&node)?;

        // Ingest node to session cache
        let ctx = self.cache_manager.get_or_create(*id);
        ctx.write().unwrap().ingest(node);

        Ok(())
    }

    fn delete_session(&self, id: &SessionId) -> Result<(), BrainError> {
        if let Ok(mut session) = self.load_session(id) {
            session.delete();
            for event in session.drain_events() {
                self.publisher.publish(event);
            }
        }
        self.repos.sessions().delete_session(id)?;
        self.cache_manager.remove(id);
        Ok(())
    }

    fn rename_session(&self, id: &SessionId, title: &str) -> Result<(), BrainError> {
        let mut session = self.load_session(id)?;
        let timestamp = SessionTimestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        session.rename(SessionTitle(title.to_string()), timestamp);
        self.save_session(id, &mut session)?;
        Ok(())
    }

    fn set_session_pinned(&self, id: &SessionId, pinned: bool) -> Result<(), BrainError> {
        let mut session = self.load_session(id)?;
        let timestamp = SessionTimestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        session.set_pinned(pinned, timestamp);
        self.save_session(id, &mut session)?;
        Ok(())
    }

    fn archive_session(&self, id: &SessionId) -> Result<(), BrainError> {
        let mut session = self.load_session(id)?;
        let timestamp = SessionTimestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        session.archive(timestamp)?;
        self.save_session(id, &mut session)?;
        Ok(())
    }

    fn restore_session(&self, id: &SessionId) -> Result<(), BrainError> {
        let mut session = self.load_session(id)?;
        let timestamp = SessionTimestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        session.restore(timestamp)?;
        self.save_session(id, &mut session)?;
        Ok(())
    }
}
