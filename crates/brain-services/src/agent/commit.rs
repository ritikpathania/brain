use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_domain::SessionId;
use brain_storage::SqliteStorage;

use crate::agent::{MemoryCommit, MemoryCommitService};

/// Concrete implementation of `MemoryCommitService` using `SqliteStorage`.
pub struct MemoryCommitServiceImpl {
    storage: Arc<SqliteStorage>,
}

impl MemoryCommitServiceImpl {
    /// Creates a new `MemoryCommitServiceImpl` wrapping the SQLite storage connection pool.
    pub fn new(storage: Arc<SqliteStorage>) -> Self {
        Self { storage }
    }
}

impl MemoryCommitService for MemoryCommitServiceImpl {
    fn commit(&self, session_id: &SessionId, commit: MemoryCommit) -> Result<(), BrainError> {
        self.storage.run_transaction(|tx| {
            let repos = tx.repositories();

            // 1. Commit graph node entities
            if !commit.nodes.is_empty() {
                repos.nodes().save_batch(&commit.nodes)?;
            }

            // 2. Commit graph edge entities
            if !commit.edges.is_empty() {
                repos.edges().save_batch(&commit.edges)?;
            }

            // 3. Commit conversation messages, appending to the existing session logs
            if !commit.messages.is_empty() {
                let mut conversation = repos
                    .sessions()
                    .load_session(session_id)?
                    .unwrap_or_else(brain_domain::Conversation::new_empty);

                for message in commit.messages {
                    conversation.messages.push(message);
                }

                repos.sessions().save_session(session_id, &conversation)?;
            }

            Ok(())
        })
    }
}
