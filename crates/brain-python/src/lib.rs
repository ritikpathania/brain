pub mod api;
pub mod loader;
pub mod runtime;

use pyo3::prelude::*;

#[pymodule]
fn brain_ai(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let api_mod = PyModule::new_bound(py, "api")?;
    let v1_mod = PyModule::new_bound(py, "v1")?;

    v1_mod.add_class::<api::PySessionId>()?;
    v1_mod.add_class::<api::PyNodeId>()?;
    v1_mod.add_class::<api::PyMemoryNode>()?;
    v1_mod.add_class::<api::PyEdge>()?;
    v1_mod.add_class::<api::PyConversation>()?;
    v1_mod.add_class::<api::PyToolCall>()?;
    v1_mod.add_class::<api::PyExecutionResult>()?;
    v1_mod.add_class::<api::PyRuntimeContext>()?;

    api_mod.add_submodule(&v1_mod)?;
    m.add_submodule(&api_mod)?;

    Ok(())
}
