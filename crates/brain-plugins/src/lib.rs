use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;

use brain_core::errors::BrainError;
use brain_core::extensibility::{ApiVersion, Plugin, PluginManifest};
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
    pub inner: Arc<RwLock<dyn Plugin>>,
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
