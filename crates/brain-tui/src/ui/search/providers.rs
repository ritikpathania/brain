//! Pluggable search providers for the global search omnibox.

use crate::client::Confidence;
use crate::ui::search::types::{
    ProviderId, SearchContext, SearchEvent, SearchEventSink, SearchProvider, SearchQuery,
    SearchResult, SearchResultAction, SearchResultKind, PROVIDER_COMMANDS, PROVIDER_LOCAL_MESSAGES,
    PROVIDER_REMOTE_MESSAGES, PROVIDER_SESSIONS,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Pluggable provider for static system command workflows.
pub struct CommandsProvider;

impl SearchProvider for CommandsProvider {
    fn provider_id(&self) -> ProviderId {
        PROVIDER_COMMANDS
    }

    fn search(
        &self,
        query: &SearchQuery,
        _context: &SearchContext,
        _cancellation_token: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    ) {
        sink.submit(SearchEvent::Started {
            generation: query.generation,
            provider: self.provider_id(),
        });

        let term = query.text.trim().to_lowercase();
        let mut matches = Vec::new();

        for cmd in crate::ui::command::COMMANDS {
            if cmd.visibility != crate::ui::command::CommandVisibility::SlashOnly
                && (term.is_empty()
                    || cmd.title.to_lowercase().contains(&term)
                    || cmd
                        .aliases
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&term))
                    || cmd
                        .keywords
                        .iter()
                        .any(|kw| kw.to_lowercase().contains(&term)))
            {
                matches.push(SearchResult {
                    entity_id: String::new(), // Commands have no knowledge-graph ID
                    title: Some(cmd.title.to_string()),
                    subtitle: Some(cmd.description.to_string()),
                    kind: SearchResultKind::Command,
                    provider_score: 1,
                    confidence: Confidence::Medium,
                    action: SearchResultAction::InvokeCommand(cmd.id),
                });
            }
        }

        sink.submit(SearchEvent::Results {
            generation: query.generation,
            provider: self.provider_id(),
            results: matches,
        });

        sink.submit(SearchEvent::Finished {
            generation: query.generation,
            provider: self.provider_id(),
        });
    }
}

/// Pluggable provider for context sessions.
pub struct SessionsProvider;

impl SearchProvider for SessionsProvider {
    fn provider_id(&self) -> ProviderId {
        PROVIDER_SESSIONS
    }

    fn search(
        &self,
        query: &SearchQuery,
        context: &SearchContext,
        _cancellation_token: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    ) {
        sink.submit(SearchEvent::Started {
            generation: query.generation,
            provider: self.provider_id(),
        });

        let term = query.text.trim().to_lowercase();
        let mut matches = Vec::new();

        for session in &context.sessions {
            if term.is_empty() || session.title.to_lowercase().contains(&term) {
                matches.push(SearchResult {
                    entity_id: session.id.to_string(),
                    title: Some(session.title.clone()),
                    subtitle: Some(if session.archived {
                        "Archived Session".to_string()
                    } else {
                        "Active Session".to_string()
                    }),
                    kind: SearchResultKind::Session,
                    provider_score: 1,
                    confidence: Confidence::Medium,
                    action: SearchResultAction::SwitchSession(session.id),
                });
            }
        }

        sink.submit(SearchEvent::Results {
            generation: query.generation,
            provider: self.provider_id(),
            results: matches,
        });

        sink.submit(SearchEvent::Finished {
            generation: query.generation,
            provider: self.provider_id(),
        });
    }
}

/// Pluggable provider for loaded local active-session messages cache.
pub struct LocalMessagesProvider;

impl SearchProvider for LocalMessagesProvider {
    fn provider_id(&self) -> ProviderId {
        PROVIDER_LOCAL_MESSAGES
    }

