use brain_core::errors::BrainError;
use brain_core::services::{RetrievalService, SessionService};
use brain_domain::{Conversation, MemoryDTO, Node, NodeDTO, SessionId};
use std::collections::HashMap;
use std::sync::Mutex;

/// Stub implementation of SessionService using an in-memory map.
pub struct StubSessionService {
    sessions: Mutex<HashMap<SessionId, Conversation>>,
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
        let conversation = Conversation::new_empty();
        let id = SessionId::new();
        self.sessions.lock().unwrap().insert(id, conversation);
        Ok(id)
    }

    fn session_exists(&self, id: &SessionId) -> Result<bool, BrainError> {
        Ok(self.sessions.lock().unwrap().contains_key(id))
    }

    fn load_session(&self, id: &SessionId) -> Result<Conversation, BrainError> {
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

    fn save_session(&self, id: &SessionId, history: &Conversation) -> Result<(), BrainError> {
        let mut guard = self.sessions.lock().unwrap();
        if !guard.contains_key(id) {
            return Err(BrainError::Session {
                session_id: *id,
                message: "Session does not exist".to_string(),
            });
        }
        guard.insert(*id, history.clone());
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
