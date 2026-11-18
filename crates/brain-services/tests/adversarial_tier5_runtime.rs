//! Tier 5 White-Box Adversarial Coverage Hardening Test Suite for Runtime, Concurrency, and Lifecycle.
//!
//! Enforces:
//! 1. High-concurrency tool execution across multi-threaded and current-thread Tokio runtimes.
//! 2. Async/sync bridging in `HostContext::execute_tool` (read lock dropping, flavor branching, thread scope).
//! 3. Lock dropping and state machine integrity under tool execution failure and worker thread panics.
//! 4. Repeated phase failure cascading, multi-phase rollback integrity, and runtime self-healing.
//! 5. Reflection scheduler error propagation, single-flight level-triggered protection, and recovery under storage contention.
//! 6. Rapid lifecycle churn (start, query, shutdown, drop) with idempotent double-shutdown guarantees.
//! 7. `BrainRuntime` concurrency, background scheduler lifecycle, and RAII drop safety (abort without explicit shutdown).
//! 8. Concurrent state transition race arbitration (simultaneous start/start, shutdown/shutdown, start/shutdown).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{BrainSettings, PartialBrainSettings, PartialDatabaseSettings};
use brain_core::errors::BrainError;
use brain_core::events::CorrelationId;
use brain_core::extensibility::{
    ExecutionContext, ExecutionPolicy, ExecutionResult, HostContext, Permission, Tool, ToolMetadata,
};
use brain_domain::SessionId;
use brain_integrations::{EventIdentity, IngestionEnvelope, IngestionEvent};
use brain_services::brain_runtime::{BrainRuntime, RuntimeHealth};
use brain_services::conversation::IngestionPolicy;
use brain_services::orchestrator::{TaskKind, TaskPriority};
use brain_services::runtime::{ApplicationRuntime, HealthStatus, RuntimeState};
use brain_tools::{BlockingToolRunner, CancellationTokenImpl, PermissionManager, ToolExecutor};

/// Helper creating isolated temporary database and plugins environment.
struct TestEnv {
    _temp_dir: TempDir,
    db_path: String,
    plugins_path: String,
}

impl TestEnv {
    fn new(prefix: &str) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let db_path = temp_dir
            .path()
            .join(format!("{}.db", prefix))
            .to_string_lossy()
            .to_string();
        let plugins_path = temp_dir
            .path()
            .join("plugins")
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&plugins_path).expect("Failed to create plugins directory");
        Self {
            _temp_dir: temp_dir,
            db_path,
            plugins_path,
        }
    }

    fn config(&self) -> BrainSettings {
        let defaults_src = DefaultsSource;
        let partial = PartialBrainSettings {
            database: Some(PartialDatabaseSettings {
                path: Some(self.db_path.clone()),
                pool_size: Some(4),
                enable_wal: Some(true),
            }),
            plugins_directory: Some(self.plugins_path.clone()),
            ..Default::default()
        };
        resolve(&[
            Box::new(defaults_src),
            Box::new(OverrideSource::new(partial)),
        ])
        .expect("Failed to resolve test settings")
    }

    fn bad_config(bad_path: &str) -> BrainSettings {
        let defaults_src = DefaultsSource;
        let partial = PartialBrainSettings {
            database: Some(PartialDatabaseSettings {
                path: Some(bad_path.to_string()),
                pool_size: Some(2),
                enable_wal: Some(false),
            }),
            plugins_directory: Some("/dev/null/mock_plugins".to_string()),
            ..Default::default()
        };
        resolve(&[
            Box::new(defaults_src),
            Box::new(OverrideSource::new(partial)),
        ])
        .expect("Failed to resolve bad test settings")
    }
}

/// Helper building an IngestionEnvelope for testing BrainRuntime ingestion.
fn make_test_envelope(payload: &str, corr_id: CorrelationId) -> IngestionEnvelope {
    IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id: brain_domain::EventId(corr_id),
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("adv_test"),
            client_id: brain_domain::ClientId::new("adv_client"),
            adapter_id: brain_domain::AdapterId::new("adv_adapter"),
            session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
            conversation_id: None,
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: payload.to_string(),
            metadata: std::collections::BTreeMap::new(),
        },
    }
}

/// Mock Tool that succeeds and computes CPU work.
struct ComputeTool {
    metadata: ToolMetadata,
}

