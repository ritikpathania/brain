use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use brain_config::schema::BrainSettings;
use brain_core::errors::BrainError;
use brain_core::extensibility::{ExecutionContext, ExecutionResult, HostContext, ToolRegistry};
use brain_core::services::RetrievalService;
use brain_domain::{Node, SessionId};
use brain_plugins::{LoaderKind, PluginManager, PluginScanner};
use brain_session::SessionCacheManager;
use brain_storage::SqliteStorage;
use brain_tools::{BlockingToolRunner, CancellationTokenImpl, ToolExecutor, ToolRegistryImpl};

/// Represents the current execution state of the runtime lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Runtime has been built but not yet started.
    Created,
    /// Runtime is executing its startup phases.
    Starting,
    /// Runtime is active and fully ready to process work.
    Running,
    /// Runtime is actively shutting down.
    Stopping,
    /// Runtime has completed shutdown.
    Stopped,
}

/// Diagnostic health status value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Subsystem is fully operational.
    Healthy,
    /// Subsystem is operational but has degraded metrics.
    Degraded,
    /// Subsystem is offline or failing queries.
    Unhealthy,
}

/// Rich structured health report for components and overall runtime health.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthReport {
    /// Aggregated overall health status of the runtime.
    pub overall: HealthStatus,
    /// Component-by-component health statuses.
    pub components: BTreeMap<String, HealthStatus>,
}

/// Immutable diagnostics record returned upon successful startup.
#[derive(Debug, Clone)]
pub struct StartupReport {
    completed_phases: Vec<String>,
    failed_phase: Option<String>,
    duration: Duration,
}

impl StartupReport {
    /// Returns the list of phase names that completed successfully.
    pub fn completed_phases(&self) -> &[String] {
        &self.completed_phases
    }

    /// Returns the phase name that caused a startup failure, if any.
    pub fn failed_phase(&self) -> Option<&str> {
        self.failed_phase.as_deref()
    }

    /// Returns the total duration of the startup transition.
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

/// Lifecycle callback listener hook.
pub trait RuntimeObserver: Send + Sync {
    /// Called when the runtime transitions into the Running state.
    fn on_started(&self, runtime: &ApplicationRuntime);
    /// Called when the runtime starts shutting down.
    fn on_stopping(&self, runtime: &ApplicationRuntime);
    /// Called when the runtime completes its shutdown sequence.
    fn on_stopped(&self, runtime: &ApplicationRuntime);
}

/// Subsystem health check querying capability.
pub trait HealthCheck: Send + Sync {
    /// Evaluates the diagnostic status of the component.
    fn health(&self) -> HealthStatus;
}

impl HealthCheck for SqliteStorage {
    fn health(&self) -> HealthStatus {
        match self.pool().get() {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }
}

impl HealthCheck for PluginManager {
    fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

// Composable StartupPhase trait (internal to crate)
pub(crate) trait StartupPhase {
    fn name(&self) -> &'static str;
    fn execute(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError>;
    fn rollback(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError>;
}

// Service Locator private to crate
pub(crate) struct RuntimeServiceLocator {
    pub storage: Option<Arc<SqliteStorage>>,
    pub session_manager: Option<Arc<SessionCacheManager>>,
    pub query_embedding_service: Option<Arc<dyn brain_core::retrieval::QueryEmbeddingService>>,
    pub retrieval_service: Option<Arc<crate::retrieval::RetrievalServiceImpl>>,
    pub conversation_manager: Option<Arc<dyn crate::conversation::ConversationManager>>,
    pub tool_registry: Option<Arc<ToolRegistryImpl>>,
    pub tool_executor: Option<Arc<ToolExecutor>>,
    pub plugin_manager: Option<Arc<PluginManager>>,
    pub streaming_runtime: Option<Arc<crate::agent::streaming::StreamingRuntime>>,
}

/// Unified composition root and lifecycle owner of the Brain Relational Engine.
pub struct ApplicationRuntime {
    config: BrainSettings,
    state: RwLock<RuntimeState>,
    service_locator: RwLock<RuntimeServiceLocator>,
    observers: RwLock<Vec<Arc<dyn RuntimeObserver>>>,
}

impl HostContext for ApplicationRuntime {
    fn retrieve(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Node>, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "HostContext query is invalid because ApplicationRuntime is not Running"
                    .to_string(),
            });
        }
        let locator = self.service_locator.read();
        let retrieval_service =
            locator
                .retrieval_service
                .clone()
                .ok_or_else(|| BrainError::Validation {
                    message: "Retrieval service not initialized".to_string(),
                })?;

