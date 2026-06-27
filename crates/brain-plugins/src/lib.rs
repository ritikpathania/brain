use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;

use brain_core::errors::BrainError;
use brain_core::extensibility::{ApiVersion, Plugin, PluginManifest, PluginEvent};
use brain_domain::{PluginId, PluginState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LoaderKind {
    Python,
    Wasm,
    Native,
}

#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub loader_kind: LoaderKind,
}

pub trait PluginLoader: Send + Sync {
    fn kind(&self) -> LoaderKind;
    fn supports(&self, path: &Path) -> bool;
    fn load(&self, descriptor: &InstalledPlugin) -> Result<Box<dyn Plugin>, BrainError>;
}

#[derive(Clone)]
pub struct PluginHandle {
    pub inner: Arc<RwLock<Box<dyn Plugin>>>,
}

pub struct ManagedPlugin {
    pub installed: InstalledPlugin,
    pub active_instance: Option<PluginHandle>,
}

#[derive(Debug, Clone)]
pub struct PluginSummary {
    pub id: PluginId,
    pub version: semver::Version,
    pub api_version: ApiVersion,
    pub state: PluginState,
    pub path: PathBuf,
    pub loader_kind: LoaderKind,
}

pub struct PluginRegistry {
    pub plugins: BTreeMap<PluginId, ManagedPlugin>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: BTreeMap::new() }
    }

    pub fn register(&mut self, installed: InstalledPlugin) -> Result<(), BrainError> {
        let id = installed.manifest.id();
        if self.plugins.contains_key(&id) {
            return Err(BrainError::InvalidTransition {
                message: format!("Plugin ID {} is already registered", id),
            });
        }
        self.plugins.insert(id, ManagedPlugin {
            installed,
            active_instance: None,
        });
        Ok(())
    }

    pub fn get(&self, id: &PluginId) -> Option<&ManagedPlugin> {
        self.plugins.get(id)
    }

    pub fn get_mut(&mut self, id: &PluginId) -> Option<&mut ManagedPlugin> {
        self.plugins.get_mut(id)
    }

    pub fn list(&self) -> Vec<PluginSummary> {
        self.plugins.values().map(|mp| {
            let state = mp.active_instance.as_ref()
                .map(|p| p.inner.read().state())
                .unwrap_or(PluginState::Discovered);
            PluginSummary {
                id: mp.installed.manifest.id(),
                version: mp.installed.manifest.version().clone(),
                api_version: mp.installed.manifest.api_version(),
                state,
                path: mp.installed.path.clone(),
                loader_kind: mp.installed.loader_kind,
            }
        }).collect()
    }
}

pub struct PluginScanner {
    loaders: Vec<Box<dyn PluginLoader>>,
}

impl PluginScanner {
    pub fn new(loaders: Vec<Box<dyn PluginLoader>>) -> Self {
        Self { loaders }
    }