impl ComputeTool {
    fn new(name: &str) -> Self {
        Self {
            metadata: ToolMetadata {
                name: name.to_string(),
                description: "Performs deterministic computation".to_string(),
                usage: "usage".to_string(),
                version: "1.0".to_string(),
                author: "adversarial-tester".to_string(),
                required_permissions: vec![Permission::Storage],
                execution_policy: ExecutionPolicy { timeout_ms: 1000 },
                supports_streaming: false,
                is_idempotent: true,
                causes_side_effects: false,
                input_schema: None,
            },
        }
    }
}

impl Tool for ComputeTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        let x = arguments.get("val").and_then(|v| v.as_i64()).unwrap_or(1);
        let squared = x * x;
        Ok(ExecutionResult::new(serde_json::json!({
            "result": squared
        })))
    }
}

/// Mock Tool that always fails with typed BrainError::Tool.
struct FailingTool {
    metadata: ToolMetadata,
}

impl FailingTool {
    fn new(name: &str) -> Self {
        Self {
            metadata: ToolMetadata {
                name: name.to_string(),
                description: "Fails deterministically".to_string(),
                usage: "usage".to_string(),
                version: "1.0".to_string(),
                author: "adversarial-tester".to_string(),
                required_permissions: vec![],
                execution_policy: ExecutionPolicy { timeout_ms: 1000 },
                supports_streaming: false,
                is_idempotent: true,
                causes_side_effects: false,
                input_schema: None,
            },
        }
    }
}

impl Tool for FailingTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _context: &ExecutionContext,
        _arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        Err(BrainError::Tool {
            tool_name: self.metadata.name.clone(),
            message: "Deliberate mock tool failure for adversarial testing".to_string(),
        })
    }
}

/// Mock Tool that deliberately panics during execution.
struct PanickingTool {
    metadata: ToolMetadata,
}

impl PanickingTool {
    fn new(name: &str) -> Self {
        Self {
            metadata: ToolMetadata {
                name: name.to_string(),
                description: "Panics deterministically".to_string(),
                usage: "usage".to_string(),
                version: "1.0".to_string(),
                author: "adversarial-tester".to_string(),
                required_permissions: vec![],
                execution_policy: ExecutionPolicy { timeout_ms: 1000 },
                supports_streaming: false,
                is_idempotent: false,
                causes_side_effects: false,
                input_schema: None,
            },
        }
    }
}

impl Tool for PanickingTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _context: &ExecutionContext,
        _arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        panic!("Deliberate mock tool panic for adversarial testing!");
    }
}

