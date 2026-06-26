use brain_core::agents::{ChatAgent, EmbeddingAgent, ExtractionAgent, PlannerAgent};
use brain_core::errors::BrainError;
use brain_domain::{Conversation, EdgeDTO, NodeDTO, SessionId, ToolCall};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::collections::HashMap;

/// Helper function to extract and format a Python exception traceback string.
fn get_traceback_str(py: Python<'_>, err: &PyErr) -> Option<String> {
    if let Some(tb) = err.traceback_bound(py) {
        if let Ok(traceback_module) = py.import_bound("traceback") {
            if let Ok(formatted) =
                traceback_module.call_method1("format_exception", (err.value_bound(py),))
            {
                if let Ok(list) = formatted.extract::<Vec<String>>() {
                    return Some(list.join(""));
                }
            }
            if let Ok(formatted) = traceback_module.call_method1("format_tb", (tb,)) {
                if let Ok(list) = formatted.extract::<Vec<String>>() {
                    return Some(list.join(""));
                }
            }
        }
    }
    None
}

/// Helper function to convert a standard `PyErr` into a `BrainError::Python`.
fn py_err_to_brain_error(py: Python<'_>, err: PyErr) -> BrainError {
    let traceback = get_traceback_str(py, &err);
    BrainError::Python {
        message: err.to_string(),
        traceback,
    }
}

/// A handle wrapping a Python agent instance and caching its method callables.
pub struct PythonAgentHandle {
    pub instance: Py<PyAny>,
    pub methods: HashMap<String, Py<PyAny>>,
    pub plugin_id: String,
    pub api_version: String,
}

impl PythonAgentHandle {
    /// Creates a new `PythonAgentHandle` by resolving and caching the required method callables.
    pub fn new(
        py: Python<'_>,
        instance: Py<PyAny>,
        required_methods: &[&str],
    ) -> Result<Self, BrainError> {
        let bound_instance = instance.bind(py);

        // Resolve plugin_id and api_version if available on the python instance
        let plugin_id = bound_instance
            .getattr("plugin_id")
            .and_then(|v| v.extract::<String>())
            .unwrap_or_else(|_| "unknown_plugin".to_string());

        let api_version = bound_instance
            .getattr("api_version")
            .and_then(|v| v.extract::<String>())
            .unwrap_or_else(|_| "unknown_version".to_string());

        let mut methods = HashMap::new();
        for &method_name in required_methods {
            match bound_instance.getattr(method_name) {
                Ok(method) => {
                    if method.is_callable() {
                        methods.insert(method_name.to_string(), method.clone().into());
                    } else {
                        return Err(BrainError::Validation {
                            message: format!(
                                "[Plugin: {}] Method '{}' on Python agent is not callable",
                                plugin_id, method_name
                            ),
                        });
                    }
                }
                Err(_) => {
                    return Err(BrainError::Validation {
                        message: format!(
                            "[Plugin: {}] Required method '{}' not found on Python agent",
                            plugin_id, method_name
                        ),
                    });
                }
            }
        }

        Ok(Self {
            instance,
            methods,
            plugin_id,
            api_version,
        })
    }

    /// Invokes a cached method with the given arguments, resolving traceback on exception.
    pub fn call_cached_method(
        &self,
        py: Python<'_>,
        method_name: &str,
        args: impl IntoPy<Py<PyTuple>>,
    ) -> Result<PyObject, BrainError> {
        let method = self
            .methods
            .get(method_name)
            .ok_or_else(|| BrainError::Validation {
                message: format!(
                    "[Plugin: {}] Cached method '{}' not found",
                    self.plugin_id, method_name
                ),
            })?;

        let bound_method = method.bind(py);
        let args_tuple: Py<PyTuple> = args.into_py(py);
        let bound_args = args_tuple.bind(py);

        bound_method
            .call1(bound_args)
            .map(|res| res.unbind())
            .map_err(|err| self.to_brain_error(py, method_name, err))
    }

    fn to_brain_error(&self, py: Python<'_>, method_name: &str, err: PyErr) -> BrainError {
        let traceback = get_traceback_str(py, &err);
        let message = format!(
            "[Plugin: {}] [API: {}] [Callsite: {}] Python execution failed: {}",
            self.plugin_id, self.api_version, method_name, err
        );
        BrainError::Python { message, traceback }
    }
}

/// ChatAgent wrapper delegating to Python implementation.
pub struct PythonChatAgent {
    pub handle: PythonAgentHandle,
    pub name: String,
}