        let request = brain_core::retrieval::RetrievalRequest {
            session_id: *session_id,
            query: query.to_string(),
            limit,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
        };
        let response = retrieval_service.execute_pipeline(&request)?;
        Ok(response.nodes)
    }

    fn execute_tool(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "HostContext tool execution is invalid because ApplicationRuntime is not Running".to_string(),
            });
        }
        let locator = self.service_locator.read();
        let tool_executor =
            locator
                .tool_executor
                .clone()
                .ok_or_else(|| BrainError::Validation {
                    message: "Tool executor not initialized".to_string(),
                })?;
        let tool_registry =
            locator
                .tool_registry
                .clone()
                .ok_or_else(|| BrainError::Validation {
                    message: "Tool registry not initialized".to_string(),
                })?;

        let tool = tool_registry
            .get_tool(tool_name)
            .ok_or_else(|| BrainError::Validation {
                message: format!("Tool '{}' not found", tool_name),
            })?;

        let context = ExecutionContext {
            session_id: *session_id,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            cancellation: Arc::new(CancellationTokenImpl::new()),
            deadline: None,
        };
        let permission_manager = brain_tools::PermissionManager::new();
        for perm in &tool.metadata().required_permissions {
            permission_manager.grant(*perm);
        }

        let handle = tokio::runtime::Handle::try_current().map_err(|e| BrainError::Tool {
            tool_name: tool_name.to_string(),
            message: format!("Failed to get tokio handle: {}", e),
        })?;

        tokio::task::block_in_place(|| {
            handle.block_on(async {
                tool_executor
                    .execute(tool, &context, &permission_manager, arguments)
                    .await
            })
        })
    }
}

impl ApplicationRuntime {
    /// Returns a new RuntimeBuilder to configure the application runtime.
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Returns the resolved immutable settings for this runtime.
    pub fn config(&self) -> &BrainSettings {
        &self.config
    }

    /// Returns the active lifecycle state of the runtime.
    pub fn state(&self) -> RuntimeState {
        *self.state.read()
    }

    /// Returns whether the runtime is fully active and ready to accept queries.
    pub fn is_ready(&self) -> bool {
        self.state() == RuntimeState::Running
    }