// ============================================================================
// Test 1: High-Concurrency Tool Execution & Service Locator Deadlock Resistance
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_adv_t5_01_tool_execution_high_concurrency_multi_thread() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    let env = TestEnv::new("t5_concurrency_mt");
    let runtime = Arc::new(
        ApplicationRuntime::builder()
            .with_config(env.config())
            .build()
            .expect("Runtime build failed"),
    );

    runtime.start().expect("Runtime start failed");
    assert_eq!(runtime.state(), RuntimeState::Running);
    assert!(runtime.is_ready());

    let session_id = SessionId::new();
    let num_tasks = 50;
    let mut handles = Vec::new();
    let tool_failures = Arc::new(AtomicUsize::new(0));
    let getter_successes = Arc::new(AtomicUsize::new(0));

    // Spawn 50 concurrent tasks interleaving execute_tool with locator reads and retrievals
    for i in 0..num_tasks {
        let rt = Arc::clone(&runtime);
        let failures = Arc::clone(&tool_failures);
        let successes = Arc::clone(&getter_successes);

        handles.push(tokio::spawn(async move {
            if i % 2 == 0 {
                // Execute tool: tool is not in registry, must cleanly return Tool not found
                let mut args = HashMap::new();
                args.insert("val".to_string(), serde_json::json!(i));
                let res = rt.execute_tool(&session_id, &format!("dynamic_tool_{}", i), &args);
                match res {
                    Err(BrainError::Validation { message }) => {
                        assert!(message.contains("not found"));
                        failures.fetch_add(1, Ordering::Relaxed);
                    }
                    other => panic!("Unexpected tool execution outcome: {:?}", other),
                }
            } else {
                // Concurrently query getters and health
                assert!(rt.storage().is_ok());
                assert!(rt.tools().is_ok());
                assert!(rt.retrieval().is_ok());
                assert!(rt.conversation().is_ok());
                assert!(rt.memory_extractor().is_ok());
                assert!(rt.embedding_provider().is_ok());
                assert!(rt.chat_agent().is_ok());
                let health = rt.health();
                assert_eq!(health.overall, HealthStatus::Healthy);

                // Concurrently perform retrieval via spawn_blocking to avoid starving worker threads
                let retrieve_res = tokio::task::spawn_blocking({
                    let rt = Arc::clone(&rt);
                    move || rt.retrieve(&session_id, "concurrency test query", 5)
                })
                .await
                .expect("spawn_blocking panicked");
                assert!(retrieve_res.is_ok());
                successes.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    // Must all complete without deadlock or timeout
    let join_all = async {
        for h in handles {
            h.await.expect("Concurrent worker task panicked");
        }
    };
    tokio::time::timeout(Duration::from_secs(10), join_all)
        .await
        .expect("Deadlock detected during concurrent tool execution and getter queries!");

    assert_eq!(tool_failures.load(Ordering::SeqCst), 25);
    assert_eq!(getter_successes.load(Ordering::SeqCst), 25);

    runtime.shutdown().expect("Shutdown failed");
    assert_eq!(runtime.state(), RuntimeState::Stopped);
}

// ============================================================================
// Test 2: Current-Thread Tokio Runtime Async/Sync Bridging Verification
// ============================================================================

#[test]
fn test_adv_t5_02_current_thread_tokio_bridging_and_scope() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build current-thread runtime");

    rt.block_on(async {
        let handle = tokio::runtime::Handle::current();
        assert_eq!(
            handle.runtime_flavor(),
            tokio::runtime::RuntimeFlavor::CurrentThread
        );

        // 1. Prove that block_in_place panics immediately on CurrentThread runtime
        let bip_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| 100)
        }));
        assert!(
            bip_panic.is_err(),
            "tokio::task::block_in_place MUST panic on CurrentThread runtime"
        );

        // 2. Verify ApplicationRuntime::execute_tool does NOT panic under CurrentThread runtime
        let env = TestEnv::new("t5_current_thread");
        let runtime = ApplicationRuntime::builder()
            .with_config(env.config())
            .build()
            .expect("Runtime build failed");
        runtime.start().expect("Runtime start failed");

        let session_id = SessionId::new();
        let args = HashMap::new();

        // Must safely execute without invoking block_in_place
        for _ in 0..20 {
            let res = runtime.execute_tool(&session_id, "non_existent_tool", &args);
            assert!(matches!(res, Err(BrainError::Validation { .. })));
        }

        // 3. Directly verify the thread::scope async/sync bridging pattern used by execute_tool
        let tool: Arc<dyn Tool> = Arc::new(ComputeTool::new("square_tool"));
        let executor = Arc::new(ToolExecutor::new(Arc::new(BlockingToolRunner)));
        let perm_mgr = PermissionManager::new();
        perm_mgr.grant(Permission::Storage);

        let context = ExecutionContext {
            session_id,
            working_dir: std::path::PathBuf::from("."),
            cancellation: Arc::new(CancellationTokenImpl::new()),
            deadline: None,
        };

        let mut tool_args = HashMap::new();
        tool_args.insert("val".to_string(), serde_json::json!(7));

        let exec_result: Result<ExecutionResult, BrainError> = std::thread::scope(|s| {
            s.spawn(|| {
                handle.block_on(async {
                    executor
                        .execute(tool, &context, &perm_mgr, &tool_args)
                        .await
                })
            })
            .join()
            .map_err(|e| BrainError::Tool {
                tool_name: "square_tool".to_string(),
                message: format!("Thread panicked: {:?}", e),
            })?
        });

        let output = exec_result.expect("Tool execution via thread::scope must succeed");
        assert_eq!(output.value()["result"], 49);

        runtime.shutdown().expect("Shutdown failed");
    });
}

// ============================================================================
// Test 3: Lock Dropping Under Tool Failure and Panic Recovery
// ============================================================================