    fn search(
        &self,
        query: &SearchQuery,
        context: &SearchContext,
        _cancellation_token: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    ) {
        sink.submit(SearchEvent::Started {
            generation: query.generation,
            provider: self.provider_id(),
        });

        let term = query.text.trim().to_lowercase();
        let mut matches = Vec::new();

        for (idx, msg) in context.active_messages.iter().enumerate() {
            if term.is_empty() || msg.content.to_lowercase().contains(&term) {
                let preview = if msg.content.len() > 50 {
                    format!("{}...", &msg.content[..50])
                } else {
                    msg.content.clone()
                };

                let role_name = match msg.role {
                    brain_domain::MessageRole::User => "User",
                    brain_domain::MessageRole::Assistant => "Assistant",
                    brain_domain::MessageRole::System => "System",
                };

                matches.push(SearchResult {
                    entity_id: String::new(), // In-session messages have no graph ID
                    title: Some(preview),
                    subtitle: Some(format!("{} message", role_name)),
                    kind: SearchResultKind::Message,
                    provider_score: 1,
                    confidence: Confidence::Medium,
                    action: SearchResultAction::JumpToMessage {
                        message_id: crate::ui::interaction::MessageId(idx as u64),
                    },
                });
            }
        }

        sink.submit(SearchEvent::Results {
            generation: query.generation,
            provider: self.provider_id(),
            results: matches,
        });

        sink.submit(SearchEvent::Finished {
            generation: query.generation,
            provider: self.provider_id(),
        });
    }
}

/// Pluggable provider for daemon-backed knowledge graph search.
///
/// Calls `ExecutionClient::search_candidates` which hits the daemon's `v1/search` endpoint.
/// Retrieval, ranking, and confidence are owned by the daemon — this provider is transport only.
pub struct RemoteMessagesProvider {
    client: Arc<dyn crate::client::ExecutionClient>,
}

impl RemoteMessagesProvider {
    /// Instantiates a new RemoteMessagesProvider.
    pub fn new(client: Arc<dyn crate::client::ExecutionClient>) -> Self {
        Self { client }
    }
}

impl SearchProvider for RemoteMessagesProvider {
    fn provider_id(&self) -> ProviderId {
        PROVIDER_REMOTE_MESSAGES
    }

    fn search(
        &self,
        query: &SearchQuery,
        _context: &SearchContext,
        cancellation_token: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    ) {
        sink.submit(SearchEvent::Started {
            generation: query.generation,
            provider: self.provider_id(),
        });

        let query_text = query.text.clone();
        let gen = query.generation;
        let client_clone = self.client.clone();
        let sink_clone = sink.clone();
        let provider_id = self.provider_id();

        tokio::spawn(async move {
            if cancellation_token.is_cancelled() {
                return;
            }

            match client_clone.search_candidates(&query_text).await {
                Ok(candidates) => {
                    if cancellation_token.is_cancelled() {
                        return;
                    }

                    let results = candidates
                        .into_iter()
                        .map(|c| SearchResult {
                            entity_id: c.entity_id,
                            // Preserve Option — do not resolve to a string here.
                            // MemoryResultViewModel is the sole place that converts None
                            // to a displayable placeholder.
                            title: c.title,
                            subtitle: c.summary,
                            kind: SearchResultKind::Knowledge, // Not Message — knowledge graph entities
                            provider_score: (c.score * 100.0) as i32,
                            confidence: c.confidence,
                            action: SearchResultAction::JumpToMessage {
                                message_id: crate::ui::interaction::MessageId(0),
                            },
                        })
                        .collect();

                    sink_clone.submit(SearchEvent::Results {
                        generation: gen,
                        provider: provider_id,
                        results,
                    });
                    sink_clone.submit(SearchEvent::Finished {
                        generation: gen,
                        provider: provider_id,
                    });
                }
                Err(brain_core::errors::BrainError::Network { .. }) => {
                    // Connection-level failure: daemon is unreachable.
                    // "Daemon unavailable" and "No results found" are distinct.
                    // Never silently swallow this into an empty result set.
                    sink_clone.submit(SearchEvent::Failed {
                        generation: gen,
                        provider: provider_id,
                        reason: crate::ui::search::types::SearchFailure::BackendUnavailable,
                    });
                }
                Err(_e) => {
                    // Other errors (parse, storage, internal): flag as internal failure.
                    if cancellation_token.is_cancelled() {
                        return;
                    }
                    sink_clone.submit(SearchEvent::Failed {
                        generation: gen,
                        provider: provider_id,
                        reason: crate::ui::search::types::SearchFailure::Internal,
                    });
                }
            }
        });
    }
}