    pub fn scan_directory(&self, plugins_dir: &Path) -> Result<Vec<InstalledPlugin>, BrainError> {
        let mut discovered = Vec::new();
        let read_dir = std::fs::read_dir(plugins_dir).map_err(|e| BrainError::Storage {
            message: format!("Failed to read plugins directory: {}", e),
            source: Some(Box::new(e)),
        })?;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                for loader in &self.loaders {
                    if loader.supports(&path) {
                        let manifest_path = path.join("plugin.toml");
                        if manifest_path.exists() {
                            if let Ok(manifest) = PluginManifest::from_path(&manifest_path) {
                                discovered.push(InstalledPlugin {
                                    manifest,
                                    path: path.clone(),
                                    loader_kind: loader.kind(),
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(discovered)
    }
}

#[derive(Debug)]
pub struct PluginDispatchError {
    pub plugin_id: PluginId,
    pub error: BrainError,
}

#[derive(Debug)]
pub struct PluginDispatchReport {
    pub dispatched: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<PluginDispatchError>,
}

pub struct PluginManager {
    registry: RwLock<PluginRegistry>,
    loaders: HashMap<LoaderKind, Box<dyn PluginLoader>>,
}

impl PluginManager {
    pub fn new(loaders: HashMap<LoaderKind, Box<dyn PluginLoader>>) -> Self {
        Self {
            registry: RwLock::new(PluginRegistry::new()),
            loaders,
        }
    }

    pub fn register(&self, installed: InstalledPlugin) -> Result<(), BrainError> {
        let mut reg = self.registry.write();
        reg.register(installed)
    }

    pub fn load(&self, id: &PluginId) -> Result<(), BrainError> {
        let inst = {
            let mut reg = self.registry.write();
            let managed = reg.get_mut(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found in registry", id),
            })?;
            if managed.active_instance.is_none() {
                let loader = self.loaders.get(&managed.installed.loader_kind)
                    .ok_or_else(|| BrainError::Validation {
                        message: format!("No loader configured for kind: {:?}", managed.installed.loader_kind),
                })?;
                let instance = loader.load(&managed.installed)?;
                managed.active_instance = Some(PluginHandle {
                    inner: Arc::new(RwLock::new(instance)),
                });
            }
            managed.active_instance.clone().unwrap()
        };
        let res = inst.inner.write().load();
        res
    }

    pub fn initialize(&self, id: &PluginId) -> Result<(), BrainError> {
        let inst = {
            let reg = self.registry.read();
            let managed = reg.get(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found", id),
            })?;
            managed.active_instance.clone().ok_or_else(|| BrainError::InvalidTransition {
                message: format!("Plugin {} is not loaded", id),
            })?
        };
        let res = inst.inner.write().initialize();
        res
    }

    pub fn activate(&self, id: &PluginId) -> Result<(), BrainError> {
        let inst = {
            let reg = self.registry.read();
            let managed = reg.get(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found", id),
            })?;
            managed.active_instance.clone().ok_or_else(|| BrainError::InvalidTransition {
                message: format!("Plugin {} is not loaded", id),
            })?
        };
        let res = inst.inner.write().activate();
        res
    }

    pub fn suspend(&self, id: &PluginId) -> Result<(), BrainError> {
        let inst = {
            let reg = self.registry.read();
            let managed = reg.get(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found", id),
            })?;
            managed.active_instance.clone().ok_or_else(|| BrainError::InvalidTransition {
                message: format!("Plugin {} is not loaded", id),
            })?
        };
        let res = inst.inner.write().suspend();
        res
    }

    pub fn resume(&self, id: &PluginId) -> Result<(), BrainError> {
        let inst = {
            let reg = self.registry.read();
            let managed = reg.get(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found", id),
            })?;
            managed.active_instance.clone().ok_or_else(|| BrainError::InvalidTransition {
                message: format!("Plugin {} is not loaded", id),
            })?
        };
        let res = inst.inner.write().resume();
        res
    }

    pub fn unload(&self, id: &PluginId) -> Result<(), BrainError> {
        let inst = {
            let reg = self.registry.read();
            let managed = reg.get(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found", id),
            })?;
            managed.active_instance.clone().ok_or_else(|| BrainError::InvalidTransition {
                message: format!("Plugin {} is not loaded", id),
            })?
        };
        let res = inst.inner.write().unload();
        res
    }

    pub fn reload(&self, id: &PluginId) -> Result<(), BrainError> {
        let (descriptor, maybe_handle) = {
            let reg = self.registry.read();
            let managed = reg.get(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found", id),
            })?;
            (managed.installed.clone(), managed.active_instance.clone())
        };

        // If the plugin is not loaded, we do not need to reload it.
        let handle = match maybe_handle {
            Some(h) => h,
            None => return Ok(()),
        };

        let loader = self.loaders.get(&descriptor.loader_kind)
            .ok_or_else(|| BrainError::Validation {
                message: format!("No compatible loader found for kind: {:?}", descriptor.loader_kind),
            })?;

        // 1. Transactionally spin up new instance up to Active in isolation
        let mut new_plugin = loader.load(&descriptor)?;

