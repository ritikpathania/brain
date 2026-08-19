use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{BrainSettings, PartialBrainSettings, PartialDatabaseSettings};
use brain_core::extensibility::HostContext;
use brain_services::runtime::{ApplicationRuntime, HealthStatus, RuntimeObserver, RuntimeState};

struct TestObserver {
    pub started_count: Arc<AtomicUsize>,
    pub stopping_count: Arc<AtomicUsize>,
    pub stopped_count: Arc<AtomicUsize>,
    pub should_panic: bool,
}

impl RuntimeObserver for TestObserver {
    fn on_started(&self, _runtime: &ApplicationRuntime) {
        if self.should_panic {
            panic!("Mock observer panic");
        }
        self.started_count.fetch_add(1, Ordering::SeqCst);
    }

    fn on_stopping(&self, _runtime: &ApplicationRuntime) {
        if self.should_panic {
            panic!("Mock observer panic");
        }
        self.stopping_count.fetch_add(1, Ordering::SeqCst);
    }

    fn on_stopped(&self, _runtime: &ApplicationRuntime) {
        if self.should_panic {
            panic!("Mock observer panic");
        }
        self.stopped_count.fetch_add(1, Ordering::SeqCst);
    }
}

fn get_temp_db_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    std::env::temp_dir()
        .join(format!("brain_test_{}.db", uuid_str))
        .to_string_lossy()
        .to_string()
}

fn get_temp_plugins_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = std::env::temp_dir().join(format!("brain_plugins_{}", uuid_str));
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().to_string()
}

fn create_valid_test_config(db_path: &str, plugins_path: &str) -> BrainSettings {
    let defaults_src = DefaultsSource;

    let partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some(db_path.to_string()),
            pool_size: Some(2),
            enable_wal: Some(false),
        }),
        plugins_directory: Some(plugins_path.to_string()),
        ..Default::default()
    };

    let override_src = OverrideSource::new(partial);
    resolve(&[Box::new(defaults_src), Box::new(override_src)]).unwrap()
}

#[tokio::test]
async fn test_runtime_builder_defaults() {
    brain_python::initialize_python_runtime();
    let runtime = ApplicationRuntime::builder().build().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Created);
    assert!(!runtime.is_ready());
}

#[tokio::test]
async fn test_runtime_successful_startup_lifecycle_and_shutdown() {
    brain_python::initialize_python_runtime();

    let db_path = get_temp_db_path();
    let plugins_path = get_temp_plugins_path();

    let config = create_valid_test_config(&db_path, &plugins_path);

    let started = Arc::new(AtomicUsize::new(0));
    let stopping = Arc::new(AtomicUsize::new(0));
    let stopped = Arc::new(AtomicUsize::new(0));

    let observer = Arc::new(TestObserver {
        started_count: started.clone(),
        stopping_count: stopping.clone(),
        stopped_count: stopped.clone(),
        should_panic: false,
    });

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .register_observer(observer)
        .build()
        .unwrap();

    assert_eq!(runtime.state(), RuntimeState::Created);

    // 1. Start runtime
    let report = runtime.start().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Running);
    assert!(runtime.is_ready());
    assert_eq!(started.load(Ordering::SeqCst), 1);
    assert_eq!(report.completed_phases().len(), 6);
    assert!(report.failed_phase().is_none());

    // Verify getters and locator integration
    assert!(runtime.retrieval().is_ok());
    assert!(runtime.plugins().is_ok());
    assert!(runtime.tools().is_ok());
    assert!(runtime.storage().is_ok());

    // 2. Health check validation
    let health = runtime.health();
    assert_eq!(health.overall, HealthStatus::Healthy);
    assert_eq!(
        *health.components.get("Storage").unwrap(),
        HealthStatus::Healthy
    );
    assert_eq!(
        *health.components.get("Python").unwrap(),
        HealthStatus::Healthy
    );
    assert_eq!(
        *health.components.get("Plugins").unwrap(),
        HealthStatus::Healthy
    );
    assert_eq!(
        *health.components.get("Tools").unwrap(),
        HealthStatus::Healthy
    );

    // 3. Shutdown runtime
    runtime.shutdown().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Stopped);
    assert!(!runtime.is_ready());
    assert_eq!(stopping.load(Ordering::SeqCst), 1);
    assert_eq!(stopped.load(Ordering::SeqCst), 1);

    // Querying getters post-shutdown must fail
    assert!(runtime.retrieval().is_err());
    assert!(runtime.plugins().is_err());
    assert!(runtime.tools().is_err());
    assert!(runtime.storage().is_err());

    // Clean up files
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&plugins_path);
}

#[tokio::test]
async fn test_runtime_startup_phase_failure_rollback() {
    brain_python::initialize_python_runtime();
    std::env::set_var("BRAIN_DATABASE_TIMEOUT_MS", "500");

    // Set invalid directory path to cause StorageMigrationPhase to fail during SqliteStorage::new
    let db_path = "/invalid_directory_path_12345/non_existent.db";
    let config = create_valid_test_config(db_path, "/tmp/mock_plugins");

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .build()
        .unwrap();

    assert_eq!(runtime.state(), RuntimeState::Created);

    // Start must fail and rollback
    let res = runtime.start();
    assert!(res.is_err());

    // State must be reset back to Created, not Running or Starting
    assert_eq!(runtime.state(), RuntimeState::Created);
    assert!(!runtime.is_ready());
    assert!(runtime.storage().is_err());
}

#[tokio::test]
async fn test_runtime_non_blocking_observers() {
    brain_python::initialize_python_runtime();

    let db_path = get_temp_db_path();
    let plugins_path = get_temp_plugins_path();
    let config = create_valid_test_config(&db_path, &plugins_path);

    let observer = Arc::new(TestObserver {
        started_count: Arc::new(AtomicUsize::new(0)),
        stopping_count: Arc::new(AtomicUsize::new(0)),
        stopped_count: Arc::new(AtomicUsize::new(0)),
        should_panic: true, // observer will panic on every lifecycle hook
    });

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .register_observer(observer)
        .build()
        .unwrap();

    // Startup must complete successfully despite panicking observer
    let report = runtime.start().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Running);
    assert_eq!(report.completed_phases().len(), 6);

    // Shutdown must also complete successfully despite panicking observer
    runtime.shutdown().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Stopped);

    // Clean up
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&plugins_path);
}

#[tokio::test]
async fn test_runtime_query_rejection_when_not_running() {
    brain_python::initialize_python_runtime();

    let db_path = get_temp_db_path();
    let plugins_path = get_temp_plugins_path();
    let config = create_valid_test_config(&db_path, &plugins_path);

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .build()
        .unwrap();

    // Created state: retrieve and execute_tool must reject
    let session_id = brain_domain::SessionId::new();
    assert!(runtime.retrieve(&session_id, "hello", 10).is_err());
    assert!(runtime
        .execute_tool(&session_id, "mock_tool", &std::collections::HashMap::new())
        .is_err());

    // Start to Running
    runtime.start().unwrap();

    // Shutdown to Stopped
    runtime.shutdown().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Stopped);

    // Stopped state: retrieve and execute_tool must reject
    assert!(runtime.retrieve(&session_id, "hello", 10).is_err());
    assert!(runtime
        .execute_tool(&session_id, "mock_tool", &std::collections::HashMap::new())
        .is_err());

    // Clean up
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&plugins_path);
}
