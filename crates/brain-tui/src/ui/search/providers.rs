//! Pluggable search providers for the global search omnibox.

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
            if cmd.visibility != crate::ui::command::CommandVisibility::SlashOnly {
                if term.is_empty()
                    || cmd.title.to_lowercase().contains(&term)
                    || cmd
                        .aliases
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&term))
                    || cmd
                        .keywords
                        .iter()
                        .any(|kw| kw.to_lowercase().contains(&term))
                {
                    matches.push(SearchResult {
                        title: cmd.title.to_string(),
                        subtitle: cmd.description.to_string(),
                        kind: SearchResultKind::Command,
                        provider_score: 1,
                        action: SearchResultAction::InvokeCommand(cmd.id),
                    });
                }
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
                    title: session.title.clone(),
                    subtitle: if session.archived {
                        "Archived Session".to_string()
                    } else {
                        "Active Session".to_string()
                    },
                    kind: SearchResultKind::Session,
                    provider_score: 1,
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
                    title: preview,
                    subtitle: format!("{} message", role_name),
                    kind: SearchResultKind::Message,
                    provider_score: 1,
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

/// Pluggable provider for remote message history.
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

            match client_clone.search_messages(&query_text).await {
                Ok(messages) => {
                    if cancellation_token.is_cancelled() {
                        return;
                    }
                    let mut results = Vec::new();
                    for msg in messages {
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

                        results.push(SearchResult {
                            title: preview,
                            subtitle: format!("{} message (history)", role_name),
                            kind: SearchResultKind::Message,
                            provider_score: 1,
                            action: SearchResultAction::JumpToMessage {
                                message_id: crate::ui::interaction::MessageId(0), // Opaque jump target for historical messages
                            },
                        });
                    }

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
                Err(_e) => {
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
