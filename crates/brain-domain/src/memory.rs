//! Domain models for persistent relational memory and stewardship.

/// Filter criteria for querying memory stewardship collections.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum MemoryFilter {
    /// All active, pinned, and consolidated memories (default).
    #[default]
    All,
    /// Explicitly pinned context memories.
    Pinned,
    /// Active runtime context observations.
    Active,
    /// Archived or cold-storage memories.
    Archived,
}

impl MemoryFilter {
    /// Returns the slash command filter argument string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Pinned => "pinned",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    /// Parses a filter string into a `MemoryFilter`.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "pinned" | "/memory pinned" => Self::Pinned,
            "active" | "/memory active" => Self::Active,
            "archived" | "/memory archived" => Self::Archived,
            _ => Self::All,
        }
    }
}

/// Explicit domain lifecycle state of a memory entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MemoryState {
    /// Active memory available for retrieval and context.
    Active,
    /// Explicitly pinned memory locked in runtime context.
    Pinned,
    /// Retained memory moved to cold storage.
    Archived,
    /// Stale or decayed memory past TTL threshold.
    Expired,
}

impl MemoryState {
    /// Returns user-facing badge text.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Pinned => "Pinned",
            Self::Archived => "Archived",
            Self::Expired => "Expired",
        }
    }
}

/// Category classification for remembered knowledge items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MemoryCategory {
    /// Explicitly pinned context node.
    PinnedContext,
    /// Active runtime context observation.
    ActiveRuntime,
    /// Consolidated long-term memory entity.
    ConsolidatedMemory,
}

impl MemoryCategory {
    /// Returns user-facing badge label text.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::PinnedContext => "Pinned Context",
            Self::ActiveRuntime => "Runtime Context",
            Self::ConsolidatedMemory => "Consolidated Memory",
        }
    }
}

/// Unified summary item describing a memory record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemorySummary {
    /// Unique memory / entity identifier.
    pub id: String,
    /// Display label / entity title.
    pub display_name: String,
    /// Category classification.
    pub category: MemoryCategory,
    /// Explicit lifecycle state.
    pub state: MemoryState,
    /// Short content preview snippet.
    pub snippet: String,
    /// Originating subsystem / source kind.
    pub source_kind: String,
}

/// Unified command mutation type for memory stewardship operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MemoryMutation {
    /// Pin memory into active context.
    Pin,
    /// Unpin memory from active context.
    Unpin,
    /// Archive memory into cold storage.
    Archive,
    /// Restore memory back to active stewardship.
    Restore,
}

impl MemoryMutation {
    /// Returns action name for UDS wire protocol.
    pub fn action_name(&self) -> &'static str {
        match self {
            Self::Pin => "v1/pin_memory",
            Self::Unpin => "v1/unpin_memory",
            Self::Archive => "v1/archive_memory",
            Self::Restore => "v1/restore_memory",
        }
    }
}

/// Lifecycle tracking state for an in-flight memory mutation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum PendingMutationState {
    /// Optimistically applied locally, waiting for daemon confirmation.
    #[default]
    Optimistic,
    /// In-flight async transport request active.
    Pending,
    /// Confirmed by daemon.
    Confirmed,
    /// Mutation failed and rolled back.
    Failed,
}
