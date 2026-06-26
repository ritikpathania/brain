use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::services::SessionService;
use brain_domain::{Conversation, Node, SessionId};
use brain_session::SessionCacheManager;
use std::sync::Arc;

/// Concrete implementation of SessionService orchestrating persistence and volatile cache.
pub struct SessionServiceImpl {
    repos: Arc<dyn RepositorySet>,
    cache_manager: Arc<SessionCacheManager>,
}

impl SessionServiceImpl {
    /// Creates a new SessionServiceImpl.
    pub fn new(repos: Arc<dyn RepositorySet>, cache_manager: Arc<SessionCacheManager>) -> Self {
        Self {
            repos,
            cache_manager,
        }
    }
}

impl SessionService for SessionServiceImpl {
    fn create_session(&self) -> Result<SessionId, BrainError> {
        let session_id = SessionId::new();
        let conversation = Conversation::new_empty();

        // Populate cache context
        self.cache_manager.get_or_create(session_id);

        // Persist session to database
        self.repos
            .sessions()
            .save_session(&session_id, &conversation)?;

        Ok(session_id)
    }

    fn session_exists(&self, id: &SessionId) -> Result<bool, BrainError> {
        if self.cache_manager.exists(id) {
            return Ok(true);
        }
        let session = self.repos.sessions().load_session(id)?;
        Ok(session.is_some())
    }

    fn load_session(&self, id: &SessionId) -> Result<Conversation, BrainError> {
        let session = self.repos.sessions().load_session(id)?;
        match session {
            Some(conversation) => {
                // Ensure context is initialized in the cache
                self.cache_manager.get_or_create(*id);
                Ok(conversation)
            }
            None => Err(BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            }),
        }
    }

    fn save_session(&self, id: &SessionId, history: &Conversation) -> Result<(), BrainError> {
        if !self.session_exists(id)? {
            return Err(BrainError::Session {
                session_id: *id,
                message: "Session does not exist".to_string(),
            });
        }
        self.repos.sessions().save_session(id, history)?;
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
        self.repos.sessions().delete_session(id)?;
        self.cache_manager.remove(id);
        Ok(())
    }
}
