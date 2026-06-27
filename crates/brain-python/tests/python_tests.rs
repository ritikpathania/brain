use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use pyo3::prelude::*;

use brain_core::agents::{ChatAgent, EmbeddingAgent, ExtractionAgent, PlannerAgent};
use brain_core::errors::BrainError;
use brain_core::extensibility::{PluginLifecycle, HostContext};
use brain_domain::{Conversation, Node, NodeId, NodeType, PluginId, PluginState, SessionId};
use brain_plugins::{InstalledPlugin, LoaderKind};
use brain_python::loader::PythonPluginLoader;

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new() -> Self {
        let uuid_str = ulid::Ulid::new().to_string();
        let path = std::env::temp_dir().join(format!("brain_test_plugins_{}", uuid_str));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct MockAgentRuntime {
    pub should_fail_tool: bool,
}

impl HostContext for MockAgentRuntime {
    fn retrieve(
        &self,
        _session_id: &SessionId,
        query: &str,
        _limit: usize,
    ) -> Result<Vec<Node>, BrainError> {
        if query == "empty" {
            Ok(Vec::new())
        } else {
            Ok(vec![Node::new(
                NodeId::new(),
                "MockNode".to_string(),
                NodeType::Concept,
            )])
        }
    }

    fn execute_tool(
        &self,
        _session_id: &SessionId,
        tool_name: &str,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<brain_core::extensibility::ExecutionResult, BrainError> {
        if self.should_fail_tool {
            return Err(BrainError::Authorization {
                message: "Permission denied".to_string(),
            });
        }

        let mut ret = serde_json::Map::new();
        ret.insert(
            "tool".to_string(),
            serde_json::Value::String(tool_name.to_string()),
        );
        ret.insert(
            "args".to_string(),
            serde_json::Value::Object(arguments.clone().into_iter().collect()),
        );
        Ok(brain_core::extensibility::ExecutionResult::new(
            serde_json::Value::Object(ret),
        ))
    }
}

fn create_test_plugin(
    base_dir: &Path,
    plugin_name: &str,
    manifest_toml: &str,
    py_code: &str,
) -> PathBuf {
    let dir = base_dir.join(plugin_name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("plugin.toml"), manifest_toml).unwrap();
    fs::write(dir.join("plugin.py"), py_code).unwrap();
    dir
}

#[test]
fn test_plugin_loader_and_agent_wrappers() {
    pyo3::prepare_freethreaded_python();
    let guard = TempDirGuard::new();

    let manifest = r#"
id = "mock_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "plugin.py"
required_permissions = ["filesystem_read"]
"#;

    let py_code = r#"
class MockPlugin:
    def __init__(self):
        self.plugin_id = "mock_plugin"
        self.api_version = "v1"
        self.name = "MockAgent"
        self.dimension = 256

    def chat(self, session_id, prompt):
        return f"Chatted: {prompt}"

    def plan_steps(self, task_description, history):
        return [
            {
                "call_id": "call_123",
                "tool_name": "mock_tool",
                "arguments": {"task": task_description}
            }
        ]

    def embed_text(self, text):
        return [0.5, 0.6, 0.7]

    def extract_graph(self, text):
        return (
            [{"id": "node_99", "label": "Node 99", "node_type": "project", "attributes": {}}],
            []
        )

    def on_load(self, ctx):
        # Verify retrieve works
        nodes = ctx.retrieve("test_query", 5)
        assert len(nodes) == 1
        assert nodes[0].label == "MockNode"

    def on_unload(self, ctx):
        pass

    def on_session_start(self, ctx):
        pass

    def on_session_end(self, ctx):
        pass
"#;

    let plugin_dir = create_test_plugin(&guard.path, "mock_plugin", manifest, py_code);

    let manifest_path = plugin_dir.join("plugin.toml");
    let core_manifest = brain_core::extensibility::PluginManifest::from_path(&manifest_path).unwrap();
    let installed = InstalledPlugin {
        manifest: core_manifest,
        path: plugin_dir.clone(),
        loader_kind: LoaderKind::Python,
    };
    let mut loaded =
        Python::with_gil(|py| PythonPluginLoader::load_plugin(py, &installed).unwrap());

    // Verify PluginLifecycle implementations
    assert_eq!(loaded.manifest.id(), "mock_plugin".parse::<PluginId>().unwrap());
    assert_eq!(loaded.state(), PluginState::Discovered);

    loaded.load().unwrap();
    assert_eq!(loaded.state(), PluginState::Loaded);

    loaded.initialize().unwrap();
    assert_eq!(loaded.state(), PluginState::Initialized);

    loaded.activate().unwrap();
    assert_eq!(loaded.state(), PluginState::Active);

    // Verify lifecycle hook trigger calls PyRuntimeContext retrieve
    let runtime = Arc::new(MockAgentRuntime {
        should_fail_tool: false,
    });
    let session_id = SessionId::new();

    Python::with_gil(|py| {
        loaded
            .trigger_on_load(py, &*runtime, session_id)
            .unwrap();
        loaded
            .trigger_on_session_start(py, &*runtime, session_id)
            .unwrap();
    });

    // Verify ChatAgent
    let chat_agent = loaded.chat_agent.as_ref().unwrap();
    assert_eq!(chat_agent.name(), "MockAgent");
    let chat_res = chat_agent.chat(session_id, "hello").unwrap();
    assert_eq!(chat_res, "Chatted: hello");

    // Verify PlannerAgent
    let planner_agent = loaded.planner_agent.as_ref().unwrap();
    let history = Conversation::new_empty();
    let plan = planner_agent.plan_steps("task", &history).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].call_id, "call_123");
    assert_eq!(plan[0].tool_name, "mock_tool");

    // Verify EmbeddingAgent
    let embedding_agent = loaded.embedding_agent.as_ref().unwrap();
    assert_eq!(embedding_agent.dimension(), 256);
    let emb = embedding_agent.embed_text("abc").unwrap();
    assert_eq!(emb, vec![0.5, 0.6, 0.7]);

    // Verify ExtractionAgent
    let extraction_agent = loaded.extraction_agent.as_ref().unwrap();
    let (nodes, edges) = extraction_agent.extract_graph("text").unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, "node_99");
    assert_eq!(nodes[0].label, "Node 99");
    assert!(edges.is_empty());
}



