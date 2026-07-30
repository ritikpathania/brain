//! Operational diagnostics service assembling system health reports.

use brain_integrations::dto::v1::SystemDiagnosticsReport;

/// Trait provider for storage engine diagnostics metadata.
pub trait StorageMetricsProvider: Send + Sync {
    /// Returns the storage backend identifier (e.g. "sqlite").
    fn storage_backend(&self) -> String;
    /// Returns the health status of SQLite connections.
    fn sqlite_status(&self) -> String;
}

/// Trait provider for worker task and counter metrics.
pub trait WorkerMetricsProvider: Send + Sync {
    /// Total client queries processed.
    fn total_queries(&self) -> u64;
    /// Total client ingests processed.
    fn total_ingests(&self) -> u64;
    /// Count of currently active worker tasks.
    fn active_workers(&self) -> u64;
    /// Engine uptime in seconds.
    fn uptime_secs(&self) -> u64;
}

/// Trait provider for configuration settings.
pub trait ConfigProvider: Send + Sync {
    /// Resolved Unix Domain Socket path.
    fn socket_path(&self) -> String;
}

/// Standalone default configuration provider implementation.
#[derive(Debug, Clone, Default)]
pub struct DefaultConfigProvider;

impl ConfigProvider for DefaultConfigProvider {
    fn socket_path(&self) -> String {
        std::env::var("BRAIN_SOCKET_PATH")
            .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.brain/daemon.sock", h)))
            .unwrap_or_else(|_| "daemon.sock".to_string())
    }
}

/// Service constructing authoritative operational diagnostics reports.
pub struct DiagnosticsService<S, W, C> {
    storage_provider: S,
    worker_provider: W,
    config_provider: C,
}

impl<S: StorageMetricsProvider, W: WorkerMetricsProvider, C: ConfigProvider>
    DiagnosticsService<S, W, C>
{
    /// Creates a new `DiagnosticsService` with injected metric providers.
    pub fn new(storage_provider: S, worker_provider: W, config_provider: C) -> Self {
        Self {
            storage_provider,
            worker_provider,
            config_provider,
        }
    }

    /// Assembles and returns an authoritative `SystemDiagnosticsReport` DTO.
    pub fn generate_report(
        &self,
        health_status: &str,
        app_version: &str,
    ) -> SystemDiagnosticsReport {
        let python_runtime = std::env::var("PYTHONVERSION")
            .ok()
            .or_else(|| Some("3.9".to_string()));

        SystemDiagnosticsReport {
            schema_version: 1,
            status: health_status.to_string(),
            version: app_version.to_string(),
            ipc_protocol_version: "v1".to_string(),
            socket_path: self.config_provider.socket_path(),
            sqlite_status: self.storage_provider.sqlite_status(),
            python_runtime,
            uptime_secs: self.worker_provider.uptime_secs(),
            storage_backend: self.storage_provider.storage_backend(),
            total_queries: self.worker_provider.total_queries(),
            total_ingests: self.worker_provider.total_ingests(),
            active_workers: self.worker_provider.active_workers(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockStorage;
    impl StorageMetricsProvider for MockStorage {
        fn storage_backend(&self) -> String {
            "sqlite_mock".to_string()
        }
        fn sqlite_status(&self) -> String {
            "ok".to_string()
        }
    }

    struct MockWorker;
    impl WorkerMetricsProvider for MockWorker {
        fn total_queries(&self) -> u64 {
            10
        }
        fn total_ingests(&self) -> u64 {
            5
        }
        fn active_workers(&self) -> u64 {
            2
        }
        fn uptime_secs(&self) -> u64 {
            120
        }
    }

    struct MockConfig;
    impl ConfigProvider for MockConfig {
        fn socket_path(&self) -> String {
            "/tmp/mock.sock".to_string()
        }
    }

    #[test]
    fn test_diagnostics_service_report_generation() {
        let service = DiagnosticsService::new(MockStorage, MockWorker, MockConfig);
        let report = service.generate_report("healthy", "0.1.0");

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.status, "healthy");
        assert_eq!(report.version, "0.1.0");
        assert_eq!(report.socket_path, "/tmp/mock.sock");
        assert_eq!(report.storage_backend, "sqlite_mock");
        assert_eq!(report.total_queries, 10);
        assert_eq!(report.total_ingests, 5);
        assert_eq!(report.active_workers, 2);
        assert_eq!(report.uptime_secs, 120);
    }
}
