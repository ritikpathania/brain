use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_domain::SessionId;
use brain_storage::SqliteStorage;

use crate::agent::{MemoryCommit, MemoryCommitService};
use crate::jobs::publisher::DomainEventPublisher;

/// Concrete implementation of `MemoryCommitService` using `SqliteStorage`.
pub struct MemoryCommitServiceImpl {
    storage: Arc<SqliteStorage>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl MemoryCommitServiceImpl {
    /// Creates a new `MemoryCommitServiceImpl` wrapping the SQLite storage connection pool.
    pub fn new(storage: Arc<SqliteStorage>, publisher: Arc<dyn DomainEventPublisher>) -> Self {
        Self { storage, publisher }
    }
}

impl MemoryCommitService for MemoryCommitServiceImpl {
    fn commit(&self, session_id: &SessionId, commit: MemoryCommit) -> Result<(), BrainError> {
        let events = self.storage.run_transaction(|tx| {
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
            let mut events = Vec::new();
            if !commit.messages.is_empty() {
                let mut conversation = repos
                    .sessions()
                    .load_session(session_id)?
                    .unwrap_or_else(brain_domain::Session::new_empty);

                for message in commit.messages {
                    conversation.add_message(message)?;
                }

                repos.sessions().save_session(session_id, &conversation)?;
                events = conversation.drain_events().collect();
            }

            Ok(events)
        })?;

        for event in events {
            self.publisher.publish(event);
        }

        Ok(())
    }
}
