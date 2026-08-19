pub mod config;
pub mod host;
pub mod server;
pub mod telemetry;
pub mod transport;

use std::sync::atomic::AtomicU64;

// Global request correlation counter
pub static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

pub use host::DaemonHost;
pub use telemetry::DaemonMetrics;
