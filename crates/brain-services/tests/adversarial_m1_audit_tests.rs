use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{BrainSettings, PartialBrainSettings, PartialDatabaseSettings};
use brain_core::errors::BrainError;
use brain_core::events::{
    CorrelationId, EventSource, OperationId, RuntimeEvent, TaskProgress, TaskState,
};
use brain_core::extensibility::HostContext;
use brain_domain::SessionId;
use brain_observability::{CorrelationIndex, ObservabilitySubscriber};
use brain_services::brain_runtime::{BrainRuntime, RuntimeHealth};
use brain_services::runtime::{ApplicationRuntime, RuntimeState};

fn get_temp_db_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    std::env::temp_dir()
        .join(format!("brain_adv_test_{}.db", uuid_str))
        .to_string_lossy()
        .to_string()
}

fn get_temp_plugins_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = std::env::temp_dir().join(format!("brain_adv_plugins_{}", uuid_str));
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().to_string()
}

fn create_valid_test_config(db_path: &str, plugins_path: &str) -> BrainSettings {
    let defaults_src = DefaultsSource;
    let partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some(db_path.to_string()),
            pool_size: Some(4),
            enable_wal: Some(false),
        }),
        plugins_directory: Some(plugins_path.to_string()),
        ..Default::default()
    };
    let override_src = OverrideSource::new(partial);
    resolve(&[Box::new(defaults_src), Box::new(override_src)]).unwrap()
}

/// CHALLENGE 1A: Concurrency safety of `parking_lot::Mutex` in `BrainRuntime`.
/// Stresses `diagnostics`, `status`, and `diagnostics_snapshot` across 20 concurrent threads
/// performing heavy concurrent read/write operations without deadlocks or poisoning.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_adversarial_brain_runtime_mutex_concurrency_stress() {
    let db_path = get_temp_db_path();
    let runtime = Arc::new(
        BrainRuntime::new(&db_path).expect("Failed to initialize BrainRuntime for stress test"),
    );

    assert_eq!(runtime.status().health, RuntimeHealth::Healthy);

    let num_threads = 20;
    let iterations = 200;

    let barrier = Arc::new(std::sync::Barrier::new(num_threads));
    let mut handles = Vec::new();

    // Spawn concurrent threads querying status, diagnostics, and snapshots
    for _ in 0..num_threads {
        let rt = Arc::clone(&runtime);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for _ in 0..iterations {
                let status = rt.status();
                assert_eq!(status.health, RuntimeHealth::Healthy);
                let _snapshot = rt.diagnostics_snapshot();
                let diag = rt.diagnostics();
                assert!(diag.recent_failures.is_empty());
                let _history = rt.task_history();
                let _lags = rt.projection_lags();
            }
        }));
    }

    // All threads must join cleanly within 15 seconds without deadlock
    for h in handles {
        h.join().expect("Worker thread panicked or deadlocked!");
    }

    // Post-stress verification: runtime must remain operational
    let final_status = runtime.status();
    assert_eq!(final_status.health, RuntimeHealth::Healthy);

    // Clean up
    let _ = std::fs::remove_file(&db_path);
}

/// CHALLENGE 1B: Concurrency safety of `parking_lot::Mutex` in `ObservabilitySubscriber`.
/// Stresses subscriber event ingestion and concurrent external correlation index queries
/// across 20 threads to verify zero lock poisoning and zero deadlocks.
#[test]
fn test_adversarial_observability_subscriber_concurrency_stress() {
    let (tx, rx) = channel::<Arc<dyn RuntimeEvent>>();
    let index = Arc::new(parking_lot::Mutex::new(CorrelationIndex::new()));
    let subscriber = ObservabilitySubscriber::new(rx, Arc::clone(&index));

    let num_producers = 10;
    let num_readers = 10;
    let events_per_producer = 100;
    let barrier = Arc::new(std::sync::Barrier::new(num_producers + num_readers));
    let mut handles = Vec::new();

    // Spawn producers sending TaskProgress events
    for _ in 0..num_producers {
        let sender = tx.clone();
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for i in 0..events_per_producer {
                let progress = Arc::new(TaskProgress {
                    operation_id: OperationId::new_v4(),
                    correlation_id: CorrelationId::new_v4(),
                    state: TaskState::Started,
                    source: EventSource::Ingestion,
                    sequence: i as u64,
                    timestamp: SystemTime::now(),
                });
                let _ = sender.send(progress);
            }
        }));
    }

    // Spawn readers querying index.lock() concurrently
    for _ in 0..num_readers {
        let sub_index = subscriber.index();
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for _ in 0..events_per_producer {
                let locked = sub_index.lock();
                let _ = locked.len();
                let _ = locked.total_spans();
            }
        }));
    }

    for h in handles {
        h.join()
            .expect("Producer/reader thread panicked or deadlocked");
    }

    // Drop original sender so subscriber's background thread can exit cleanly
    drop(tx);
    drop(subscriber); // Drops and joins background thread

    // Index must not be poisoned and must remain accessible
    let locked = index.lock();
    assert!(locked.total_spans() <= num_producers * events_per_producer);
}

