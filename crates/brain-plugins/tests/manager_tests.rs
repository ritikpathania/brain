use parking_lot::RwLock;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ApiVersion, CapabilityDescriptor, ExecutionResult, HostContext, Plugin, PluginCapabilities,
    PluginContext, PluginEvent, PluginEventHandler, PluginEventKind, PluginLifecycle,
    PluginManifest, PluginMetadata,
};
use brain_domain::{Node, PluginId, PluginState, SessionId};
use brain_plugins::{InstalledPlugin, LoaderKind, PluginLoader, PluginManager};

struct DummyHost;

impl HostContext for DummyHost {
    fn retrieve(
        &self,
        _session_id: &SessionId,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<Node>, BrainError> {
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
    should_fail_initialize: bool,
    should_fail_activate: bool,
    barrier: Option<Arc<std::sync::Barrier>>,
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
        if self.should_fail_initialize {
            return Err(BrainError::Validation {
                message: "Mock initialize failure".to_string(),
            });
        }
        if let Some(ref barrier) = self.barrier {
            barrier.wait(); // Wait for test thread to signal unload start
            barrier.wait(); // Wait for test thread to complete unload
        }
        self.state = PluginState::Initialized;
        Ok(())
    }

    fn activate(&mut self) -> Result<(), BrainError> {
        if self.should_fail_activate {
            return Err(BrainError::Validation {
                message: "Mock activate failure".to_string(),
            });
        }
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
    should_fail_initialize: bool,
    should_fail_activate: bool,
    barrier: Option<Arc<std::sync::Barrier>>,
    load_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl PluginLoader for MockLoader {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Native
    }

    fn supports(&self, _path: &std::path::Path) -> bool {
        true
    }

    fn load(&self, descriptor: &InstalledPlugin) -> Result<Box<dyn Plugin>, BrainError> {
        let count = self
            .load_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let active_barrier = if count >= 1 {
            self.barrier.clone()
        } else {
            None
        };
        let active_fail_activate = if count >= 1 {
            self.should_fail_activate
        } else {
            false
        };
        let active_fail_initialize = if count >= 1 {
            self.should_fail_initialize
        } else {
            false
        };
        println!(
            "MOCKLOADER LOAD: count = {}, active_fail_activate = {}",
            count, active_fail_activate
        );
        Ok(Box::new(MockPlugin {
            manifest: descriptor.manifest.clone(),
            state: PluginState::Discovered,
            event_count: self.event_count.clone(),
            should_fail_dispatch: self.should_fail_dispatch,
            should_fail_initialize: active_fail_initialize,
            should_fail_activate: active_fail_activate,
            barrier: active_barrier,
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
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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

#[test]
fn test_plugin_manager_concurrency_stress() {
    let event_count = Arc::new(RwLock::new(0));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = Arc::new(PluginManager::new(loaders));
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

    let mut threads = Vec::new();

    // Spawn 8 threads running parallel operations
    // Thread 1 & 2: reload(id)
    for _ in 0..2 {
        let m = manager.clone();
        threads.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = m.reload(&id);
            }
        }));
    }

    // Thread 3 & 4: dispatch_event(&event)
    let host = Arc::new(DummyHost);
    for _ in 0..2 {
        let m = manager.clone();
        let h = host.clone();
        threads.push(thread::spawn(move || {
            let context = PluginContext {
                host: &*h,
                session_id: None,
            };
            let event = PluginEvent {
                kind: PluginEventKind::SessionStart,
                context: &context,
            };
            for _ in 0..10_000 {
                let _ = m.dispatch_event(&event);
            }
        }));
    }

    // Thread 5: list()
    {
        let m = manager.clone();
        threads.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = m.list();
            }
        }));
    }

    // Thread 6: load(id)
    {
        let m = manager.clone();
        threads.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = m.load(&id);
            }
        }));
    }

    // Thread 7: unload(id)
    {
        let m = manager.clone();
        threads.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = m.unload(&id);
            }
        }));
    }

    // Thread 8: suspend(id) / resume(id)
    {
        let m = manager.clone();
        threads.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = m.suspend(&id);
                let _ = m.resume(&id);
            }
        }));
    }

    // Join all threads
    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn test_reload_race_parallel_reloads() {
    let event_count = Arc::new(RwLock::new(0));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = Arc::new(PluginManager::new(loaders));
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

    // Spawn 4 threads to call reload simultaneously
    let mut handles = Vec::new();
    for _ in 0..4 {
        let m = manager.clone();
        handles.push(thread::spawn(move || m.reload(&id)));
    }

    for h in handles {
        assert!(h.join().unwrap().is_ok());
    }

    assert_eq!(manager.list()[0].state, PluginState::Active);
}

