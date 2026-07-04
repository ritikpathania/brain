pub mod config;
pub mod plugins;
pub mod projection;
pub mod retrieval;
pub mod server;
pub mod stm;
pub mod storage;
pub mod telemetry;
pub mod workers;

use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type GlobalState = Arc<RwLock<HashMap<String, stm::SessionContext>>>;

// Global request correlation counter
pub static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

pub use telemetry::DaemonMetrics;

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn daemon_bridge(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    Ok(())
}