        // Ensure that failure to load/initialize/activate cleans up the new plugin best-effort
        let mut load_and_activate = || -> Result<(), BrainError> {
            new_plugin.load()?;
            if let Err(e) = new_plugin.initialize() {
                if let Err(ue) = new_plugin.unload() {
                    tracing::warn!(
                        plugin_id = %id,
                        api_version = ?descriptor.manifest.api_version(),
                        phase = "initialize",
                        original_error = ?e,
                        rollback_error = ?ue,
                        "Failed to unload new plugin instance after initialization failure during reload"
                    );
                }
                return Err(e);
            }
            if let Err(e) = new_plugin.activate() {
                if let Err(ue) = new_plugin.unload() {
                    tracing::warn!(
                        plugin_id = %id,
                        api_version = ?descriptor.manifest.api_version(),
                        phase = "activate",
                        original_error = ?e,
                        rollback_error = ?ue,
                        "Failed to unload new plugin instance after activation failure during reload"
                    );
                }
                return Err(e);
            }
            Ok(())
        };

        if let Err(e) = load_and_activate() {
            return Err(e);
        }

        // 2. Perform the RCU swap under the write lock by replacing the inner Box inside the existing active handle.
        // This ensures that concurrent dispatch threads holding cloned handles observe the new plugin instance.
        // Check for concurrent unloads.
        let mut old_plugin = {
            let mut reg = self.registry.write();
            let managed = reg.get_mut(id).ok_or_else(|| BrainError::Validation {
                message: format!("Plugin {} not found during swap", id),
            })?;
            let is_unloaded = managed.active_instance.as_ref()
                .map(|h| h.inner.read().state() == PluginState::Unloaded)
                .unwrap_or(true);
            if is_unloaded {
                // Plugin was unloaded by another thread during reload. Rollback new plugin and abort.
                if let Err(ue) = new_plugin.unload() {
                    tracing::warn!(
                        plugin_id = %id,
                        api_version = ?descriptor.manifest.api_version(),
                        phase = "rollback_unload",
                        original_error = "Plugin was unloaded concurrently",
                        rollback_error = ?ue,
                        "Failed to unload new plugin instance after concurrent unload during reload"
                    );
                }
                return Err(BrainError::InvalidTransition {
                    message: "Plugin was unloaded during reload".to_string(),
                });
            }
            
            // Swap the inner instance inside the existing registry handle.
            let mut inner = handle.inner.write();
            std::mem::replace(&mut *inner, new_plugin)
        };

        // 3. Unload the old instance outside the registry lock (Best effort / rollback safe)
        if let Err(e) = old_plugin.unload() {
            tracing::warn!(
                plugin_id = %id,
                api_version = ?descriptor.manifest.api_version(),
                phase = "unload_old",
                error = ?e,
                "Failed to unload old instance during reload"
            );
        }

        Ok(())
    }

    pub fn list(&self) -> Vec<PluginSummary> {
        self.registry.read().list()
    }

    pub fn dispatch_event(&self, event: &PluginEvent<'_>) -> PluginDispatchReport {
        // Clone Arc handles under short read lock
        let active_plugins: Vec<PluginHandle> = {
            let reg = self.registry.read();
            reg.plugins.values()
                .filter_map(|mp| {
                    if let Some(ref inst) = mp.active_instance {
                        if inst.inner.read().state() == PluginState::Active {
                            return Some(inst.clone());
                        }
                    }
                    None
                })
                .collect()
        };

        // Dispatch outside the registry lock
        let mut dispatched = 0;
        let mut succeeded = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        for handle in active_plugins {
            dispatched += 1;
            let plugin = handle.inner.read();
            match plugin.dispatch(event) {
                Ok(_) => succeeded += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(PluginDispatchError {
                        plugin_id: plugin.manifest().id(),
                        error: e,
                    });
                }
            }
        }

        PluginDispatchReport {
            dispatched,
            succeeded,
            failed,
            errors,
        }
    }
}