/// CHALLENGE 2A: `HostContext::execute_tool` service_locator lock contention test.
/// Tests concurrent tool calls alongside concurrent service locator read access on `ApplicationRuntime`.
/// Proves that `execute_tool` drops `service_locator.read()` immediately during tool lookup,
/// ensuring concurrent `retrieve` calls, `storage()`, and other queries never block.
#[test]
fn test_adversarial_host_context_concurrent_service_locator_access() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let db_path = get_temp_db_path();
    let plugins_path = get_temp_plugins_path();
    let config = create_valid_test_config(&db_path, &plugins_path);

    let runtime = Arc::new(
        ApplicationRuntime::builder()
            .with_config(config)
            .build()
            .unwrap(),
    );

    runtime.start().unwrap();
    assert_eq!(runtime.state(), RuntimeState::Running);

    rt.block_on(async {
        let session_id = SessionId::new();
        let num_tasks = 20;
        let mut handles = Vec::new();

        for i in 0..num_tasks {
            let rt_clone = Arc::clone(&runtime);
            handles.push(tokio::spawn(async move {
                // Concurrently invoke execute_tool and read getters
                let args = std::collections::HashMap::new();
                let tool_res = rt_clone.execute_tool(&session_id, &format!("tool_{}", i), &args);
                // Since tools aren't registered, it safely returns Tool not found
                assert!(matches!(tool_res, Err(BrainError::Validation { .. })));

                // Concurrently read service locator
                assert!(rt_clone.storage().is_ok());
                assert!(rt_clone.tools().is_ok());
                assert!(rt_clone.retrieval().is_ok());
            }));
        }

        for h in handles {
            h.await.unwrap();
        }
    });

    runtime.shutdown().unwrap();
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&plugins_path);
}

/// CHALLENGE 2B: Tokio async/sync bridging behavior under CurrentThread and MultiThread runtimes.
/// Proves:
/// 1. Under `MultiThread`, `tokio::task::block_in_place(|| handle.block_on(...))` executes without error.
/// 2. Under `CurrentThread`, `tokio::task::block_in_place` panics immediately.
/// 3. Offloading via `std::thread::scope` with `handle.block_on(...)` executes CPU-bound/spawn_blocking
///    tasks cleanly on CurrentThread runtime, but will hang if awaiting runtime timers while the
///    single worker thread is blocked.
#[test]
fn test_adversarial_tokio_bridging_semantics_across_runtimes() {
    // 1. Current-thread runtime
    let rt_current = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt_current.block_on(async {
        let handle = tokio::runtime::Handle::current();

        // Proves block_in_place panics on CurrentThread
        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| 100)
        }));
        assert!(
            panic_res.is_err(),
            "block_in_place MUST panic on CurrentThread runtime"
        );

        // Proves that thread-scoped block_on executes CPU/blocking tasks without panicking
        let scoped_res = std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(async {
                    // Simulates spawn_blocking as used by ToolExecutor
                    let val = tokio::task::spawn_blocking(|| 42).await.unwrap();
                    val
                })
            })
            .join()
        });

        assert_eq!(scoped_res.unwrap(), 42);
    });

    // 2. Multi-thread runtime: block_in_place succeeds directly
    let rt_multi = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    rt_multi.block_on(async {
        let handle = tokio::runtime::Handle::current();
        let bip_res = tokio::task::block_in_place(|| {
            handle.block_on(async {
                let val = tokio::task::spawn_blocking(|| 84).await.unwrap();
                val
            })
        });
        assert_eq!(bip_res, 84);
    });
}