#[test]
fn test_reload_race_unload_during_reload() {
    let event_count = Arc::new(RwLock::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: Some(barrier.clone()),
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = Arc::new(PluginManager::new(loaders));
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

    // Spawn thread to call reload, which will block on initialization
    let m = manager.clone();
    let reload_handle = thread::spawn(move || m.reload(&id));

    // Wait for the reload thread to enter initialize() and block
    barrier.wait();

    // Concurrently unload the plugin (sets active_instance = None)
    manager.unload(&id).unwrap();

    // Signal the reload thread to continue
    barrier.wait();

    // Reload must fail because the plugin was concurrently unloaded during reload
    let res = reload_handle.join().unwrap();
    assert!(res.is_err());
    match res.err().unwrap() {
        BrainError::InvalidTransition { message } => {
            assert!(message.contains("Plugin was unloaded during reload"));
        }
        other => panic!("Expected InvalidTransition, got {:?}", other),
    }
}

#[test]
fn test_reload_rcu_dispatch_routing() {
    let event_count = Arc::new(RwLock::new(0));
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = Arc::new(PluginManager::new(loaders));
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

    // Dispatch before reload
    let host = DummyHost;
    let context = PluginContext {
        host: &host,
        session_id: None,
    };
    let event = PluginEvent {
        kind: PluginEventKind::SessionStart,
        context: &context,
    };
    manager.dispatch_event(&event);
    assert_eq!(*event_count.read(), 1);

    // Reload
    manager.reload(&id).unwrap();

    // Dispatch after reload
    manager.dispatch_event(&event);
    assert_eq!(*event_count.read(), 2);
}

#[test]
fn test_reload_loader_failure_cleanup() {
    let event_count = Arc::new(RwLock::new(0));
    // A loader configured to fail activation specifically during reload (count >= 1)
    let loader = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
        should_fail_initialize: false,
        should_fail_activate: true,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader as Box<dyn PluginLoader>);

    let manager = Arc::new(PluginManager::new(loaders));
    let id = PluginId::new();
    let installed = InstalledPlugin {
        manifest: make_manifest(id),
        path: PathBuf::from("/tmp/mock_plugin"),
        loader_kind: LoaderKind::Native,
    };

    manager.register(installed).unwrap();
    manager.load(&id).unwrap();
    manager.initialize(&id).unwrap();
    manager.activate(&id).unwrap(); // This succeeds because load_count was 0

    // Reload must fail because the new instance fails activation (load_count = 1)
    let reload_res = manager.reload(&id);
    assert!(reload_res.is_err());

    // The registry state should still remain Active (points to the original plugin)
    assert_eq!(manager.list()[0].state, PluginState::Active);
}

#[test]
fn test_event_dispatch_isolation() {
    let event_count = Arc::new(RwLock::new(0));

    // Create 3 loaders: A fails, B and C succeed.
    let loader_ok = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: false,
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let loader_fail = Box::new(MockLoader {
        event_count: event_count.clone(),
        should_fail_dispatch: true,
        should_fail_initialize: false,
        should_fail_activate: false,
        barrier: None,
        load_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let mut loaders = HashMap::new();
    loaders.insert(LoaderKind::Native, loader_ok as Box<dyn PluginLoader>);
    loaders.insert(LoaderKind::Python, loader_fail as Box<dyn PluginLoader>);
    let manager = PluginManager::new(loaders);

    let id_a = PluginId::new();
    let id_b = PluginId::new();
    let id_c = PluginId::new();

    let installed_a = InstalledPlugin {
        manifest: make_manifest(id_a),
        path: PathBuf::from("/tmp/plugin_a"),
        loader_kind: LoaderKind::Python,
    };
    let installed_b = InstalledPlugin {
        manifest: make_manifest(id_b),
        path: PathBuf::from("/tmp/plugin_b"),
        loader_kind: LoaderKind::Native,
    };
    let installed_c = InstalledPlugin {
        manifest: make_manifest(id_c),
        path: PathBuf::from("/tmp/plugin_c"),
        loader_kind: LoaderKind::Native,
    };

    manager.register(installed_a).unwrap();
    manager.register(installed_b).unwrap();
    manager.register(installed_c).unwrap();

    // Load, initialize and activate all
    for id in &[id_a, id_b, id_c] {
        manager.load(id).unwrap();
        manager.initialize(id).unwrap();
        manager.activate(id).unwrap();
    }

    let host = DummyHost;
    let context = PluginContext {
        host: &host,
        session_id: None,
    };
    let event = PluginEvent {
        kind: PluginEventKind::SessionStart,
        context: &context,
    };

    // Dispatch event
    let report = manager.dispatch_event(&event);

    // Verify: B and C still execute, only A's error is reported, registry state unchanged
    assert_eq!(report.dispatched, 3);
    assert_eq!(report.succeeded, 2);
    assert_eq!(report.failed, 1);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].plugin_id, id_a);
    assert!(report.errors[0]
        .error
        .to_string()
        .contains("Mock dispatch failure"));

    // Check states
    assert_eq!(
        manager.list().iter().find(|p| p.id == id_a).unwrap().state,
        PluginState::Active
    );
    assert_eq!(
        manager.list().iter().find(|p| p.id == id_b).unwrap().state,
        PluginState::Active
    );
    assert_eq!(
        manager.list().iter().find(|p| p.id == id_c).unwrap().state,
        PluginState::Active
    );
}
