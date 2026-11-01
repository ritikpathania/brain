use std::str::FromStr;
use std::sync::Arc;

use parking_lot::RwLock;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::IntoPyObjectExt;

use brain_core::extensibility::{ExecutionResult, HostContext};
use brain_domain::{Edge, MessageRole, Node, Session, SessionId, ToolCall};

/// Converts a PyObject representation of JSON-compatible values to a `serde_json::Value`.
pub fn py_to_json(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        Ok(serde_json::Value::Null)
    } else if let Ok(b) = obj.extract::<bool>() {
        Ok(serde_json::Value::Bool(b))
    } else if let Ok(i) = obj.extract::<i64>() {
        Ok(serde_json::Value::Number(i.into()))
    } else if let Ok(f) = obj.extract::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(f) {
            Ok(serde_json::Value::Number(num))
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(
                "Invalid float value",
            ))
        }
    } else if let Ok(s) = obj.extract::<String>() {
        Ok(serde_json::Value::String(s))
    } else if let Ok(dict) = obj.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key_str = k.extract::<String>()?;
            let val_json = py_to_json(_py, &v)?;
            map.insert(key_str, val_json);
        }
        Ok(serde_json::Value::Object(map))
    } else if let Ok(list) = obj.cast::<PyList>() {
        let mut vec = Vec::new();
        for item in list.iter() {
            vec.push(py_to_json(_py, &item)?);
        }
        Ok(serde_json::Value::Array(vec))
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Unsupported type for JSON conversion",
        ))
    }
}

/// Converts a `serde_json::Value` to a Python object representation.
pub fn json_to_py(py: Python<'_>, val: &serde_json::Value) -> PyResult<Py<PyAny>> {
    use serde_json::Value;
    match val {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => b.into_py_any(py),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py_any(py)
            } else if let Some(f) = n.as_f64() {
                f.into_py_any(py)
            } else {
                Err(pyo3::exceptions::PyValueError::new_err("Invalid number"))
            }
        }
        Value::String(s) => s.as_str().into_py_any(py),
        Value::Array(arr) => {
            let py_list = PyList::empty(py);
            for v in arr {
                let py_val = json_to_py(py, v)?;
                py_list.append(py_val)?;
            }
            Ok(py_list.into_any().unbind())
        }
        Value::Object(obj) => {
            let py_dict = PyDict::new(py);
            for (k, v) in obj {
                let py_val = json_to_py(py, v)?;
                py_dict.set_item(k, py_val)?;
            }
            Ok(py_dict.into_any().unbind())
        }
    }
}

/// PyO3 wrapper class for SessionId.
#[pyclass(name = "SessionId")]
#[derive(Clone)]
pub struct PySessionId {
    pub inner: SessionId,
}

impl Default for PySessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PySessionId {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: SessionId::new(),
        }
    }

    #[staticmethod]
    pub fn from_string(s: &str) -> PyResult<Self> {
        let inner = SessionId::from_str(s)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    pub fn __str__(&self) -> String {
        self.inner.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("SessionId('{}')", self.inner)
    }
}

/// PyO3 wrapper class for NodeId.
#[pyclass(name = "NodeId")]
#[derive(Clone)]
pub struct PyNodeId {
    pub inner: brain_domain::NodeId,
}

impl Default for PyNodeId {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl PyNodeId {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: brain_domain::NodeId::new(),
        }
    }

    #[staticmethod]
    pub fn from_string(s: &str) -> PyResult<Self> {
        let inner = brain_domain::NodeId::from_str(s)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    pub fn __str__(&self) -> String {
        self.inner.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("NodeId('{}')", self.inner)
    }
}

/// PyO3 wrapper class for MemoryNode (read-only model).
#[pyclass(name = "MemoryNode")]
#[derive(Clone)]
pub struct PyMemoryNode {
    pub inner: Node,
}

#[pymethods]
impl PyMemoryNode {
    #[getter]
    pub fn id(&self) -> PyNodeId {
        PyNodeId {
            inner: self.inner.id,
        }
    }

    #[getter]
    pub fn label(&self) -> String {
        self.inner.label.clone()
    }

    #[getter]
    pub fn node_type(&self) -> String {
        self.inner.node_type.to_string()
    }

    #[getter]
    pub fn properties(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let map_val =
            serde_json::Value::Object(self.inner.properties.clone().into_iter().collect());
        json_to_py(py, &map_val)
    }

    #[getter]
    pub fn updated_at(&self) -> u64 {
        self.inner.updated_at
    }

    pub fn __repr__(&self) -> String {
        format!(
            "MemoryNode(id='{}', label='{}', type='{}')",
            self.inner.id,
            self.inner.label,
            self.node_type()
        )
    }
}

/// PyO3 wrapper class for Edge (read-only model).
#[pyclass(name = "Edge")]
#[derive(Clone)]
pub struct PyEdge {
    pub inner: Edge,
}

#[pymethods]
impl PyEdge {
    #[getter]
    pub fn source(&self) -> PyNodeId {
        PyNodeId {
            inner: self.inner.source,
        }
    }

    #[getter]
    pub fn target(&self) -> PyNodeId {
        PyNodeId {
            inner: self.inner.target,
        }
    }

    #[getter]
    pub fn relation(&self) -> String {
        self.inner.relation.to_string()
    }

    #[getter]
    pub fn weight(&self) -> f64 {
        self.inner.weight
    }

    #[getter]
    pub fn updated_at(&self) -> u64 {
        self.inner.updated_at
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Edge(source='{}', target='{}', relation='{}', weight={})",
            self.inner.source, self.inner.target, self.inner.relation, self.inner.weight
        )
    }
}

