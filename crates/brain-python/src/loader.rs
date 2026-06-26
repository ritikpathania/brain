use pyo3::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::runtime::{
    py_err_to_brain_error, PythonChatAgent, PythonEmbeddingAgent, PythonExtractionAgent,
    PythonPlannerAgent,
};
use brain_core::errors::BrainError;
use brain_core::extensibility::{PluginLifecycle, PluginRegistryLookup};
use brain_domain::{PluginId, PluginState};

/// Representation of the configuration manifest file: `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub api_version: String,
    pub entrypoint: String,
    pub required_permissions: Vec<String>,
}

/// A stateful loaded Python plugin encapsulating Python resources and implementing PluginLifecycle.
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub plugin_id: PluginId,
    pub path: PathBuf,
    pub state: PluginState,
    pub module: Py<PyAny>,
    pub instance: Py<PyAny>,
    pub chat_agent: Option<PythonChatAgent>,
    pub planner_agent: Option<PythonPlannerAgent>,
    pub embedding_agent: Option<PythonEmbeddingAgent>,
    pub extraction_agent: Option<PythonExtractionAgent>,
}

impl PluginLifecycle for LoadedPlugin {
    fn id(&self) -> &PluginId {
        &self.plugin_id
    }

    fn current_state(&self) -> PluginState {
        self.state
    }

    fn dependencies(&self) -> Vec<PluginId> {
        Vec::new()
    }

    fn discover(&mut self, path: &Path) -> Result<(), BrainError> {
        self.path = path.to_path_buf();
        self.state = PluginState::DependenciesResolved;
        Ok(())
    }

    fn resolve_dependencies(
        &mut self,
        _registry: &dyn PluginRegistryLookup,
    ) -> Result<(), BrainError> {
        self.state = PluginState::Validated;
        Ok(())
    }

    fn validate(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Loaded;
        Ok(())
    }

    fn load(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Initialized;
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Active;
        Ok(())
    }

    fn activate(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Active;
        Ok(())
    }

    fn suspend(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Suspended;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Active;
        Ok(())
    }

    fn unload(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Unloaded;
        Ok(())
    }
}

impl LoadedPlugin {
    /// Triggers the optional Python-side `on_load(ctx)` hook.
    pub fn trigger_on_load(
        &mut self,
        py: Python<'_>,
        runtime: std::sync::Arc<dyn brain_core::agents::AgentRuntime>,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            runtime,
            session_id,
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_load", ctx)
    }

    /// Triggers the optional Python-side `on_unload(ctx)` hook.
    pub fn trigger_on_unload(
        &mut self,
        py: Python<'_>,
        runtime: std::sync::Arc<dyn brain_core::agents::AgentRuntime>,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            runtime,
            session_id,
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_unload", ctx)
    }

    /// Triggers the optional Python-side `on_session_start(ctx)` hook.
    pub fn trigger_on_session_start(
        &self,
        py: Python<'_>,
        runtime: std::sync::Arc<dyn brain_core::agents::AgentRuntime>,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            runtime,
            session_id,
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_session_start", ctx)
    }

    /// Triggers the optional Python-side `on_session_end(ctx)` hook.
    pub fn trigger_on_session_end(
        &self,
        py: Python<'_>,
        runtime: std::sync::Arc<dyn brain_core::agents::AgentRuntime>,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            runtime,
            session_id,
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_session_end", ctx)
    }
}

