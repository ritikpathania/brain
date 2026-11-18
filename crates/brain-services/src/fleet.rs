//! Fleet registry and durable preference management service.

use brain_domain::{FleetJobSummary, FleetPreferences, FleetSnapshot, ForegroundSession};
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

/// State managed by the Fleet registry.
#[derive(Debug, Default)]
struct FleetRegistryState {
    /// Active interactive foreground sessions keyed by client_id.
    sessions: BTreeMap<String, ForegroundSession>,
    /// Durable preferences (pins, groups, view mode).
    preferences: FleetPreferences,
}

/// Service managing active foreground sessions and durable preferences.
#[derive(Debug, Clone)]
pub struct FleetRegistry {
    state: Arc<RwLock<FleetRegistryState>>,
    storage_path: Option<PathBuf>,
}

impl FleetRegistry {
    /// Create a new Fleet registry with optional persistence path.
    pub fn new(storage_path: Option<PathBuf>) -> Self {
        let mut preferences = FleetPreferences::default();
        if let Some(ref path) = storage_path {
            if path.exists() {
                if let Ok(data) = fs::read_to_string(path) {
                    if let Ok(loaded) = serde_json::from_str::<FleetPreferences>(&data) {
                        preferences = loaded;
                    }
                }
            }
        }

        Self {
            state: Arc::new(RwLock::new(FleetRegistryState {
                sessions: BTreeMap::new(),
                preferences,
            })),
            storage_path,
        }
    }

    /// Register or update an active foreground session.
    pub fn register_session(&self, session: ForegroundSession) {
        let mut guard = self.state.write();
        guard.sessions.insert(session.client_id.clone(), session);
    }

    /// Unregister a foreground session on disconnect / exit.
    pub fn unregister_session(&self, client_id: &str) -> Option<ForegroundSession> {
        let mut guard = self.state.write();
        guard.sessions.remove(client_id)
    }

    /// Update session status.
    pub fn update_status(&self, client_id: &str, status: String, status_detail: String) {
        let mut guard = self.state.write();
        if let Some(session) = guard.sessions.get_mut(client_id) {
            session.status = status;
            session.status_detail = status_detail;
            session.updated_at_ms = chrono::Utc::now().timestamp_millis() as u64;
        }
    }

    /// Check whether a session ID is currently active in any foreground client.
    pub fn is_active_foreground_session(&self, session_id: &str) -> bool {
        let guard = self.state.read();
        guard.sessions.values().any(|s| s.session_id == session_id)
    }

    /// Retrieve the consolidated Fleet snapshot relative to a caller.
    pub fn get_snapshot(
        &self,
        requester_client_id: Option<&str>,
        background_tasks: Vec<FleetJobSummary>,
    ) -> FleetSnapshot {
        let guard = self.state.read();
        let mut foreground_sessions: Vec<ForegroundSession> =
            guard.sessions.values().cloned().collect();
        // Sort sessions by started_at_ms ascending
        foreground_sessions.sort_by_key(|s| s.started_at_ms);

        let pinned_ids = guard.preferences.pinned_ids.clone();
        let group_tags = guard.preferences.group_tags.clone();
        let known_groups = guard.preferences.known_groups();

        FleetSnapshot {
            foreground_sessions,
            background_tasks,
            pinned_ids,
            group_tags,
            known_groups,
            requester_client_id: requester_client_id.map(|s| s.to_string()),
        }
    }

    /// Pin an item.
    pub fn pin(&self, id: impl Into<String>) {
        let mut guard = self.state.write();
        guard.preferences.pin(id);
        self.persist_locked(&guard.preferences);
    }

    /// Unpin an item.
    pub fn unpin(&self, id: &str) {
        let mut guard = self.state.write();
        guard.preferences.unpin(id);
        self.persist_locked(&guard.preferences);
    }

    /// Reorder pinned items.
    pub fn reorder_pinned(&self, new_order: Vec<String>) {
        let mut guard = self.state.write();
        guard.preferences.reorder_pinned(new_order);
        self.persist_locked(&guard.preferences);
    }

    /// Set group tag.
    pub fn set_group(&self, id: impl Into<String>, group: impl Into<String>) {
        let mut guard = self.state.write();
        guard.preferences.set_group(id, group);
        self.persist_locked(&guard.preferences);
    }

