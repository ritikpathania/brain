//! Domain models for multi-process foreground session registry and durable Fleet preferences.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Registered active foreground interactive TUI session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForegroundSession {
    /// Unique client instance UUID.
    pub client_id: String,
    /// Operating system process identifier.
    pub pid: u32,
    /// Authoritative Brain session identifier.
    pub session_id: String,
    /// Working directory for workspace scoping.
    pub cwd: String,
    /// Display name or repo name.
    pub display_name: String,
    /// Timestamp when process started (milliseconds since epoch).
    pub started_at_ms: u64,
    /// High-level session lifecycle status.
    pub status: String,
    /// Descriptive status text (e.g. active tool or pending query).
    pub status_detail: String,
    /// Registered frontend capabilities.
    pub capabilities: Vec<String>,
    /// Frontend application version.
    pub version: String,
    /// Last heartbeat / activity timestamp.
    pub updated_at_ms: u64,
}

impl ForegroundSession {
    /// Create a new registered foreground session.
    pub fn new(
        client_id: impl Into<String>,
        pid: u32,
        session_id: impl Into<String>,
        cwd: impl Into<String>,
        display_name: impl Into<String>,
        started_at_ms: u64,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            pid,
            session_id: session_id.into(),
            cwd: cwd.into(),
            display_name: display_name.into(),
            started_at_ms,
            status: "idle".to_string(),
            status_detail: String::new(),
            capabilities: Vec::new(),
            version: "1.0.0".to_string(),
            updated_at_ms: started_at_ms,
        }
    }
}

/// Lightweight summary of a background task for Fleet rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetJobSummary {
    /// Unique task/job identifier.
    pub id: String,
    /// Task kind/template (e.g. "bg", "research", "agent").
    pub kind: String,
    /// Display name / label.
    pub label: String,
    /// Title / workspace directory name.
    pub title: String,
    /// Subtext description or active query.
    pub subtext: String,
    /// Execution status ("pending", "running", "waiting", "completed", "failed", "cancelled", "idle").
    pub status: String,
    /// Start timestamp (milliseconds since epoch).
    pub started_at_ms: u64,
    /// Workspace working directory.
    pub cwd: String,
    /// Group tag assigned.
    pub group: Option<String>,
    /// Whether pinned.
    pub pinned: bool,
}

/// Durable user preferences for Fleet view (pins, groups, view mode).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FleetPreferences {
    /// Ordered list of pinned task/session identifiers.
    pub pinned_ids: Vec<String>,
    /// Group tag assignments (task/session ID -> group name).
    pub group_tags: BTreeMap<String, String>,
    /// Preferred default view mode ("state", "directory", "group").
    pub view_mode: Option<String>,
}

impl FleetPreferences {
    /// Pin a task/session by ID.
    pub fn pin(&mut self, id: impl Into<String>) {
        let id_str = id.into();
        if !self.pinned_ids.contains(&id_str) {
            self.pinned_ids.push(id_str);
        }
    }

    /// Unpin a task/session by ID.
    pub fn unpin(&mut self, id: &str) {
        self.pinned_ids.retain(|p| p != id);
    }

    /// Reorder pinned identifiers.
    pub fn reorder_pinned(&mut self, new_order: Vec<String>) {
        // Keep valid existing pinned ids in the requested order, appending any missing ones
        let mut reordered = Vec::new();
        for id in new_order {
            if self.pinned_ids.contains(&id) && !reordered.contains(&id) {
                reordered.push(id);
            }
        }
        for id in &self.pinned_ids {
            if !reordered.contains(id) {
                reordered.push(id.clone());
            }
        }
        self.pinned_ids = reordered;
    }

    /// Set group tag for a task/session.
    pub fn set_group(&mut self, id: impl Into<String>, group: impl Into<String>) {
        let g = group.into().trim().to_string();
        if g.is_empty() {
            self.clear_group(&id.into());
        } else {
            self.group_tags.insert(id.into(), g);
        }
    }

    /// Clear group tag for a task/session.
    pub fn clear_group(&mut self, id: &str) {
        self.group_tags.remove(id);
    }

    /// Return unique known groups sorted alphabetically.
    pub fn known_groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = self
            .group_tags
            .values()
            .filter(|g| !g.is_empty())
            .cloned()
            .collect();
        groups.sort();
        groups.dedup();
        groups
    }
}

/// Unified Fleet snapshot projection tailored for a requesting client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetSnapshot {
    /// Active interactive foreground sessions.
    pub foreground_sessions: Vec<ForegroundSession>,
    /// Background tasks / jobs.
    pub background_tasks: Vec<FleetJobSummary>,
    /// Authoritative ordered list of pinned IDs.
    pub pinned_ids: Vec<String>,
    /// Group tag assignments.
    pub group_tags: BTreeMap<String, String>,
    /// List of unique known group names for autocomplete.
    pub known_groups: Vec<String>,
    /// Client ID of the requesting terminal (to resolve "current session" relative to caller).
    pub requester_client_id: Option<String>,
}