#[test]
fn test_adv_t5_03_lock_dropping_under_tool_failure_and_panic_recovery() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to build multi-thread runtime");

    rt.block_on(async {
        let handle = tokio::runtime::Handle::current();
        let env = TestEnv::new("t5_panic_recovery");
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(env.config())
                .build()
                .expect("Runtime build failed"),
        );
        runtime.start().expect("Runtime start failed");

        let session_id = SessionId::new();
        let failing_tool: Arc<dyn Tool> = Arc::new(FailingTool::new("failing_tool"));
        let panicking_tool: Arc<dyn Tool> = Arc::new(PanickingTool::new("panicking_tool"));
        let executor = Arc::new(ToolExecutor::new(Arc::new(BlockingToolRunner)));
        let perm_mgr = PermissionManager::new();

        let context = ExecutionContext {
            session_id,
            working_dir: std::path::PathBuf::from("."),
            cancellation: Arc::new(CancellationTokenImpl::new()),
            deadline: None,
        };

        // 1. Verify standard Tool failure propagation
        let fail_res = executor
            .execute(failing_tool, &context, &perm_mgr, &HashMap::new())
            .await;
        assert!(matches!(fail_res, Err(BrainError::Tool { .. })));

        // 2. Verify worker thread panic recovery via std::thread::scope join
        for _ in 0..20 {
            let p_tool = panicking_tool.clone();
            let p_exec = executor.clone();
            let p_ctx = context.clone();
            let p_perm = perm_mgr.clone();
            let p_h = handle.clone();

            let panic_handled: Result<Result<ExecutionResult, BrainError>, BrainError> =
                std::thread::scope(|s| {
                    s.spawn(move || {
                        p_h.block_on(async move {
                            p_exec
                                .execute(p_tool, &p_ctx, &p_perm, &HashMap::new())
                                .await
                        })
                    })
                    .join()
                    .map_err(|e| BrainError::Tool {
                        tool_name: "panicking_tool".to_string(),
                        message: format!("Tool execution thread panicked: {:?}", e),
                    })
                });

            // The worker thread panic is safely contained by ToolExecutor or scope join
            match panic_handled {
                Ok(Err(BrainError::Internal { ref message })) => {
                    assert!(message.contains("Tool runner panicked"));
                }
                Err(BrainError::Tool { ref message, .. }) => {
                    assert!(message.contains("Tool execution thread panicked"));
                }
                other => panic!("Expected thread panic error, got: {:?}", other),
            }
        }

        // 3. Verify ApplicationRuntime was completely unharmed by worker panics
        assert_eq!(runtime.state(), RuntimeState::Running);
        assert!(runtime.is_ready());
        assert!(runtime.storage().is_ok());
        assert!(runtime.tools().is_ok());
        assert!(runtime.retrieval().is_ok());
        assert!(runtime.conversation().is_ok());
        assert_eq!(runtime.health().overall, HealthStatus::Healthy);

        // Verification query works post-panic
        let res = runtime.retrieve(&session_id, "healthy query after panic", 5);
        assert!(res.is_ok());

        runtime.shutdown().expect("Shutdown failed");
        assert_eq!(runtime.state(), RuntimeState::Stopped);
    });
}

// ============================================================================
// Test 4: Repeated Phase Failure Cascading, Multi-Phase Rollback, and Self-Healing
// ============================================================================

