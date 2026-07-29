//! Immutable Copy-on-Write Versioned Configuration Engine (Phase G.5).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Monotonic configuration version identifier wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConfigVersion(pub u64);

impl ConfigVersion {
    /// Returns initial version V1.
    pub fn v1() -> Self {
        Self(1)
    }

    /// Allocates next version index.
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

/// Immutable, versioned configuration snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeConfiguration {
    /// Version identifier.
    pub version: ConfigVersion,
    /// Creation timestamp in milliseconds.
    pub created_at_ms: u64,
    /// Optional parent version identifier.
    pub parent_version: Option<ConfigVersion>,
    /// Task retry limit.
    pub max_task_retries: u32,
    /// Task execution timeout in milliseconds.
    pub task_timeout_ms: u64,
    /// Maximum events between snapshots.
    pub snapshot_max_events: usize,
    /// Retention TTL in milliseconds.
    pub retention_ttl_ms: u64,
    /// Compaction event threshold.
    pub compaction_threshold: usize,
    /// Dynamic key-value configuration overrides.
    pub settings: HashMap<String, String>,
}

impl RuntimeConfiguration {
    /// Creates a default V1 configuration.
    pub fn default_v1() -> Self {
        let mut settings = HashMap::new();
        settings.insert("execution.mode".to_string(), "deterministic".to_string());
        settings.insert("logging.level".to_string(), "info".to_string());

        Self {
            version: ConfigVersion::v1(),
            created_at_ms: 1000,
            parent_version: None,
            max_task_retries: 3,
            task_timeout_ms: 30000,
            snapshot_max_events: 100,
            retention_ttl_ms: 86400000, // 24h
            compaction_threshold: 1000,
            settings,
        }
    }
}

/// Difference record between two configuration versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigurationDiff {
    /// Version before update.
    pub from_version: ConfigVersion,
    /// Version after update.
    pub to_version: ConfigVersion,
    /// List of changed parameter keys and (old_value, new_value).
    pub changes: Vec<(String, String, String)>,
}

/// Copy-on-Write thread-safe Configuration Manager.
pub struct ConfigurationManager {
    versions: RwLock<HashMap<ConfigVersion, Arc<RuntimeConfiguration>>>,
    active_version: RwLock<ConfigVersion>,
}

impl Default for ConfigurationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationManager {
    /// Initializes `ConfigurationManager` with default V1 configuration.
    pub fn new() -> Self {
        let v1 = Arc::new(RuntimeConfiguration::default_v1());
        let mut versions = HashMap::new();
        versions.insert(ConfigVersion::v1(), v1);

        Self {
            versions: RwLock::new(versions),
            active_version: RwLock::new(ConfigVersion::v1()),
        }
    }

    /// Retrieves a specific configuration version.
    pub fn get_version(&self, version: ConfigVersion) -> Option<Arc<RuntimeConfiguration>> {
        let versions = self.versions.read().unwrap();
        versions.get(&version).cloned()
    }

    /// Retrieves the currently active configuration version.
    pub fn active_configuration(&self) -> Arc<RuntimeConfiguration> {
        let active_ver = *self.active_version.read().unwrap();
        self.get_version(active_ver)
            .expect("Active configuration missing")
    }

    /// Creates a Copy-on-Write draft based on a parent version.
    pub fn create_draft(
        &self,
        parent_version: ConfigVersion,
    ) -> Result<RuntimeConfiguration, String> {
        let parent = self
            .get_version(parent_version)
            .ok_or_else(|| format!("Parent version {:?} not found", parent_version))?;

        let versions = self.versions.read().unwrap();
        let next_ver = ConfigVersion(versions.keys().map(|v| v.0).max().unwrap_or(0) + 1);

        let mut draft = (*parent).clone();
        draft.version = next_ver;
        draft.parent_version = Some(parent_version);
        draft.created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(draft)
    }

    /// Validates a draft configuration.
    pub fn validate(&self, draft: &RuntimeConfiguration) -> Result<(), String> {
        if draft.max_task_retries == 0 {
            return Err("max_task_retries must be > 0".to_string());
        }
        if draft.task_timeout_ms < 100 {
            return Err("task_timeout_ms must be >= 100ms".to_string());
        }
        Ok(())
    }

    /// Activates a new version after validation.
    pub fn activate(&self, new_config: RuntimeConfiguration) -> Result<ConfigVersion, String> {
        self.validate(&new_config)?;

        let ver = new_config.version;
        let mut versions = self.versions.write().unwrap();
        if versions.contains_key(&ver) {
            return Err(format!(
                "Version {:?} already exists and cannot be overwritten",
                ver
            ));
        }

        versions.insert(ver, Arc::new(new_config));
        let mut active = self.active_version.write().unwrap();
        *active = ver;

        Ok(ver)
    }

    /// Performs rollback to a previous version by allocating a new CoW version with past settings.
    pub fn rollback(&self, target_version: ConfigVersion) -> Result<ConfigVersion, String> {
        let target = self
            .get_version(target_version)
            .ok_or_else(|| format!("Target rollback version {:?} not found", target_version))?;

        let active_ver = *self.active_version.read().unwrap();
        let mut draft = self.create_draft(active_ver)?;

        draft.max_task_retries = target.max_task_retries;
        draft.task_timeout_ms = target.task_timeout_ms;
        draft.snapshot_max_events = target.snapshot_max_events;
        draft.retention_ttl_ms = target.retention_ttl_ms;
        draft.compaction_threshold = target.compaction_threshold;
        draft.settings = target.settings.clone();

        self.activate(draft)
    }

    /// Computes deterministic diff between two versions.
    pub fn compute_diff(
        &self,
        from_ver: ConfigVersion,
        to_ver: ConfigVersion,
    ) -> Result<ConfigurationDiff, String> {
        let from = self
            .get_version(from_ver)
            .ok_or_else(|| format!("Version {:?} not found", from_ver))?;
        let to = self
            .get_version(to_ver)
            .ok_or_else(|| format!("Version {:?} not found", to_ver))?;

        let mut changes = Vec::new();

        if from.max_task_retries != to.max_task_retries {
            changes.push((
                "max_task_retries".to_string(),
                from.max_task_retries.to_string(),
                to.max_task_retries.to_string(),
            ));
        }
        if from.task_timeout_ms != to.task_timeout_ms {
            changes.push((
                "task_timeout_ms".to_string(),
                from.task_timeout_ms.to_string(),
                to.task_timeout_ms.to_string(),
            ));
        }
        if from.snapshot_max_events != to.snapshot_max_events {
            changes.push((
                "snapshot_max_events".to_string(),
                from.snapshot_max_events.to_string(),
                to.snapshot_max_events.to_string(),
            ));
        }

        Ok(ConfigurationDiff {
            from_version: from_ver,
            to_version: to_ver,
            changes,
        })
    }
}
