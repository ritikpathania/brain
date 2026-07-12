//! Chat state history, message structures, and model roles.

use crate::ui::interaction::markdown::{MarkdownDocument, MessageRevision};

/// Type-safe message identifier wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MessageId(pub u64);

/// Message author category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// End-user input.
    User,
    /// Assistant model response.
    Assistant,
    /// System instruction guidelines.
    System,
}

/// Dynamic generation state of the active session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationState {
    /// Inactive.
    Idle,
    /// Awaiting backend daemon connection.
    Waiting,
    /// Streaming token responses.
    Streaming {
        /// ID of the message being modified.
        message: MessageId,
        /// Highest processed sequence number.
        last_sequence: u64,
    },
    /// Completed token generation.
    Completed {
        /// ID of the finished message.
        message: MessageId,
    },
    /// Stop request issued.
    Cancelling {
        /// ID of the message being cancelled.
        message: MessageId,
    },
    /// Generation failure.
    Error {
        /// ID of the failed message.
        message: MessageId,
    },
}

/// Representation of an individual chat message log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Opaque message identifier.
    pub id: MessageId,
    /// Author classification.
    pub role: MessageRole,
    /// Incremental markdown document content.
    pub text: MarkdownDocument,
    /// Monotonic revision sequence tracker.
    pub revision: MessageRevision,
}

/// An ordered collection representing conversation history.
pub struct ChatState {
    messages: Vec<ChatMessage>,
    next_msg_id: u64,
}

impl ChatState {
    /// Instantiates a new empty ChatState.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            next_msg_id: 1,
        }
    }

    /// Access the ordered message list slice.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Pushes a new message, generating an opaque unique sequential MessageId.
    pub fn push_message(&mut self, role: MessageRole, text_str: String) -> MessageId {
        let id = MessageId(self.next_msg_id);
        self.next_msg_id += 1;
        let mut text = MarkdownDocument::new();
        text.append(&text_str);
        self.messages.push(ChatMessage {
            id,
            role,
            text,
            revision: MessageRevision(0),
        });
        id
    }

    /// Clears conversation history but preserves sequence increments.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Appends a token string chunk to the message content matching id.
    pub fn append_token(&mut self, id: MessageId, text: &str) -> Result<(), &'static str> {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.text.append(text);
            msg.revision.0 += 1;
            Ok(())
        } else {
            Err("Message not found")
        }
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}