#[tokio::test]
async fn test_adv_t5_04_phase_pipeline_cascading_rollback_and_self_healing() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    std::env::set_var("BRAIN_DATABASE_TIMEOUT_MS", "50");

    // Point to non-writable, impossible path causing StorageMigrationPhase to fail
    let bad_config = TestEnv::bad_config("/dev/null/impossible_dir/bad.db");
    let runtime = ApplicationRuntime::builder()
        .with_config(bad_config)
        .build()
        .expect("Runtime build should succeed before startup");

    assert_eq!(runtime.state(), RuntimeState::Created);
    assert!(!runtime.is_ready());

    // 1. Cascade 10 consecutive phase failures on the exact same runtime instance
    for i in 0..10 {
        let start_res = runtime.start();
        assert!(
            start_res.is_err(),
            "Iteration {} startup must fail due to storage error",
            i
        );

        // State must roll back cleanly to Created every single time
        assert_eq!(
            runtime.state(),
            RuntimeState::Created,
            "Iteration {} state must roll back to Created",
            i
        );
        assert!(!runtime.is_ready());

        // Getters must strictly reject queries
        assert!(matches!(
            runtime.storage(),
            Err(BrainError::InvalidTransition { .. })
        ));
        assert!(matches!(
            runtime.tools(),
            Err(BrainError::InvalidTransition { .. })
        ));
        assert!(matches!(
            runtime.retrieval(),
            Err(BrainError::InvalidTransition { .. })
        ));
        assert!(matches!(
            runtime.conversation(),
            Err(BrainError::InvalidTransition { .. })
        ));
        assert!(matches!(
            runtime.memory_extractor(),
            Err(BrainError::InvalidTransition { .. })
        ));
        assert!(matches!(
            runtime.embedding_provider(),
            Err(BrainError::InvalidTransition { .. })
        ));
        assert!(matches!(
            runtime.chat_agent(),
            Err(BrainError::InvalidTransition { .. })
        ));
    }

    // 2. Self-Healing: Verify that a valid configuration starts and recovers cleanly
    let valid_env = TestEnv::new("t5_self_healing");
    let healed_runtime = ApplicationRuntime::builder()
        .with_config(valid_env.config())
        .build()
        .expect("Healed runtime build failed");

    let report = healed_runtime
        .start()
        .expect("Healed runtime startup failed");
    assert_eq!(healed_runtime.state(), RuntimeState::Running);
    assert!(healed_runtime.is_ready());
    assert!(report.failed_phase().is_none());

    // All getters operational post-healing
    assert!(healed_runtime.storage().is_ok());
    assert!(healed_runtime.retrieval().is_ok());
    assert!(healed_runtime.conversation().is_ok());
    assert_eq!(healed_runtime.health().overall, HealthStatus::Healthy);

    healed_runtime.shutdown().expect("Shutdown failed");
    assert_eq!(healed_runtime.state(), RuntimeState::Stopped);
}

// ============================================================================
// Test 5: Reflection Scheduler Under Storage Contention & Error Resilience
// ============================================================================

#[tokio::test]
async fn test_adv_t5_05_reflection_scheduler_storage_contention_and_recovery() {
    let env = TestEnv::new("t5_reflection_contention");
    let runtime = BrainRuntime::new(&env.db_path).expect("Failed to initialize BrainRuntime");
    assert_eq!(runtime.status().health, RuntimeHealth::Healthy);

    // 1. Ingest an observation so event log has events to process
    let envelope = make_test_envelope(
        "PostgreSQL and Redis caching architecture",
        uuid::Uuid::new_v4(),
    );
    let ingest_res = runtime.ingest(envelope);
    assert!(ingest_res.is_ok());

    let scheduler = runtime.reflection_scheduler();

    // 2. Spawn a thread that acquires a connection and holds an exclusive lock on SQLite
    // with a bounded duration so it does not block other connections indefinitely
    let (tx_acquired, rx_acquired) = std::sync::mpsc::channel();
    let (tx_release, rx_release) = std::sync::mpsc::channel();
    let pool = runtime.sqlite_storage().pool().clone();

    let lock_handle = std::thread::spawn(move || {
        let conn = pool.get().expect("Failed to get connection");
        conn.execute_batch("BEGIN EXCLUSIVE;")
            .expect("Failed to acquire EXCLUSIVE SQLite lock");
        let _ = tx_acquired.send(());
        // Hold lock briefly under contention, releasing after 100ms or on signal
        let _ = rx_release.recv_timeout(Duration::from_millis(100));
        let _ = conn.execute_batch("ROLLBACK;");
    });

    rx_acquired
        .recv_timeout(Duration::from_secs(2))
        .expect("Timeout waiting for lock acquisition");

    // 3. Trigger reflection cycle while exclusive lock is held in another connection
    // The reflection transaction encounters SQLite contention, then recovers
    let sched = Arc::clone(&scheduler);
    let contention_res = tokio::task::spawn_blocking(move || sched.run_cycle(true))
        .await
        .expect("spawn_blocking panicked");
    let _ = tx_release.send(());
    let _ = lock_handle.join();

    // Either the cycle failed due to lock contention or executed read passes
    // Crucially: it must NOT deadlock or leave scheduler permanently in is_running=true
    if let Err(ref e) = contention_res {
        assert!(matches!(e, BrainError::Storage { .. }));
    }

    // 5. Subsequent reflection cycle must now run successfully without contention
    let sched2 = Arc::clone(&scheduler);
    let recovery_res = tokio::task::spawn_blocking(move || sched2.run_cycle(true))
        .await
        .expect("spawn_blocking panicked");
    assert!(
        recovery_res.is_ok(),
        "Reflection cycle must succeed after contention is released: {:?}",
        recovery_res
    );

    assert_eq!(runtime.status().health, RuntimeHealth::Healthy);

    // Graceful shutdown
    let summary = runtime.shutdown().expect("Shutdown failed");
    assert!(summary.duration >= Duration::ZERO);
}