    /// Clear group tag.
    pub fn clear_group(&self, id: &str) {
        let mut guard = self.state.write();
        guard.preferences.clear_group(id);
        self.persist_locked(&guard.preferences);
    }

    /// List known group names.
    pub fn list_groups(&self) -> Vec<String> {
        let guard = self.state.read();
        guard.preferences.known_groups()
    }

    fn persist_locked(&self, prefs: &FleetPreferences) {
        if let Some(ref path) = self.storage_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(prefs) {
                let _ = fs::write(path, json);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_register_and_unregister_foreground_session() {
        let registry = FleetRegistry::new(None);
        let s1 = ForegroundSession::new("client-1", 1234, "sess-1", "/path/a", "repo-a", 1000);
        let s2 = ForegroundSession::new("client-2", 5678, "sess-2", "/path/b", "repo-b", 2000);

        registry.register_session(s1);
        registry.register_session(s2);

        let snap_1 = registry.get_snapshot(Some("client-1"), vec![]);
        assert_eq!(snap_1.foreground_sessions.len(), 2);
        assert_eq!(snap_1.requester_client_id.as_deref(), Some("client-1"));

        // Unregister client-2
        let removed = registry.unregister_session("client-2");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().client_id, "client-2");

        let snap_after = registry.get_snapshot(Some("client-1"), vec![]);
        assert_eq!(snap_after.foreground_sessions.len(), 1);
        assert_eq!(snap_after.foreground_sessions[0].client_id, "client-1");
    }

    #[test]
    fn test_durable_pins_and_reorder() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("fleet_preferences.json");

        {
            let registry = FleetRegistry::new(Some(store_path.clone()));
            registry.pin("task-1");
            registry.pin("task-2");
            registry.pin("task-3");
            // Idempotent duplicate pin
            registry.pin("task-2");

            let snap = registry.get_snapshot(None, vec![]);
            assert_eq!(snap.pinned_ids, vec!["task-1", "task-2", "task-3"]);

            // Reorder
            registry.reorder_pinned(vec![
                "task-3".to_string(),
                "task-1".to_string(),
                "task-2".to_string(),
            ]);
            let snap_reordered = registry.get_snapshot(None, vec![]);
            assert_eq!(
                snap_reordered.pinned_ids,
                vec!["task-3", "task-1", "task-2"]
            );
        }

        // Restart registry loading from disk
        {
            let registry2 = FleetRegistry::new(Some(store_path));
            let snap2 = registry2.get_snapshot(None, vec![]);
            assert_eq!(snap2.pinned_ids, vec!["task-3", "task-1", "task-2"]);

            registry2.unpin("task-1");
            let snap3 = registry2.get_snapshot(None, vec![]);
            assert_eq!(snap3.pinned_ids, vec!["task-3", "task-2"]);
        }
    }

    #[test]
    fn test_durable_group_tags() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("fleet_preferences.json");

        {
            let registry = FleetRegistry::new(Some(store_path.clone()));
            registry.set_group("task-1", "frontend");
            registry.set_group("task-2", "backend");
            registry.set_group("task-3", "frontend");

            assert_eq!(registry.list_groups(), vec!["backend", "frontend"]);
            let snap = registry.get_snapshot(None, vec![]);
            assert_eq!(snap.group_tags.get("task-1").unwrap(), "frontend");
            assert_eq!(snap.group_tags.get("task-2").unwrap(), "backend");
        }

        // Restart
        {
            let registry2 = FleetRegistry::new(Some(store_path));
            assert_eq!(registry2.list_groups(), vec!["backend", "frontend"]);

            registry2.clear_group("task-1");
            let snap2 = registry2.get_snapshot(None, vec![]);
            assert!(!snap2.group_tags.contains_key("task-1"));
            assert_eq!(snap2.group_tags.get("task-2").unwrap(), "backend");
        }
    }

    #[test]
    fn test_is_active_foreground_session() {
        let registry = FleetRegistry::new(None);
        assert!(!registry.is_active_foreground_session("sess-active-123"));

        let session =
            ForegroundSession::new("client-1", 9999, "sess-active-123", "/cwd", "Brain", 1000);
        registry.register_session(session);

        assert!(registry.is_active_foreground_session("sess-active-123"));
        assert!(!registry.is_active_foreground_session("sess-other-456"));

        registry.unregister_session("client-1");
        assert!(!registry.is_active_foreground_session("sess-active-123"));
    }
}