    /// Exposes the composite RetrievalService facade.
    pub fn retrieval(&self) -> Result<Arc<dyn RetrievalService>, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "Runtime is not in Running state".to_string(),
            });
        }
        let locator = self.service_locator.read();
        locator
            .retrieval_service
            .clone()
            .map(|s| s as Arc<dyn RetrievalService>)
            .ok_or_else(|| BrainError::Validation {
                message: "Retrieval service not initialized".to_string(),
            })
    }

    /// Exposes the StreamingRuntime observer layer.
    pub fn streaming(&self) -> Result<Arc<crate::agent::streaming::StreamingRuntime>, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "Runtime is not in Running state".to_string(),
            });
        }
        let locator = self.service_locator.read();
        locator
            .streaming_runtime
            .clone()
            .ok_or_else(|| BrainError::Validation {
                message: "Streaming runtime not initialized".to_string(),
            })
    }

    /// Exposes the dynamic PluginManager.
    pub fn plugins(&self) -> Result<Arc<PluginManager>, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "Runtime is not in Running state".to_string(),
            });
        }
        let locator = self.service_locator.read();
        locator
            .plugin_manager
            .clone()
            .ok_or_else(|| BrainError::Validation {
                message: "Plugin manager not initialized".to_string(),
            })
    }

    /// Exposes the ToolExecutor.
    pub fn tools(&self) -> Result<Arc<ToolExecutor>, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "Runtime is not in Running state".to_string(),
            });
        }
        let locator = self.service_locator.read();
        locator
            .tool_executor
            .clone()
            .ok_or_else(|| BrainError::Validation {
                message: "Tool executor not initialized".to_string(),
            })
    }

    /// Exposes the SQLite storage engine repository set.
    pub fn storage(&self) -> Result<Arc<SqliteStorage>, BrainError> {
        if self.state() != RuntimeState::Running {
            return Err(BrainError::InvalidTransition {
                message: "Runtime is not in Running state".to_string(),
            });
        }
        let locator = self.service_locator.read();
        locator
            .storage
            .clone()
            .ok_or_else(|| BrainError::Validation {
                message: "Storage not initialized".to_string(),
            })
    }

    /// Aggregates active status across components and returns a diagnostic health report.
    pub fn health(&self) -> HealthReport {
        let mut components = BTreeMap::new();
        let state = self.state();

        if state != RuntimeState::Running {
            components.insert("OverallState".to_string(), HealthStatus::Unhealthy);
            return HealthReport {
                overall: HealthStatus::Unhealthy,
                components,
            };
        }

        let locator = self.service_locator.read();

        // 1. Storage Health
        let storage_status = if let Some(ref storage) = locator.storage {
            storage.health()
        } else {
            HealthStatus::Unhealthy
        };
        components.insert("Storage".to_string(), storage_status);

        // 2. Python Health
        let python_status = pyo3::Python::with_gil(|_py| HealthStatus::Healthy);
        components.insert("Python".to_string(), python_status);

        // 3. Plugins Health
        let plugins_status = if locator.plugin_manager.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        };
        components.insert("Plugins".to_string(), plugins_status);

        // 4. Tools Health
        let tools_status = if locator.tool_executor.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        };
        components.insert("Tools".to_string(), tools_status);

        let mut overall = HealthStatus::Healthy;
        for status in components.values() {
            if *status == HealthStatus::Unhealthy {
                overall = HealthStatus::Unhealthy;
                break;
            } else if *status == HealthStatus::Degraded && overall == HealthStatus::Healthy {
                overall = HealthStatus::Degraded;
            }
        }

        HealthReport {
            overall,
            components,
        }
    }

    /// Starts all subsystems in sequential order, performing rollback on failures.
    pub fn start(&self) -> Result<StartupReport, BrainError> {
        let mut state_lock = self.state.write();
        if *state_lock != RuntimeState::Created {
            return Err(BrainError::InvalidTransition {
                message: format!("Cannot start runtime from state {:?}", *state_lock),
            });
        }
        *state_lock = RuntimeState::Starting;
        drop(state_lock);

        let phases: Vec<Box<dyn StartupPhase>> = vec![
            Box::new(ConfigValidationPhase),
            Box::new(StorageMigrationPhase),
            Box::new(PythonInvariantPhase),
            Box::new(ToolRegistryPhase),
            Box::new(PluginScanningPhase),
            Box::new(ServicesReadyPhase),
        ];

        let start_time = Instant::now();
        let mut completed = Vec::new();
        let mut failed_phase = None;
        let mut startup_err = None;

        for phase in phases {
            let name = phase.name();
            match phase.execute(self) {
                Ok(()) => {
                    completed.push(phase);
                }
                Err(e) => {
                    failed_phase = Some(name.to_string());
                    startup_err = Some(e);
                    break;
                }
            }
        }

        let duration = start_time.elapsed();

        if let Some(err) = startup_err {
            for phase in completed.into_iter().rev() {
                if let Err(rollback_err) = phase.rollback(self) {
                    tracing::warn!(
                        "Failed to rollback phase '{}' during startup failure recovery: {:?}",
                        phase.name(),
                        rollback_err
                    );
                }
            }
            *self.state.write() = RuntimeState::Created;
            return Err(err);
        }

        *self.state.write() = RuntimeState::Running;

        self.notify_started();

        let completed_names = completed.iter().map(|p| p.name().to_string()).collect();
        Ok(StartupReport {
            completed_phases: completed_names,
            failed_phase,
            duration,
        })
    }

    /// Gracefully tears down all components in reverse order.
    pub fn shutdown(&self) -> Result<(), BrainError> {
        let mut state_lock = self.state.write();
        if *state_lock == RuntimeState::Stopped || *state_lock == RuntimeState::Stopping {
            return Ok(());
        }
        *state_lock = RuntimeState::Stopping;
        drop(state_lock);

        self.notify_stopping();

        let mut locator = self.service_locator.write();
        if let Some(ref plugin_manager) = locator.plugin_manager {
            let active_plugins = plugin_manager.list();
            for plugin in active_plugins {
                if let Err(e) = plugin_manager.unload(&plugin.id) {
                    tracing::warn!(
                        "Failed to unload plugin '{}' during shutdown: {:?}",
                        plugin.id,
                        e
                    );
                }
            }
        }

        locator.storage = None;
        locator.session_manager = None;
        locator.retrieval_service = None;
        locator.tool_registry = None;
        locator.tool_executor = None;
        locator.plugin_manager = None;

        *self.state.write() = RuntimeState::Stopped;

        self.notify_stopped();

        Ok(())
    }

    fn notify_started(&self) {
        let observers = self.observers.read();
        for observer in observers.iter() {
            let observer = observer.clone();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                observer.on_started(self);
            }));
            if let Err(e) = res {
                tracing::warn!("RuntimeObserver::on_started panicked: {:?}", e);
            }
        }
    }

    fn notify_stopping(&self) {
        let observers = self.observers.read();
        for observer in observers.iter() {
            let observer = observer.clone();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                observer.on_stopping(self);
            }));
            if let Err(e) = res {
                tracing::warn!("RuntimeObserver::on_stopping panicked: {:?}", e);
            }
        }
    }

    fn notify_stopped(&self) {
        let observers = self.observers.read();
        for observer in observers.iter() {
            let observer = observer.clone();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                observer.on_stopped(self);
            }));
            if let Err(e) = res {
                tracing::warn!("RuntimeObserver::on_stopped panicked: {:?}", e);
            }
        }
    }
}