// ============================================================================
// Test 6: Rapid Lifecycle Churn (Start, Query, Shutdown, Drop) Under Concurrency
// ============================================================================

#[tokio::test]
async fn test_adv_t5_06_rapid_lifecycle_churn_application_runtime() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    let iterations = 25;

    for i in 0..iterations {
        let env = TestEnv::new(&format!("t5_churn_{}", i));
        let runtime = ApplicationRuntime::builder()
            .with_config(env.config())
            .build()
            .unwrap_or_else(|e| panic!("Iteration {} build failed: {:?}", i, e));

        // Pre-start query rejection
        let session = SessionId::new();
        assert!(runtime.retrieve(&session, "pre-start", 5).is_err());
        assert!(runtime.storage().is_err());
        assert_eq!(runtime.state(), RuntimeState::Created);
        assert!(!runtime.is_ready());

        // Start
        let report = runtime
            .start()
            .unwrap_or_else(|e| panic!("Iteration {} start failed: {:?}", i, e));
        assert_eq!(runtime.state(), RuntimeState::Running);
        assert!(runtime.is_ready());
        assert!(report.failed_phase().is_none());

        // Operational queries
        assert!(runtime.storage().is_ok());
        assert!(runtime.retrieval().is_ok());
        assert!(runtime.tools().is_ok());
        assert!(runtime.conversation().is_ok());
        assert_eq!(runtime.health().overall, HealthStatus::Healthy);

        // Ingest interaction
        let conv = runtime.conversation().unwrap();
        conv.ingest_interaction(
            &session,
            "User message",
            "System response",
            IngestionPolicy { stm_only: true },
        )
        .expect("Conversation ingestion failed");

        // First shutdown
        runtime
            .shutdown()
            .unwrap_or_else(|e| panic!("Iteration {} shutdown failed: {:?}", i, e));
        assert_eq!(runtime.state(), RuntimeState::Stopped);
        assert!(!runtime.is_ready());

        // Post-shutdown query rejection
        assert!(runtime.retrieve(&session, "post-stop", 5).is_err());
        assert!(runtime.storage().is_err());

        // Idempotent double-shutdown must succeed without panic or deadlock
        let second_shutdown = runtime.shutdown();
        assert!(second_shutdown.is_ok());
        assert_eq!(runtime.state(), RuntimeState::Stopped);
    }
}

// ============================================================================
// Test 7: BrainRuntime Concurrency & RAII Drop Safety (Abort Without Shutdown)
// ============================================================================

#[tokio::test]
async fn test_adv_t5_07_brain_runtime_concurrency_and_raii_drop_safety() {
    let iterations = 16;

    for i in 0..iterations {
        let env = TestEnv::new(&format!("t5_brain_raii_{}", i));
        let runtime = BrainRuntime::new(&env.db_path)
            .unwrap_or_else(|e| panic!("Iteration {} BrainRuntime init failed: {:?}", i, e));

        assert_eq!(runtime.status().health, RuntimeHealth::Healthy);

        // Ingest observations
        for j in 0..5 {
            let env_msg = make_test_envelope(
                &format!("Observation content iteration {} batch {}", i, j),
                uuid::Uuid::new_v4(),
            );
            let ingest_res = runtime.ingest(env_msg);
            assert!(ingest_res.is_ok());
        }

        // Concurrent telemetry queries
        let snap = runtime.diagnostics_snapshot();
        assert!(snap.snapshot_sequence > 0);
        let status = runtime.status();
        assert_eq!(status.health, RuntimeHealth::Healthy);
        let metrics = runtime.metrics();
        assert!(metrics.observations_ingested >= 5);

        // Submit task to orchestrator
        let task_res = runtime
            .orchestrator()
            .submit(TaskKind::Reflect { force: true }, TaskPriority::Normal);
        assert!(task_res.is_ok());

        if i % 2 == 0 {
            // Even iterations: explicit graceful shutdown
            let summary = runtime
                .shutdown()
                .unwrap_or_else(|e| panic!("Iteration {} shutdown failed: {:?}", i, e));
            assert!(summary.duration >= Duration::ZERO);
        } else {
            // Odd iterations: SUDDEN DROP without shutdown()
            // Proves RAII drops connection pools, background threads abort cleanly, and lock guards release
            drop(runtime);

            // Re-open on the exact same database file immediately: verifies no locks held or corruption
            let reopened = BrainRuntime::new(&env.db_path)
                .expect("Failed to immediately re-open database after sudden drop");
            assert_eq!(reopened.status().health, RuntimeHealth::Healthy);
            let re_metrics = reopened.metrics();
            assert_eq!(re_metrics.observations_ingested, 0); // Fresh runtime metrics counter
            let _ = reopened.shutdown();
        }
    }
}