#[test]
fn test_plugin_isolation_and_fault_tolerance() {
    pyo3::prepare_freethreaded_python();
    let guard = TempDirGuard::new();

    // 1. Create a valid plugin
    let manifest_ok = r#"
id = "ok_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "plugin.py"
required_permissions = []
"#;
    let py_code_ok = r#"
class OkPlugin:
    def chat(self, session_id, prompt):
        return "OK"
"#;
    create_test_plugin(&guard.path, "ok_plugin", manifest_ok, py_code_ok);

    // 2. Create a broken plugin with Python syntax error
    let manifest_broken = r#"
id = "broken_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "plugin.py"
required_permissions = []
"#;
    let py_code_broken = r#"
class BrokenPlugin
    # Syntax error: missing colon
    def chat(self):
        pass
"#;
    create_test_plugin(
        &guard.path,
        "broken_plugin",
        manifest_broken,
        py_code_broken,
    );

    let loaded = Python::with_gil(|py| PythonPluginLoader::scan_and_load_plugins(py, &guard.path));

    // Verify that the valid plugin is loaded successfully, and the broken one is isolated and skipped
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].manifest.id(), "ok_plugin".parse::<PluginId>().unwrap());
}

#[test]
fn test_gil_release_and_parallelism() {
    pyo3::prepare_freethreaded_python();
    let guard = TempDirGuard::new();

    let manifest = r#"
id = "parallel_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "plugin.py"
required_permissions = []
"#;
    let py_code = r#"
import time
class ParallelPlugin:
    def chat(self, session_id, prompt):
        # Simulate CPU work or wait
        time.sleep(0.01)
        return "done"
"#;
    let plugin_dir = create_test_plugin(&guard.path, "parallel_plugin", manifest, py_code);

    let loaded = Python::with_gil(|py| PythonPluginLoader::load_from_dir(py, &plugin_dir).unwrap());

    let loaded = Arc::new(loaded);
    let mut handles = Vec::new();

    for i in 0..5 {
        let plugin = loaded.clone();
        let handle = thread::spawn(move || {
            let session_id = SessionId::new();
            let chat_agent = plugin.chat_agent.as_ref().unwrap();
            let prompt = format!("prompt {}", i);
            let res = chat_agent.chat(session_id, &prompt).unwrap();
            assert_eq!(res, "done");
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_permission_checks() {
    pyo3::prepare_freethreaded_python();
    let guard = TempDirGuard::new();

    let manifest = r#"
id = "permission_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "plugin.py"
required_permissions = []
"#;
    let py_code = r#"
class PermissionPlugin:
    def chat(self, session_id, prompt):
        return "done"
    def on_load(self, ctx):
        # This will fail because the mock runtime is set to fail tool execution
        ctx.execute_tool("mock_tool", {"arg": "val"})
"#;
    let plugin_dir = create_test_plugin(&guard.path, "permission_plugin", manifest, py_code);

    let loaded =
        Python::with_gil(|py| PythonPluginLoader::load_from_dir(py, &plugin_dir).unwrap());
    let runtime = Arc::new(MockAgentRuntime {
        should_fail_tool: true,
    });
    let session_id = SessionId::new();

    // Expect the execution error to propagate back as a BrainError::Python wrapping PyErr
    let res = Python::with_gil(|py| loaded.trigger_on_load(py, &*runtime, session_id));
    assert!(res.is_err());
    match res.err().unwrap() {
        BrainError::Python { message, .. } => {
            assert!(message.contains("Permission denied"));
        }
        other => panic!("Expected BrainError::Python, got: {:?}", other),
    }
}

#[test]
fn test_exception_propagation_and_tracebacks() {
    pyo3::prepare_freethreaded_python();
    let guard = TempDirGuard::new();

    let manifest = r#"
id = "exception_plugin"
version = "1.0.0"
api_version = "v1"
entrypoint = "plugin.py"
required_permissions = []
"#;
    let py_code = r#"
class ExceptionPlugin:
    def chat(self, session_id, prompt):
        raise ValueError("simulated python exception")
"#;
    let plugin_dir = create_test_plugin(&guard.path, "exception_plugin", manifest, py_code);

    let loaded = Python::with_gil(|py| PythonPluginLoader::load_from_dir(py, &plugin_dir).unwrap());
    let chat_agent = loaded.chat_agent.as_ref().unwrap();
    let session_id = SessionId::new();

    let res = chat_agent.chat(session_id, "hello");
    assert!(res.is_err());
    match res.err().unwrap() {
        BrainError::Python { message, traceback } => {
            assert!(message.contains("ValueError: simulated python exception"));
            assert!(traceback.is_some());
            let tb = traceback.unwrap();
            assert!(tb.contains("chat"));
        }
        other => panic!("Expected BrainError::Python, got: {:?}", other),
    }
}

#[test]
fn test_version_validation() {
    pyo3::prepare_freethreaded_python();
    let guard = TempDirGuard::new();

    let manifest = r#"
id = "incompatible_plugin"
version = "1.0.0"
api_version = "v2"
entrypoint = "plugin.py"
required_permissions = []
"#;
    let py_code = r#"
class IncompatiblePlugin:
    pass
"#;
    let plugin_dir = create_test_plugin(&guard.path, "incompatible_plugin", manifest, py_code);

    let res = Python::with_gil(|py| PythonPluginLoader::load_from_dir(py, &plugin_dir));
    assert!(res.is_err());
    match res.err().unwrap() {
        BrainError::Validation { message } => {
            assert!(message.contains("Unsupported plugin API version 'v2'"));
        }
        other => panic!("Expected BrainError::Validation, got: {:?}", other),
    }
}