/// Builder helper to construct an ApplicationRuntime instance.
pub struct RuntimeBuilder {
    config: Option<BrainSettings>,
    storage: Option<Arc<SqliteStorage>>,
    session_manager: Option<Arc<SessionCacheManager>>,
    query_embedding_service: Option<Arc<dyn brain_core::retrieval::QueryEmbeddingService>>,
    conversation_manager: Option<Arc<dyn crate::conversation::ConversationManager>>,
    tool_executor: Option<Arc<ToolExecutor>>,
    plugin_manager: Option<Arc<PluginManager>>,
    observers: Vec<Arc<dyn RuntimeObserver>>,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBuilder {
    /// Creates a empty, default builder.
    pub fn new() -> Self {
        Self {
            config: None,
            storage: None,
            session_manager: None,
            query_embedding_service: None,
            conversation_manager: None,
            tool_executor: None,
            plugin_manager: None,
            observers: Vec::new(),
        }
    }

    /// Configures the settings for the runtime.
    pub fn with_config(mut self, config: BrainSettings) -> Self {
        self.config = Some(config);
        self
    }

    /// Pre-injects a mock or custom database storage engine.
    pub fn with_storage(mut self, storage: Arc<SqliteStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Pre-injects a mock or custom session cache manager.
    pub fn with_session_manager(mut self, manager: Arc<SessionCacheManager>) -> Self {
        self.session_manager = Some(manager);
        self
    }

    /// Pre-injects a custom conversation manager.
    pub fn with_conversation_manager(
        mut self,
        manager: Arc<dyn crate::conversation::ConversationManager>,
    ) -> Self {
        self.conversation_manager = Some(manager);
        self
    }

    /// Pre-injects a mock or custom plugin manager.
    pub fn with_plugin_manager(mut self, manager: Arc<PluginManager>) -> Self {
        self.plugin_manager = Some(manager);
        self
    }

    /// Pre-injects a mock or custom tool execution engine.
    pub fn with_tool_executor(mut self, executor: Arc<ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// Registers a custom lifecycle callback observer.
    pub fn register_observer(mut self, observer: Arc<dyn RuntimeObserver>) -> Self {
        self.observers.push(observer);
        self
    }

    /// Configures a custom QueryEmbeddingService.
    pub fn with_query_embedding_service(
        mut self,
        service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    ) -> Self {
        self.query_embedding_service = Some(service);
        self
    }

    /// Builds the `ApplicationRuntime` instance without performing side effects.
    pub fn build(self) -> Result<ApplicationRuntime, BrainError> {
        let config = match self.config {
            Some(c) => c,
            None => {
                let defaults_src = brain_config::loader::DefaultsSource;
                brain_config::loader::resolve(&[Box::new(defaults_src)])?
            }
        };

        let service_locator = RuntimeServiceLocator {
            storage: self.storage,
            session_manager: self.session_manager,
            query_embedding_service: self.query_embedding_service,
            retrieval_service: None,
            conversation_manager: self.conversation_manager,
            tool_registry: None,
            tool_executor: self.tool_executor,
            plugin_manager: self.plugin_manager,
            streaming_runtime: None,
        };

        Ok(ApplicationRuntime {
            config,
            state: RwLock::new(RuntimeState::Created),
            service_locator: RwLock::new(service_locator),
            observers: RwLock::new(self.observers),
        })
    }
}

// Concrete startup phase implementations
struct ConfigValidationPhase;
impl StartupPhase for ConfigValidationPhase {
    fn name(&self) -> &'static str {
        "Config Validation"
    }
    fn execute(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        brain_config::validation::validate(&runtime.config)
    }
    fn rollback(&self, _runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        Ok(())
    }
}

struct StorageMigrationPhase;
impl StartupPhase for StorageMigrationPhase {
    fn name(&self) -> &'static str {
        "Storage Migration"
    }
    fn execute(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        let mut locator = runtime.service_locator.write();
        if locator.storage.is_some() {
            return Ok(());
        }
        let db_settings = runtime.config.database();
        let storage = SqliteStorage::new(
            db_settings.path(),
            db_settings.pool_size(),
            db_settings.enable_wal(),
        )?;
        locator.storage = Some(Arc::new(storage));
        Ok(())
    }
    fn rollback(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        runtime.service_locator.write().storage = None;
        Ok(())
    }
}

struct PythonInvariantPhase;
impl StartupPhase for PythonInvariantPhase {
    fn name(&self) -> &'static str {
        "Python Invariant"
    }
    fn execute(&self, _runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        pyo3::prepare_freethreaded_python();
        Ok(())
    }
    fn rollback(&self, _runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        Ok(())
    }
}