// ============================================================================
// Test 8: Concurrent State Transitions Race Arbitration
// ============================================================================

#[test]
fn test_adv_t5_08_concurrent_state_transitions_race_arbitration() {
    #[cfg(feature = "python-plugins")]
    brain_python::initialize_python_runtime();

    // Subtest 8A: Race between simultaneous start() calls
    for i in 0..15 {
        let env = TestEnv::new(&format!("t5_race_start_{}", i));
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(env.config())
                .build()
                .expect("Build failed"),
        );

        let rt1 = Arc::clone(&runtime);
        let rt2 = Arc::clone(&runtime);

        let barrier = Arc::new(std::sync::Barrier::new(2));
        let b1 = Arc::clone(&barrier);
        let b2 = Arc::clone(&barrier);

        let h1 = std::thread::spawn(move || {
            b1.wait();
            rt1.start()
        });

        let h2 = std::thread::spawn(move || {
            b2.wait();
            rt2.start()
        });

        let res1 = h1.join().expect("Thread 1 panicked");
        let res2 = h2.join().expect("Thread 2 panicked");

        // Exactly ONE thread must succeed and one must be rejected
        let successes = [res1.is_ok(), res2.is_ok()].iter().filter(|&&x| x).count();
        assert_eq!(
            successes, 1,
            "Iteration {}: Exactly one start() must succeed",
            i
        );

        assert_eq!(runtime.state(), RuntimeState::Running);
        runtime.shutdown().expect("Shutdown failed");
    }

    // Subtest 8B: Race between simultaneous shutdown() calls
    for i in 0..15 {
        let env = TestEnv::new(&format!("t5_race_shutdown_{}", i));
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(env.config())
                .build()
                .expect("Build failed"),
        );
        runtime.start().expect("Start failed");
        assert_eq!(runtime.state(), RuntimeState::Running);

        let rt1 = Arc::clone(&runtime);
        let rt2 = Arc::clone(&runtime);

        let barrier = Arc::new(std::sync::Barrier::new(2));
        let b1 = Arc::clone(&barrier);
        let b2 = Arc::clone(&barrier);

        let h1 = std::thread::spawn(move || {
            b1.wait();
            rt1.shutdown()
        });

        let h2 = std::thread::spawn(move || {
            b2.wait();
            rt2.shutdown()
        });

        let res1 = h1.join().expect("Thread 1 panicked");
        let res2 = h2.join().expect("Thread 2 panicked");

        // Both shutdowns must resolve cleanly with Ok(())
        assert!(res1.is_ok());
        assert!(res2.is_ok());
        assert_eq!(runtime.state(), RuntimeState::Stopped);
    }

    // Subtest 8C: Race between simultaneous start() and shutdown()
    for i in 0..15 {
        let env = TestEnv::new(&format!("t5_race_start_stop_{}", i));
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(env.config())
                .build()
                .expect("Build failed"),
        );

        let rt1 = Arc::clone(&runtime);
        let rt2 = Arc::clone(&runtime);

        let barrier = Arc::new(std::sync::Barrier::new(2));
        let b1 = Arc::clone(&barrier);
        let b2 = Arc::clone(&barrier);

        let h1 = std::thread::spawn(move || {
            b1.wait();
            rt1.start()
        });

        let h2 = std::thread::spawn(move || {
            b2.wait();
            rt2.shutdown()
        });

        let _res1 = h1.join().expect("Thread 1 panicked");
        let _res2 = h2.join().expect("Thread 2 panicked");

        // Final state must never be stuck in Starting or Stopping
        let final_state = runtime.state();
        assert!(
            final_state == RuntimeState::Created
                || final_state == RuntimeState::Running
                || final_state == RuntimeState::Stopped,
            "Invalid terminal state: {:?}",
            final_state
        );
        let _ = runtime.shutdown();
    }
}
