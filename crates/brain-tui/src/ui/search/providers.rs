//! Pluggable search providers for the global search omnibox.

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use crate::ui::search::types::{
    ProviderId, SearchQuery, SearchContext, SearchEventSink, SearchEvent,
    SearchResult, SearchResultKind, SearchResultAction,
    PROVIDER_COMMANDS, PROVIDER_SESSIONS, PROVIDER_LOCAL_MESSAGES, SearchProvider
};

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
                    || cmd.aliases.iter().any(|alias| alias.to_lowercase().contains(&term))
                    || cmd.keywords.iter().any(|kw| kw.to_lowercase().contains(&term))
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