struct ToolRegistryPhase;
impl StartupPhase for ToolRegistryPhase {
    fn name(&self) -> &'static str {
        "Tool Registry"
    }
    fn execute(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        let mut locator = runtime.service_locator.write();
        if locator.tool_executor.is_some() {
            return Ok(());
        }
        let registry = Arc::new(ToolRegistryImpl::new());
        let executor = Arc::new(ToolExecutor::new(Arc::new(BlockingToolRunner)));
        locator.tool_registry = Some(registry);
        locator.tool_executor = Some(executor);
        Ok(())
    }
    fn rollback(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        let mut locator = runtime.service_locator.write();
        locator.tool_registry = None;
        locator.tool_executor = None;
        Ok(())
    }
}

struct PluginScanningPhase;
impl StartupPhase for PluginScanningPhase {
    fn name(&self) -> &'static str {
        "Plugin Scanning"
    }
    fn execute(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        let mut locator = runtime.service_locator.write();
        if locator.plugin_manager.is_some() {
            return Ok(());
        }
        let mut loaders: HashMap<LoaderKind, Box<dyn brain_plugins::PluginLoader>> = HashMap::new();
        loaders.insert(
            LoaderKind::Python,
            Box::new(brain_python::loader::PythonPluginLoader)
                as Box<dyn brain_plugins::PluginLoader>,
        );
        let manager = PluginManager::new(loaders);
        let scanner = PluginScanner::new(vec![Box::new(brain_python::loader::PythonPluginLoader)
            as Box<dyn brain_plugins::PluginLoader>]);
        let path = Path::new(runtime.config.plugins_directory());
        let installed = if path.exists() {
            scanner.scan_directory(path)?
        } else {
            Vec::new()
        };
        for plugin in installed {
            manager.register(plugin)?;
        }
        locator.plugin_manager = Some(Arc::new(manager));
        Ok(())
    }
    fn rollback(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        runtime.service_locator.write().plugin_manager = None;
        Ok(())
    }
}

