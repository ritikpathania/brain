use brain_core::errors::BrainError;
use brain_core::services::{RetrievalService, SessionService};
use brain_domain::{Session, SessionTitle, SessionTimestamp, MemoryDTO, Node, NodeDTO, SessionId};
use std::collections::HashMap;
use std::sync::Mutex;

/// Stub implementation of SessionService using an in-memory map.
pub struct StubSessionService {
    sessions: Mutex<HashMap<SessionId, Session>>,
    nodes: Mutex<HashMap<SessionId, Vec<Node>>>,
}

impl StubSessionService {
    /// Creates a new StubSessionService.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            nodes: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for StubSessionService {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionService for StubSessionService {
    fn create_session(&self) -> Result<SessionId, BrainError> {
        let id = SessionId::new();
        let session = Session::new(
            id,
            SessionTitle("New Session".to_string()),
            SessionTimestamp(0),
        );
        self.sessions.lock().unwrap().insert(id, session);
        Ok(id)
    }

    fn session_exists(&self, id: &SessionId) -> Result<bool, BrainError> {
        Ok(self.sessions.lock().unwrap().contains_key(id))
    }

    fn load_session(&self, id: &SessionId) -> Result<Session, BrainError> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            })
    }

    fn save_session(&self, id: &SessionId, session: &mut Session) -> Result<(), BrainError> {
        let mut guard = self.sessions.lock().unwrap();
        if !guard.contains_key(id) {
            return Err(BrainError::Session {
                session_id: *id,
                message: "Session does not exist".to_string(),
            });
        }
        guard.insert(*id, session.clone());
        Ok(())
    }

    fn ingest_node(&self, id: &SessionId, node: Node) -> Result<(), BrainError> {
        if !self.session_exists(id)? {
            return Err(BrainError::Session {
                session_id: *id,
                message: "Session does not exist".to_string(),
            });
        }
        self.nodes
            .lock()
            .unwrap()
            .entry(*id)
            .or_default()
            .push(node);
        Ok(())
    }

    fn delete_session(&self, id: &SessionId) -> Result<(), BrainError> {
        self.sessions.lock().unwrap().remove(id);
        self.nodes.lock().unwrap().remove(id);
        Ok(())
    }

    fn rename_session(&self, id: &SessionId, title: &str) -> Result<(), BrainError> {
        let mut guard = self.sessions.lock().unwrap();
        if let Some(s) = guard.get_mut(id) {
            s.rename(SessionTitle(title.to_string()), SessionTimestamp(0));
            Ok(())
        } else {
            Err(BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            })
        }
    }

    fn set_session_pinned(&self, id: &SessionId, pinned: bool) -> Result<(), BrainError> {
        let mut guard = self.sessions.lock().unwrap();
        if let Some(s) = guard.get_mut(id) {
            s.set_pinned(pinned, SessionTimestamp(0));
            Ok(())
        } else {
            Err(BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            })
        }
    }

    fn archive_session(&self, id: &SessionId) -> Result<(), BrainError> {
        let mut guard = self.sessions.lock().unwrap();
        if let Some(s) = guard.get_mut(id) {
            s.archive(SessionTimestamp(0))?;
            Ok(())
        } else {
            Err(BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            })
        }
    }

    fn restore_session(&self, id: &SessionId) -> Result<(), BrainError> {
        let mut guard = self.sessions.lock().unwrap();
        if let Some(s) = guard.get_mut(id) {
            s.restore(SessionTimestamp(0))?;
            Ok(())
        } else {
            Err(BrainError::Session {
                session_id: *id,
                message: "Session not found".to_string(),
            })
        }
    }
}

/// Stub implementation of RetrievalService returning mock memory matches.
pub struct StubRetrievalService {
    mock_results: Mutex<Vec<MemoryDTO>>,
}

impl StubRetrievalService {
    /// Creates a new StubRetrievalService.
    pub fn new() -> Self {
        Self {
            mock_results: Mutex::new(Vec::new()),
        }
    }

    /// Sets mock results returned by retrieve().
    pub fn set_results(&self, results: Vec<MemoryDTO>) {
        let mut guard = self.mock_results.lock().unwrap();
        *guard = results;
    }
}

impl Default for StubRetrievalService {
    fn default() -> Self {
        Self::new()
    }
}

impl RetrievalService for StubRetrievalService {
    fn retrieve(
        &self,
        _session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryDTO>, BrainError> {
        let guard = self.mock_results.lock().unwrap();
        let matched: Vec<MemoryDTO> = guard
            .iter()
            .filter(|r| r.node.label.to_lowercase().contains(&query.to_lowercase()))
            .cloned()
            .collect();

        let mut results = if matched.is_empty() {
            let node_dto = NodeDTO::new(
                brain_domain::NodeId::new().to_string(),
                format!("Stub result for '{}'", query),
                "Concept".to_string(),
                serde_json::json!({}),
            );
            vec![MemoryDTO::new(node_dto, Vec::new(), Vec::new())]
        } else {
            matched
        };

        results.truncate(limit);
        Ok(results)
    }
}

/// Stub implementation of DomainEventPublisher that collects published events.
pub struct StubDomainEventPublisher {
    events: Mutex<Vec<brain_domain::DomainEvent>>,
}

impl StubDomainEventPublisher {
    /// Creates a new StubDomainEventPublisher.
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    /// Clears all collected events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Returns a clone of all published events.
    pub fn get_events(&self) -> Vec<brain_domain::DomainEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl Default for StubDomainEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::jobs::publisher::DomainEventPublisher for StubDomainEventPublisher {
    fn publish(&self, event: brain_domain::DomainEvent) {
        self.events.lock().unwrap().push(event);
    }
}
