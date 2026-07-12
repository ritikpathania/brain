use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Every knowledge object must know where it originated.
/// This enum represents the different sources of knowledge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "source_type", rename_all = "lowercase")]
pub enum Provenance {
    /// Originated from a markdown file.
    MarkdownFile {
        /// File path.
        path: String,
        /// Repository URL or name if applicable.
        repository: Option<String>,
        /// Commit hash.
        commit: Option<String>,
        /// Author of the change.
        author: Option<String>,
        /// Timestamp of document modification.
        timestamp: u64,
    },
    /// Originated from a Slack message.
    Slack {
        /// Workspace ID/domain.
        workspace: String,
        /// Channel ID/name.
        channel: String,
        /// Message unique ID.
        message_id: String,
        /// Author username or user ID.
        author: String,
        /// Message post timestamp.
        timestamp: u64,
    },
    /// Originated from an email.
    Email {
        /// Message ID from email headers.
        message_id: String,
        /// Sender email address.
        from: String,
        /// Recipient email addresses.
        to: Vec<String>,
        /// Email subject.
        subject: String,
        /// Received timestamp.
        timestamp: u64,
    },
    /// Originated from git history.
    Git {
        /// Repository identifier.
        repository: String,
        /// Commit hash.
        commit: String,
        /// Commit author.
        author: String,
        /// Commit timestamp.
        timestamp: u64,
    },
    /// Originated from local filesystem.
    Filesystem {
        /// Relative file path.
        path: String,
        /// Absolute path.
        absolute_path: String,
        /// Creation timestamp if available.
        created_at: Option<u64>,
        /// Last modification timestamp.
        modified_at: Option<u64>,
    },
    /// Originated from Notion database or page.
    Notion {
        /// Notion page ID.
        page_id: String,
        /// Notion workspace name.
        workspace: String,
        /// Page creator or updater.
        author: String,
        /// Last edit timestamp.
        timestamp: u64,
    },
    /// Originated from a web page.
    Web {
        /// Full HTTP URL.
        url: String,
        /// Page HTML title.
        title: Option<String>,
        /// Scraping timestamp.
        timestamp: u64,
    },
    /// Custom, extensible fallback provenance.
    Custom {
        /// Identity of the provider.
        provider: String,
        /// Arbitrary extra parameters.
        details: HashMap<String, serde_json::Value>,
    },
}