impl PythonChatAgent {
    pub fn new(py: Python<'_>, instance: Py<PyAny>) -> Result<Self, BrainError> {
        let handle = PythonAgentHandle::new(py, instance.clone(), &["chat"])?;

        let bound_instance = instance.bind(py);
        let name = bound_instance
            .getattr("name")
            .and_then(|v| {
                if v.is_callable() {
                    v.call0().and_then(|r| r.extract::<String>())
                } else {
                    v.extract::<String>()
                }
            })
            .unwrap_or_else(|_| "PythonChatAgent".to_string());

        Ok(Self { handle, name })
    }
}

impl ChatAgent for PythonChatAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn chat(&self, session_id: SessionId, prompt: &str) -> Result<String, BrainError> {
        Python::with_gil(|py| {
            let py_sid = Py::new(py, crate::api::PySessionId { inner: session_id })
                .map_err(|e| py_err_to_brain_error(py, e))?;

            let res = self
                .handle
                .call_cached_method(py, "chat", (py_sid, prompt))?;
            let bound_res = res.bind(py);

            let reply = bound_res
                .extract::<String>()
                .map_err(|e| py_err_to_brain_error(py, e))?;

            Ok(reply)
        })
    }
}

/// PlannerAgent wrapper delegating to Python implementation.
pub struct PythonPlannerAgent {
    pub handle: PythonAgentHandle,
    pub name: String,
}

impl PythonPlannerAgent {
    pub fn new(py: Python<'_>, instance: Py<PyAny>) -> Result<Self, BrainError> {
        let handle = PythonAgentHandle::new(py, instance.clone(), &["plan_steps"])?;

        let bound_instance = instance.bind(py);
        let name = bound_instance
            .getattr("name")
            .and_then(|v| {
                if v.is_callable() {
                    v.call0().and_then(|r| r.extract::<String>())
                } else {
                    v.extract::<String>()
                }
            })
            .unwrap_or_else(|_| "PythonPlannerAgent".to_string());

        Ok(Self { handle, name })
    }
}

impl PlannerAgent for PythonPlannerAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn plan_steps(
        &self,
        task_description: &str,
        history: &Conversation,
    ) -> Result<Vec<ToolCall>, BrainError> {
        enum RawToolCall {
            Direct(ToolCall),
            Json(serde_json::Value),
        }

        let raw_items = Python::with_gil(|py| {
            let py_conv = Py::new(
                py,
                crate::api::PyConversation {
                    inner: history.clone(),
                },
            )
            .map_err(|e| py_err_to_brain_error(py, e))?;

            let res =
                self.handle
                    .call_cached_method(py, "plan_steps", (task_description, py_conv))?;
            let bound_res = res.bind(py);

            let bound_list = bound_res
                .downcast::<PyList>()
                .map_err(|e| py_err_to_brain_error(py, e.into()))?;

            let mut raw_items = Vec::new();
            for item in bound_list.iter() {
                if let Ok(py_tc) = item.downcast::<crate::api::PyToolCall>() {
                    let borrow_tc = py_tc.try_borrow().map_err(|e| BrainError::Python {
                        message: format!("Failed to borrow PyToolCall: {}", e),
                        traceback: None,
                    })?;
                    raw_items.push(RawToolCall::Direct(borrow_tc.inner.clone()));
                } else {
                    let json_val = crate::api::py_to_json(py, &item)
                        .map_err(|e| py_err_to_brain_error(py, e))?;
                    raw_items.push(RawToolCall::Json(json_val));
                }
            }

            Ok::<_, BrainError>(raw_items)
        })?;

        let mut tool_calls = Vec::new();
        for raw in raw_items {
            match raw {
                RawToolCall::Direct(tc) => tool_calls.push(tc),
                RawToolCall::Json(json_val) => {
                    let tc: ToolCall =
                        serde_json::from_value(json_val).map_err(|e| BrainError::Validation {
                            message: format!("Invalid ToolCall returned from planner: {}", e),
                        })?;
                    tool_calls.push(tc);
                }
            }
        }

        Ok(tool_calls)
    }
}

/// EmbeddingAgent wrapper delegating to Python implementation.
pub struct PythonEmbeddingAgent {
    pub handle: PythonAgentHandle,
    pub name: String,
    pub dimension: usize,
}