/// Dynamically imports a Python module using importlib.util to isolate individual plugins.
fn load_python_module(
    py: Python<'_>,
    plugin_id: &str,
    entrypoint_path: &Path,
) -> PyResult<PyObject> {
    let importlib_util = py.import_bound("importlib.util")?;
    let entrypoint_str = entrypoint_path.to_string_lossy().to_string();

    let spec =
        importlib_util.call_method1("spec_from_file_location", (plugin_id, &entrypoint_str))?;

    if spec.is_none() {
        return Err(pyo3::exceptions::PyImportError::new_err(format!(
            "Could not create spec from file path: {}",
            entrypoint_str
        )));
    }

    let module = importlib_util.call_method1("module_from_spec", (&spec,))?;

    // Register the module in sys.modules to support relative imports
    let sys = py.import_bound("sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item(plugin_id, &module)?;

    let loader = spec.getattr("loader")?;
    loader.call_method1("exec_module", (&module,))?;

    Ok(module.unbind())
}

/// Robust class resolver checking for custom classes and common naming conventions.
fn find_agent_class<'py>(
    module: &Bound<'py, PyModule>,
    plugin_id: &str,
) -> PyResult<Bound<'py, PyAny>> {
    // 1. Try "Agent"
    if let Ok(cls) = module.getattr("Agent") {
        if cls.is_instance_of::<pyo3::types::PyType>() {
            return Ok(cls);
        }
    }

    // 2. Try camel-cased plugin ID
    let camel_id = plugin_id
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>();
    if let Ok(cls) = module.getattr(camel_id.as_str()) {
        if cls.is_instance_of::<pyo3::types::PyType>() {
            return Ok(cls);
        }
    }

    // 3. Search all attributes of the module for a class/type that defines agent methods
    let dict = module.dict();
    for key in dict.keys() {
        let key_str = key.extract::<String>()?;
        if key_str.starts_with('_') {
            continue;
        }
        if let Ok(val) = module.getattr(key_str.as_str()) {
            if val.is_instance_of::<pyo3::types::PyType>()
                && (val.getattr("chat").is_ok()
                    || val.getattr("plan_steps").is_ok()
                    || val.getattr("embed_text").is_ok()
                    || val.getattr("extract_graph").is_ok())
            {
                return Ok(val);
            }
        }
    }

    Err(pyo3::exceptions::PyAttributeError::new_err(format!(
        "No suitable agent class found in entrypoint for plugin '{}'",
        plugin_id
    )))
}

/// Helper function to locate and execute optional Python hooks.
fn call_lifecycle_hook(
    py: Python<'_>,
    instance: &Py<PyAny>,
    module: &Py<PyAny>,
    hook_name: &str,
    ctx: crate::api::PyRuntimeContext,
) -> Result<(), BrainError> {
    let bound_inst = instance.bind(py);
    if let Ok(hook) = bound_inst.getattr(hook_name) {
        if hook.is_callable() {
            hook.call1((ctx,))
                .map_err(|e| py_err_to_brain_error(py, e))?;
            return Ok(());
        }
    }

    let bound_mod = module.bind(py);
    if let Ok(hook) = bound_mod.getattr(hook_name) {
        if hook.is_callable() {
            hook.call1((ctx,))
                .map_err(|e| py_err_to_brain_error(py, e))?;
            return Ok(());
        }
    }

    Ok(())
}

/// Scans directory structures and manages dynamic Python plugin loading with fault isolation.
pub struct PythonPluginLoader;

impl PythonPluginLoader {
    /// Loads a single Python plugin from its directory.
    pub fn load_plugin(py: Python<'_>, plugin_dir: &Path) -> Result<LoadedPlugin, BrainError> {
        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            return Err(BrainError::Validation {
                message: format!("plugin.toml not found in {}", plugin_dir.display()),
            });
        }

        let manifest_content =
            fs::read_to_string(&manifest_path).map_err(|e| BrainError::Storage {
                message: format!("Failed to read plugin.toml: {}", e),
                source: Some(Box::new(e)),
            })?;

        let manifest: PluginManifest =
            toml::from_str(&manifest_content).map_err(|e| BrainError::Validation {
                message: format!(
                    "Failed to parse plugin.toml in {}: {}",
                    plugin_dir.display(),
                    e
                ),
            })?;

        // Validate API version compatibility
        if manifest.api_version != "v1" {
            return Err(BrainError::Validation {
                message: format!(
                    "Unsupported plugin API version '{}' in plugin '{}'. Only 'v1' is supported.",
                    manifest.api_version, manifest.id
                ),
            });
        }

