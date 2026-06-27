use pyo3::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::runtime::{
    py_err_to_brain_error, PythonChatAgent, PythonEmbeddingAgent, PythonExtractionAgent,
    PythonPlannerAgent,
};
use brain_core::errors::BrainError;
use brain_core::extensibility::{
    Plugin, PluginCapabilities, PluginEvent, PluginEventKind,
    PluginEventHandler, PluginLifecycle, PluginManifest, PluginMetadata, PluginCapability,
    CapabilityDescriptor, HostContext
};
use brain_domain::{PluginId, PluginState};
use brain_plugins::{InstalledPlugin, LoaderKind, PluginLoader};

/// A stateful loaded Python plugin encapsulating Python resources and implementing Plugin.
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
    pub capabilities: Vec<CapabilityDescriptor>,
}

impl PluginMetadata for LoadedPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

impl PluginLifecycle for LoadedPlugin {
    fn state(&self) -> PluginState {
        self.state
    }

    fn load(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Loaded;
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), BrainError> {
        self.state = PluginState::Initialized;
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

impl PluginCapabilities for LoadedPlugin {
    fn capabilities(&self) -> &[CapabilityDescriptor] {
        &self.capabilities
    }
}

impl PluginEventHandler for LoadedPlugin {
    fn dispatch(&self, event: &PluginEvent<'_>) -> Result<(), BrainError> {
        Python::with_gil(|py| {
            let session_id = event.context.session_id.ok_or_else(|| BrainError::Validation {
                message: "Session ID is required for Python runtime context".to_string(),
            })?;
            let ctx = crate::api::PyRuntimeContext {
                host_ptr: unsafe { std::mem::transmute(event.context.host) },
                session_id: Some(session_id),
                is_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            };
            match event.kind {
                PluginEventKind::Load => {
                    call_lifecycle_hook(py, &self.instance, &self.module, "on_load", ctx)
                }
                PluginEventKind::Unload => {
                    call_lifecycle_hook(py, &self.instance, &self.module, "on_unload", ctx)
                }
                PluginEventKind::SessionStart => {
                    call_lifecycle_hook(py, &self.instance, &self.module, "on_session_start", ctx)
                }
                PluginEventKind::SessionEnd => {
                    call_lifecycle_hook(py, &self.instance, &self.module, "on_session_end", ctx)
                }
            }
        })
    }
}

impl Plugin for LoadedPlugin {}

impl LoadedPlugin {
    /// Triggers the optional Python-side `on_load(ctx)` hook.
    pub fn trigger_on_load(
        &self,
        py: Python<'_>,
        host: &dyn HostContext,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            host_ptr: unsafe { std::mem::transmute(host) },
            session_id: Some(session_id),
            is_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_load", ctx)
    }

    /// Triggers the optional Python-side `on_unload(ctx)` hook.
    pub fn trigger_on_unload(
        &self,
        py: Python<'_>,
        host: &dyn HostContext,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            host_ptr: unsafe { std::mem::transmute(host) },
            session_id: Some(session_id),
            is_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_unload", ctx)
    }

    /// Triggers the optional Python-side `on_session_start(ctx)` hook.
    pub fn trigger_on_session_start(
        &self,
        py: Python<'_>,
        host: &dyn HostContext,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            host_ptr: unsafe { std::mem::transmute(host) },
            session_id: Some(session_id),
            is_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        call_lifecycle_hook(py, &self.instance, &self.module, "on_session_start", ctx)
    }

    /// Triggers the optional Python-side `on_session_end(ctx)` hook.
    pub fn trigger_on_session_end(
        &self,
        py: Python<'_>,
        host: &dyn HostContext,
        session_id: brain_domain::SessionId,
    ) -> Result<(), BrainError> {
        let ctx = crate::api::PyRuntimeContext {
            host_ptr: unsafe { std::mem::transmute(host) },
            session_id: Some(session_id),
            is_valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
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
    let is_valid = ctx.is_valid.clone();
    let res = (|| {
        let bound_inst = instance.bind(py);
        if let Ok(hook) = bound_inst.getattr(hook_name) {
            if hook.is_callable() {
                hook.call1((ctx.clone(),))
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
    })();
    // Invalidate the runtime context immediately on hook completion to detect Python misuse.
    is_valid.store(false, std::sync::atomic::Ordering::Relaxed);
    res
}

/// Scans directory structures and manages dynamic Python plugin loading with fault isolation.
pub struct PythonPluginLoader;

impl PluginLoader for PythonPluginLoader {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Python
    }

    fn supports(&self, path: &Path) -> bool {
        path.join("plugin.toml").exists()
    }

    fn load(&self, descriptor: &InstalledPlugin) -> Result<Box<dyn Plugin>, BrainError> {
        Python::with_gil(|py| {
            let loaded = Self::load_plugin(py, descriptor)?;
            Ok(Box::new(loaded) as Box<dyn Plugin>)
        })
    }
}

impl PythonPluginLoader {
    /// Loads a single Python plugin from its directory.
    pub fn load_plugin(py: Python<'_>, descriptor: &InstalledPlugin) -> Result<LoadedPlugin, BrainError> {
        let manifest = &descriptor.manifest;
        let plugin_dir = &descriptor.path;

        let entrypoint_path = plugin_dir.join(manifest.entrypoint());
        if !entrypoint_path.exists() {
            return Err(BrainError::Validation {
                message: format!(
                    "Entrypoint '{}' not found in plugin '{}'",
                    manifest.entrypoint().display(),
                    manifest.id()
                ),
            });
        }

        let module = load_python_module(py, &manifest.id().to_string(), &entrypoint_path)
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
            find_agent_class(bound_module, &manifest.id().to_string()).map_err(|e| BrainError::Python {
                message: format!(
                    "Failed to find agent class in entrypoint for plugin '{}': {}",
                    manifest.id(), e
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

        let mut capabilities = Vec::new();
        if let Some(agent) = chat_agent.clone() {
            capabilities.push(CapabilityDescriptor {
                name: "chat",
                capability: PluginCapability::Chat(Arc::new(agent)),
            });
        }
        if let Some(agent) = planner_agent.clone() {
            capabilities.push(CapabilityDescriptor {
                name: "planner",
                capability: PluginCapability::Planner(Arc::new(agent)),
            });
        }
        if let Some(agent) = embedding_agent.clone() {
            capabilities.push(CapabilityDescriptor {
                name: "embedding",
                capability: PluginCapability::Embedding(Arc::new(agent)),
            });
        }
        if let Some(agent) = extraction_agent.clone() {
            capabilities.push(CapabilityDescriptor {
                name: "extraction",
                capability: PluginCapability::Extraction(Arc::new(agent)),
            });
        }

        Ok(LoadedPlugin {
            manifest: manifest.clone(),
            plugin_id: manifest.id(),
            path: plugin_dir.to_path_buf(),
            state: PluginState::Discovered,
            module,
            instance,
            chat_agent,
            planner_agent,
            embedding_agent,
            extraction_agent,
            capabilities,
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
                let manifest_path = path.join("plugin.toml");
                if manifest_path.exists() {
                    if let Ok(manifest) = PluginManifest::from_path(&manifest_path) {
                        let installed = InstalledPlugin {
                            manifest,
                            path: path.clone(),
                            loader_kind: LoaderKind::Python,
                        };
                        match Self::load_plugin(py, &installed) {
                            Ok(plugin) => {
                                loaded.push(plugin);
                            }
                            Err(e) => {
                                tracing::error!("Failed to load plugin from {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }
        loaded
    }

    /// Loads a plugin from its directory.
    pub fn load_from_dir(py: Python<'_>, plugin_dir: &Path) -> Result<LoadedPlugin, BrainError> {
        let manifest_path = plugin_dir.join("plugin.toml");
        let manifest = PluginManifest::from_path(&manifest_path)?;
        let descriptor = InstalledPlugin {
            manifest,
            path: plugin_dir.to_path_buf(),
            loader_kind: LoaderKind::Python,
        };
        Self::load_plugin(py, &descriptor)
    }
}