impl PythonEmbeddingAgent {
    pub fn new(py: Python<'_>, instance: Py<PyAny>) -> Result<Self, BrainError> {
        let handle = PythonAgentHandle::new(py, instance.clone(), &["embed_text"])?;

        let bound_instance = instance.bind(py);
        let name = bound_instance
            .getattr("name")
            .and_then(|v| {
                if v.is_callable() {
                    v.call0().and_then(|r| r.extract::<String>())
                } else {
                    v.extract::<String>()
                }
            })
            .unwrap_or_else(|_| "PythonEmbeddingAgent".to_string());

        let dimension = bound_instance
            .getattr("dimension")
            .and_then(|v| {
                if v.is_callable() {
                    v.call0().and_then(|r| r.extract::<usize>())
                } else {
                    v.extract::<usize>()
                }
            })
            .map_err(|e| BrainError::Validation {
                message: format!("Failed to retrieve embedding dimension: {}", e),
            })?;

        Ok(Self {
            handle,
            name,
            dimension,
        })
    }
}

impl EmbeddingAgent for PythonEmbeddingAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        Python::with_gil(|py| {
            let res = self.handle.call_cached_method(py, "embed_text", (text,))?;
            let bound_res = res.bind(py);

            let embedding = bound_res
                .extract::<Vec<f32>>()
                .map_err(|e| py_err_to_brain_error(py, e))?;

            Ok(embedding)
        })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// ExtractionAgent wrapper delegating to Python implementation.
pub struct PythonExtractionAgent {
    pub handle: PythonAgentHandle,
    pub name: String,
}

impl PythonExtractionAgent {
    pub fn new(py: Python<'_>, instance: Py<PyAny>) -> Result<Self, BrainError> {
        let handle = PythonAgentHandle::new(py, instance.clone(), &["extract_graph"])?;

        let bound_instance = instance.bind(py);
        let name = bound_instance
            .getattr("name")
            .and_then(|v| {
                if v.is_callable() {
                    v.call0().and_then(|r| r.extract::<String>())
                } else {
                    v.extract::<String>()
                }
            })
            .unwrap_or_else(|_| "PythonExtractionAgent".to_string());

        Ok(Self { handle, name })
    }
}