        let entrypoint_path = plugin_dir.join(&manifest.entrypoint);
        if !entrypoint_path.exists() {
            return Err(BrainError::Validation {
                message: format!(
                    "Entrypoint '{}' not found in plugin '{}'",
                    manifest.entrypoint, manifest.id
                ),
            });
        }

        let module = load_python_module(py, &manifest.id, &entrypoint_path)
            .map_err(|e| py_err_to_brain_error(py, e))?;

        let bound_module =
            module
                .bind(py)
                .downcast::<PyModule>()
                .map_err(|e| BrainError::Python {
                    message: format!("Failed to downcast module to PyModule: {}", e),
                    traceback: None,
                })?;

        let agent_class =
            find_agent_class(bound_module, &manifest.id).map_err(|e| BrainError::Python {
                message: format!(
                    "Failed to find agent class in entrypoint for plugin '{}': {}",
                    manifest.id, e
                ),
                traceback: Some(e.to_string()),
            })?;

        let instance = agent_class
            .call0()
            .map_err(|e| py_err_to_brain_error(py, e))?
            .unbind();

        let chat_agent = if instance.bind(py).getattr("chat").is_ok() {
            Some(PythonChatAgent::new(py, instance.clone())?)
        } else {
            None
        };

        let planner_agent = if instance.bind(py).getattr("plan_steps").is_ok() {
            Some(PythonPlannerAgent::new(py, instance.clone())?)
        } else {
            None
        };

        let embedding_agent = if instance.bind(py).getattr("embed_text").is_ok() {
            Some(PythonEmbeddingAgent::new(py, instance.clone())?)
        } else {
            None
        };

        let extraction_agent = if instance.bind(py).getattr("extract_graph").is_ok() {
            Some(PythonExtractionAgent::new(py, instance.clone())?)
        } else {
            None
        };

        let plugin_id = manifest.id.parse::<PluginId>().unwrap_or_else(|_| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher1 = DefaultHasher::new();
            manifest.id.hash(&mut hasher1);
            let h1 = hasher1.finish();

            let mut hasher2 = DefaultHasher::new();
            (&manifest.id, "salt").hash(&mut hasher2);
            let h2 = hasher2.finish();

            let mut bytes = [0u8; 16];
            bytes[0..8].copy_from_slice(&h1.to_be_bytes());
            bytes[8..16].copy_from_slice(&h2.to_be_bytes());

            PluginId(ulid::Ulid::from_bytes(bytes))
        });

        Ok(LoadedPlugin {
            manifest,
            plugin_id,
            path: plugin_dir.to_path_buf(),
            state: PluginState::Discovered,
            module,
            instance,
            chat_agent,
            planner_agent,
            embedding_agent,
            extraction_agent,
        })
    }

    /// Scans a directory and loads all valid Python plugins with strict fault isolation.
    pub fn scan_and_load_plugins(py: Python<'_>, plugins_dir: &Path) -> Vec<LoadedPlugin> {
        let mut loaded = Vec::new();
        let read_dir = match fs::read_dir(plugins_dir) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(
                    "Failed to read plugins directory {}: {}",
                    plugins_dir.display(),
                    e
                );
                return loaded;
            }
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                match Self::load_plugin(py, &path) {
                    Ok(mut plugin) => {
                        // Automatically run through discovery lifecycle transitions
                        if let Err(e) = plugin.discover(&path) {
                            tracing::error!(
                                "Failed discover lifecycle for plugin '{}': {}",
                                plugin.manifest.id,
                                e
                            );
                        } else {
                            loaded.push(plugin);
                        }
                    }
                    Err(e) => {
                        // Strict plugin isolation: log error and proceed
                        tracing::error!("Failed to load plugin from {}: {}", path.display(), e);
                    }
                }
            }
        }
        loaded
    }
}