/// PyO3 wrapper class for Conversation (read-only model).
#[pyclass(name = "Conversation")]
#[derive(Clone)]
pub struct PyConversation {
    pub inner: Session,
}

#[pymethods]
impl PyConversation {
    #[getter]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }

    #[getter]
    pub fn messages(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let py_list = PyList::empty(py);
        for msg in &self.inner.messages {
            let py_dict = PyDict::new(py);
            py_dict.set_item("id", msg.id.to_string())?;
            py_dict.set_item(
                "role",
                match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                    MessageRole::Tool => "tool",
                    MessageRole::Thinking => "thinking",
                },
            )?;
            py_dict.set_item("content", &msg.content)?;
            py_dict.set_item("timestamp", msg.timestamp)?;
            py_list.append(py_dict)?;
        }
        Ok(py_list.into_any().unbind())
    }

    #[getter]
    pub fn metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let py_dict = PyDict::new(py);
        py_dict.set_item("title", &self.inner.title.0)?;
        py_dict.set_item("archived", self.inner.archived)?;
        py_dict.set_item("pinned", self.inner.pinned)?;
        py_dict.set_item("updated_at", self.inner.updated_at.0)?;
        Ok(py_dict.into_any().unbind())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Conversation(id='{}', messages_count={})",
            self.inner.id,
            self.inner.messages.len()
        )
    }
}

/// PyO3 wrapper class for ToolCall (read-only model).
#[pyclass(name = "ToolCall")]
#[derive(Clone)]
pub struct PyToolCall {
    pub inner: ToolCall,
}

#[pymethods]
impl PyToolCall {
    #[getter]
    pub fn call_id(&self) -> String {
        self.inner.call_id.clone()
    }

    #[getter]
    pub fn tool_name(&self) -> String {
        self.inner.tool_name.clone()
    }

    #[getter]
    pub fn arguments(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let map_val = serde_json::Value::Object(self.inner.arguments.clone().into_iter().collect());
        json_to_py(py, &map_val)
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ToolCall(call_id='{}', tool_name='{}')",
            self.inner.call_id, self.inner.tool_name
        )
    }
}

/// PyO3 wrapper class for ExecutionResult (read-only model).
#[pyclass(name = "ExecutionResult")]
#[derive(Clone)]
pub struct PyExecutionResult {
    pub inner: ExecutionResult,
}

#[pymethods]
impl PyExecutionResult {
    #[getter]
    pub fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        json_to_py(py, self.inner.value())
    }

    pub fn __repr__(&self) -> String {
        format!("ExecutionResult(value={:?})", self.inner.value())
    }
}

/// PyO3 capability object: RuntimeContext.
/// Exposes Python-native helper methods to query memory and run tools,
/// sandboxing Python scripts completely from Rust connection details.
///
/// The context owns a reference-counted host handle rather than borrowing a
/// host from the lifecycle callback. Python may retain or use this object from
/// another Python thread while it is active. Once the lifecycle hook settles,
/// every retained copy observes the same closed bridge.
#[pyclass(name = "RuntimeContext")]
#[derive(Clone)]
pub struct PyRuntimeContext {
    bridge: Arc<RuntimeContextBridge>,
    session_id: SessionId,
}

struct RuntimeContextBridge {
    /// `None` is the settled state. An operation clones the owned `Arc` while
    /// holding the read lock, then may run without the lock or the GIL.
    host: RwLock<Option<Arc<dyn HostContext>>>,
}

impl RuntimeContextBridge {
    fn new(host: Arc<dyn HostContext>) -> Self {
        Self {
            host: RwLock::new(Some(host)),
        }
    }

    fn active_host(&self) -> PyResult<Arc<dyn HostContext>> {
        self.host
            .read()
            .clone()
            .ok_or_else(runtime_context_closed_error)
    }

    fn close(&self) {
        self.host.write().take();
    }
}

fn runtime_context_closed_error() -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err("RuntimeContext is closed")
}

impl PyRuntimeContext {
    /// Creates an active context backed by an owned, thread-safe host handle.
    pub fn new(host: Arc<dyn HostContext>, session_id: SessionId) -> Self {
        Self {
            bridge: Arc::new(RuntimeContextBridge::new(host)),
            session_id,
        }
    }

    /// Settles this lifecycle context for every Python reference to its bridge.
    /// In-flight operations retain their own `Arc` and complete safely.
    pub fn close(&self) {
        self.bridge.close();
    }
}

#[pymethods]
impl PyRuntimeContext {
    pub fn retrieve(
        &self,
        py: Python<'_>,
        query: String,
        limit: usize,
    ) -> PyResult<Vec<PyMemoryNode>> {
        let host = self.bridge.active_host()?;
        let session_id = self.session_id;

        let nodes = py
            .detach(move || host.retrieve(&session_id, &query, limit))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(nodes
            .into_iter()
            .map(|n| PyMemoryNode { inner: n })
            .collect())
    }

    pub fn execute_tool(
        &self,
        py: Python<'_>,
        tool_name: String,
        arguments: &Bound<'_, PyDict>,
    ) -> PyResult<PyExecutionResult> {
        let host = self.bridge.active_host()?;
        let session_id = self.session_id;

        let json_args = py_to_json(py, arguments)?;
        let map_args = match json_args {
            serde_json::Value::Object(map) => map.into_iter().collect(),
            _ => {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "Arguments must be a dictionary",
                ))
            }
        };

        let res = py
            .detach(move || host.execute_tool(&session_id, &tool_name, &map_args))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyExecutionResult { inner: res })
    }
}