impl ExtractionAgent for PythonExtractionAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn extract_graph(&self, text: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        let (raw_nodes, raw_edges) = Python::with_gil(|py| {
            let res = self
                .handle
                .call_cached_method(py, "extract_graph", (text,))?;
            let bound_res = res.bind(py);

            let (py_nodes, py_edges) =
                if let Ok(tuple) = bound_res.downcast::<pyo3::types::PyTuple>() {
                    if tuple.len() != 2 {
                        return Err(BrainError::Python {
                            message: format!(
                                "Expected extract_graph to return a tuple of size 2, got size {}",
                                tuple.len()
                            ),
                            traceback: None,
                        });
                    }
                    (
                        tuple
                            .get_item(0)
                            .map_err(|e| py_err_to_brain_error(py, e))?,
                        tuple
                            .get_item(1)
                            .map_err(|e| py_err_to_brain_error(py, e))?,
                    )
                } else if let Ok(list) = bound_res.downcast::<PyList>() {
                    if list.len() != 2 {
                        return Err(BrainError::Python {
                            message: format!(
                                "Expected extract_graph to return a list of size 2, got size {}",
                                list.len()
                            ),
                            traceback: None,
                        });
                    }
                    (
                        list.get_item(0).map_err(|e| py_err_to_brain_error(py, e))?,
                        list.get_item(1).map_err(|e| py_err_to_brain_error(py, e))?,
                    )
                } else if let Ok(dict) = bound_res.downcast::<PyDict>() {
                    let nodes = dict
                        .get_item("nodes")
                        .map_err(|e| py_err_to_brain_error(py, e))?
                        .ok_or_else(|| BrainError::Python {
                            message: "Expected extract_graph dict to contain 'nodes'".to_string(),
                            traceback: None,
                        })?;
                    let edges = dict
                        .get_item("edges")
                        .map_err(|e| py_err_to_brain_error(py, e))?
                        .ok_or_else(|| BrainError::Python {
                            message: "Expected extract_graph dict to contain 'edges'".to_string(),
                            traceback: None,
                        })?;
                    (nodes, edges)
                } else {
                    return Err(BrainError::Python {
                        message: format!(
                            "Expected extract_graph to return tuple, list, or dict, got: {:?}",
                            bound_res
                        ),
                        traceback: None,
                    });
                };

            // Extract nodes
            let mut raw_nodes = Vec::new();
            let bound_nodes = py_nodes
                .downcast::<PyList>()
                .map_err(|e| py_err_to_brain_error(py, e.into()))?;
            for item in bound_nodes.iter() {
                let json_val =
                    crate::api::py_to_json(py, &item).map_err(|e| py_err_to_brain_error(py, e))?;
                raw_nodes.push(json_val);
            }

            // Extract edges
            let mut raw_edges = Vec::new();
            let bound_edges = py_edges
                .downcast::<PyList>()
                .map_err(|e| py_err_to_brain_error(py, e.into()))?;
            for item in bound_edges.iter() {
                let json_val =
                    crate::api::py_to_json(py, &item).map_err(|e| py_err_to_brain_error(py, e))?;
                raw_edges.push(json_val);
            }

            Ok::<_, BrainError>((raw_nodes, raw_edges))
        })?;

        let mut nodes = Vec::new();
        for json_val in raw_nodes {
            let node: NodeDTO =
                serde_json::from_value(json_val).map_err(|e| BrainError::Validation {
                    message: format!("Invalid NodeDTO: {}", e),
                })?;
            nodes.push(node);
        }

        let mut edges = Vec::new();
        for json_val in raw_edges {
            let edge: EdgeDTO =
                serde_json::from_value(json_val).map_err(|e| BrainError::Validation {
                    message: format!("Invalid EdgeDTO: {}", e),
                })?;
            edges.push(edge);
        }

        Ok((nodes, edges))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_agent_handle_and_wrappers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let mock_agent_code = r#"
class MockAgent:
    def __init__(self):
        self.plugin_id = "test_plugin"
        self.api_version = "1.0.0"
        self.name = "TestAgent"
        self.dimension = 128

    def chat(self, session_id, prompt):
        return f"Chatted: {prompt} for session {session_id}"

    def plan_steps(self, task_description, history):
        return [
            {
                "call_id": "call_1",
                "tool_name": "test_tool",
                "arguments": {"arg1": task_description}
            }
        ]

    def embed_text(self, text):
        return [0.1, 0.2, 0.3]

    def extract_graph(self, text):
        return (
            [{"id": "node_1", "label": "Node 1", "node_type": "concept", "attributes": {}}],
            [{"source": "node_1", "target": "node_2", "relation": "links", "weight": 1.0}]
        )
"#;
            let locals = pyo3::types::PyDict::new_bound(py);
            py.run_bound(mock_agent_code, None, Some(&locals)).unwrap();
            let mock_class = locals.get_item("MockAgent").unwrap().unwrap();
            let instance: Py<PyAny> = mock_class.call0().unwrap().into();

            // Test PythonAgentHandle constructor & required methods caching
            let handle = PythonAgentHandle::new(
                py,
                instance.clone(),
                &["chat", "plan_steps", "embed_text", "extract_graph"],
            )
            .unwrap();
            assert_eq!(handle.plugin_id, "test_plugin");
            assert_eq!(handle.api_version, "1.0.0");

            // Test missing method validation
            let missing_handle_res =
                PythonAgentHandle::new(py, instance.clone(), &["non_existent_method"]);
            assert!(missing_handle_res.is_err());

            // Test PythonChatAgent
            let chat_agent = PythonChatAgent::new(py, instance.clone()).unwrap();
            assert_eq!(chat_agent.name(), "TestAgent");
            let session_id = SessionId::new();
            let chat_res = chat_agent.chat(session_id, "hello").unwrap();
            assert_eq!(
                chat_res,
                format!("Chatted: hello for session {}", session_id)
            );

            // Test PythonPlannerAgent
            let planner_agent = PythonPlannerAgent::new(py, instance.clone()).unwrap();
            let history = Conversation::new_empty();
            let plans = planner_agent.plan_steps("do task", &history).unwrap();
            assert_eq!(plans.len(), 1);
            assert_eq!(plans[0].call_id, "call_1");
            assert_eq!(plans[0].tool_name, "test_tool");
            assert_eq!(
                plans[0].arguments.get("arg1").unwrap().as_str().unwrap(),
                "do task"
            );

            // Test PythonEmbeddingAgent
            let embedding_agent = PythonEmbeddingAgent::new(py, instance.clone()).unwrap();
            assert_eq!(embedding_agent.dimension(), 128);
            let vec = embedding_agent.embed_text("text").unwrap();
            assert_eq!(vec, vec![0.1, 0.2, 0.3]);

            // Test PythonExtractionAgent
            let extraction_agent = PythonExtractionAgent::new(py, instance.clone()).unwrap();
            let (nodes, edges) = extraction_agent.extract_graph("text").unwrap();
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].id, "node_1");
            assert_eq!(nodes[0].label, "Node 1");
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].source, "node_1");
            assert_eq!(edges[0].relation, "links");
        });
    }
}