struct ServicesReadyPhase;
impl StartupPhase for ServicesReadyPhase {
    fn name(&self) -> &'static str {
        "Services Ready"
    }
    fn execute(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        let mut locator = runtime.service_locator.write();
        if locator.session_manager.is_some()
            && locator.retrieval_service.is_some()
            && locator.conversation_manager.is_some()
            && locator.streaming_runtime.is_some()
        {
            return Ok(());
        }
        let storage = locator
            .storage
            .clone()
            .ok_or_else(|| BrainError::Validation {
                message: "Storage dependency missing for ServicesReadyPhase".to_string(),
            })?;
        let session_manager = Arc::new(SessionCacheManager::new());
        let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
        let query_embedding_service =
            locator.query_embedding_service.clone().unwrap_or_else(|| {
                let default_provider = Arc::new(brain_core::retrieval::NoopEmbeddingProvider);
                Arc::new(brain_core::retrieval::DefaultQueryEmbeddingService::new(
                    default_provider,
                ))
            });
        let retrieval_service = Arc::new(crate::retrieval::RetrievalServiceImpl::new_with_config(
            storage.clone(),
            &runtime.config,
            session_manager.clone(),
            registry.clone(),
            query_embedding_service,
        ));
        let conversation_manager: Arc<dyn crate::conversation::ConversationManager> =
            Arc::new(crate::conversation::ConversationManagerImpl::new(
                storage.clone(),
                storage.clone(),
                session_manager.clone(),
                Arc::new(crate::conversation::WordSpaceTokenCounter),
                Arc::new(crate::conversation::DummyMemoryExtractor),
                Arc::new(crate::conversation::PromotionEngineImpl::new(
                    crate::conversation::CountThresholdPromotionPolicy::new(5),
                )),
                Arc::new(crate::conversation::CountThresholdSummaryPolicy::new(10)),
                Arc::new(crate::conversation::SqliteCheckpointStore::new(
                    storage.clone(),
                )),
                retrieval_service.clone(),
                Arc::new(crate::conversation::DummyChatAgent),
                None,
                registry,
            ));
        let streaming_runtime = Arc::new(crate::agent::streaming::StreamingRuntime::new(Arc::new(
            crate::agent::streaming::DefaultStreamEventMapper,
        )));
        locator.session_manager = Some(session_manager);
        locator.retrieval_service = Some(retrieval_service);
        locator.conversation_manager = Some(conversation_manager);
        locator.streaming_runtime = Some(streaming_runtime);
        Ok(())
    }
    fn rollback(&self, runtime: &ApplicationRuntime) -> Result<(), BrainError> {
        let mut locator = runtime.service_locator.write();
        locator.session_manager = None;
        locator.retrieval_service = None;
        locator.conversation_manager = None;
        locator.streaming_runtime = None;
        Ok(())
    }
}
