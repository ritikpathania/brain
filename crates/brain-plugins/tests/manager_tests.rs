use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ApiVersion, Plugin, PluginCapabilities, PluginContext, PluginEvent, PluginEventKind,
    PluginEventHandler, PluginLifecycle, PluginManifest, PluginMetadata,
    CapabilityDescriptor, HostContext, ExecutionResult
};
use brain_domain::{PluginId, PluginState, Node, SessionId};
use brain_plugins::{InstalledPlugin, LoaderKind, PluginLoader, PluginManager};

struct DummyHost;

impl HostContext for DummyHost {
    fn retrieve(&self, _session_id: &SessionId, _query: &str, _limit: usize) -> Result<Vec<Node>, BrainError> {
        Ok(Vec::new())
    }

    fn execute_tool(
        &self,
        _session_id: &SessionId,
        _tool_name: &str,
        _arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        Ok(ExecutionResult::new(serde_json::Value::Null))
    }
}

struct MockPlugin {
    manifest: PluginManifest,
    state: PluginState,
    event_count: Arc<RwLock<usize>>,
    should_fail_dispatch: bool,
}

impl PluginMetadata for MockPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

impl PluginLifecycle for MockPlugin {
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

impl PluginCapabilities for MockPlugin {
    fn capabilities(&self) -> &[CapabilityDescriptor] {
        &[]
    }
}

impl PluginEventHandler for MockPlugin {
    fn dispatch(&self, _event: &PluginEvent<'_>) -> Result<(), BrainError> {
        if self.should_fail_dispatch {
            return Err(BrainError::Validation {
                message: "Mock dispatch failure".to_string(),
            });
        }
        let mut count = self.event_count.write();
        *count += 1;
        Ok(())
    }
}

impl Plugin for MockPlugin {}

struct MockLoader {
    event_count: Arc<RwLock<usize>>,
    should_fail_dispatch: bool,
}

impl PluginLoader for MockLoader {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Native
    }

    fn supports(&self, _path: &std::path::Path) -> bool {
        true
    }

    fn load(&self, descriptor: &InstalledPlugin) -> Result<Box<dyn Plugin>, BrainError> {
        Ok(Box::new(MockPlugin {
            manifest: descriptor.manifest.clone(),
            state: PluginState::Discovered,
            event_count: self.event_count.clone(),
            should_fail_dispatch: self.should_fail_dispatch,
        }))
    }
}

fn make_manifest(id: PluginId) -> PluginManifest {
    PluginManifest::new(
        id,
        semver::Version::parse("1.0.0").unwrap(),
        ApiVersion::V1,
        PathBuf::from("entrypoint.py"),
        BTreeSet::new(),
    )
}

#[test]
fn test_plugin_registration_and_lifecycle() {
    let event_count = Arc::new(RwLock::new(0));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = PluginManager::new(loaders);
    let id = PluginId::new();
    let installed = InstalledPlugin {
        manifest: make_manifest(id),
        path: PathBuf::from("/tmp/mock_plugin"),
        loader_kind: LoaderKind::Native,
    };

    manager.register(installed).unwrap();

    let list = manager.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].state, PluginState::Discovered);

    // Run lifecycle transitions
    manager.load(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Loaded);

    manager.initialize(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Initialized);

    manager.activate(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Active);

    // Event dispatch
    let host = DummyHost;
    let context = PluginContext {
        host: &host,
        session_id: None,
    };
    let event = PluginEvent {
        kind: PluginEventKind::SessionStart,
        context: &context,
    };
    let report = manager.dispatch_event(&event);
    assert_eq!(report.dispatched, 1);
    assert_eq!(report.succeeded, 1);
    assert_eq!(*event_count.read(), 1);

    // Suspend / Resume
    manager.suspend(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Suspended);

    manager.resume(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Active);

    // Unload
    manager.unload(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Unloaded);
}

#[test]
fn test_plugin_dispatch_errors() {
    let event_count = Arc::new(RwLock::new(0));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: true,
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = PluginManager::new(loaders);
    let id = PluginId::new();
    let installed = InstalledPlugin {
        manifest: make_manifest(id),
        path: PathBuf::from("/tmp/mock_plugin"),
        loader_kind: LoaderKind::Native,
    };

    manager.register(installed).unwrap();
    manager.load(&id).unwrap();
    manager.initialize(&id).unwrap();
    manager.activate(&id).unwrap();

    let host = DummyHost;
    let context = PluginContext {
        host: &host,
        session_id: None,
    };
    let event = PluginEvent {
        kind: PluginEventKind::SessionStart,
        context: &context,
    };
    let report = manager.dispatch_event(&event);
    assert_eq!(report.dispatched, 1);
    assert_eq!(report.succeeded, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].plugin_id, id);
}

#[test]
fn test_plugin_hot_reload() {
    let event_count = Arc::new(RwLock::new(0));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = PluginManager::new(loaders);
    let id = PluginId::new();
    let installed = InstalledPlugin {
        manifest: make_manifest(id),
        path: PathBuf::from("/tmp/mock_plugin"),
        loader_kind: LoaderKind::Native,
    };

    manager.register(installed).unwrap();
    manager.load(&id).unwrap();
    manager.initialize(&id).unwrap();
    manager.activate(&id).unwrap();

    // Reload
    manager.reload(&id).unwrap();
    assert_eq!(manager.list()[0].state, PluginState::Active);
}
