use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ApiVersion, CapabilityDescriptor, ExecutionResult, HostContext, Plugin, PluginCapabilities,
    PluginContext, PluginEvent, PluginEventHandler, PluginEventKind, PluginLifecycle,
    PluginManifest, PluginMetadata,
};
use brain_domain::{Node, PluginId, PluginState, SessionId};
use brain_plugins::{InstalledPlugin, LoaderKind, PluginLoader, PluginManager, PluginScanner};
use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

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

struct BenchPlugin {
    manifest: PluginManifest,
    state: PluginState,
}
impl PluginMetadata for BenchPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}
impl PluginLifecycle for BenchPlugin {
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
impl PluginCapabilities for BenchPlugin {
    fn capabilities(&self) -> &[CapabilityDescriptor] {
        &[]
    }
}
impl PluginEventHandler for BenchPlugin {
    fn dispatch(&self, _event: &PluginEvent<'_>) -> Result<(), BrainError> {
        Ok(())
    }
}
impl Plugin for BenchPlugin {}

struct BenchLoader;
impl PluginLoader for BenchLoader {
    fn kind(&self) -> LoaderKind {
        LoaderKind::Native
    }
    fn supports(&self, _path: &Path) -> bool {
        true
    }
    fn load(&self, descriptor: &InstalledPlugin) -> Result<Box<dyn Plugin>, BrainError> {
        Ok(Box::new(BenchPlugin {
            manifest: descriptor.manifest.clone(),
            state: PluginState::Discovered,
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

fn bench_plugin_operations(c: &mut Criterion) {
    // 1. Discovery benchmark (scanning a directory)
    let temp_dir = std::env::temp_dir().join(format!("bench_plugins_{}", std::process::id()));
    let plugin_dir = temp_dir.join("mock_plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let toml_content = r#"
id = "bench_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "entrypoint.py"
required_permissions = []
"#;
    std::fs::write(plugin_dir.join("plugin.toml"), toml_content).unwrap();

    let loaders = vec![Box::new(BenchLoader) as Box<dyn PluginLoader>];
    let scanner = PluginScanner::new(loaders);

    c.bench_function("discovery", |b| {
        b.iter(|| {
            let res = scanner.scan_directory(&temp_dir).unwrap();
            assert!(!res.is_empty());
        })
    });

    let _ = std::fs::remove_dir_all(&temp_dir);

    // 2. Setup manager and registries for other benchmarks
    let mut loaders = HashMap::new();
    loaders.insert(
        LoaderKind::Native,
        Box::new(BenchLoader) as Box<dyn PluginLoader>,
    );
    let manager = PluginManager::new(loaders);
    let id = PluginId::new();
    let installed = InstalledPlugin {
        manifest: make_manifest(id),
        path: PathBuf::from("/tmp/mock_plugin"),
        loader_kind: LoaderKind::Native,
    };

    manager.register(installed).unwrap();

    // 3. Lookup benchmark
    c.bench_function("lookup", |b| {
        b.iter(|| {
            let list = manager.list();
            assert_eq!(list.len(), 1);
        })
    });

    // 4. Load benchmark
    c.bench_function("load", |b| {
        b.iter(|| {
            let _ = manager.unload(&id);
            manager.load(&id).unwrap();
            manager.initialize(&id).unwrap();
            manager.activate(&id).unwrap();
        })
    });

    // 5. Reload benchmark
    c.bench_function("reload", |b| {
        b.iter(|| {
            manager.reload(&id).unwrap();
        })
    });

    // 6. Dispatch benchmark
    let host = DummyHost;
    let context = PluginContext {
        host: &host,
        session_id: None,
    };
    let event = PluginEvent {
        kind: PluginEventKind::SessionStart,
        context: &context,
    };

    c.bench_function("dispatch", |b| {
        b.iter(|| {
            let report = manager.dispatch_event(&event);
            assert_eq!(report.succeeded, 1);
        })
    });
}

criterion_group!(benches, bench_plugin_operations);
criterion_main!(benches);
