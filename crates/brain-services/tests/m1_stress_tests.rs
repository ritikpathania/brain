use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_services::brain_runtime::BrainRuntime;
use brain_services::retrieval::eval_harness::production_corpus::ProductionCorpusBuilder;

/// Helper to count open file descriptors in current process on Unix/macOS.
fn count_open_fds() -> usize {
    std::fs::read_dir("/dev/fd")
        .map(|entries| entries.count())
        .unwrap_or(0)
}

/// Stress Test 1.1: ProductionCorpusBuilder Sequential Repetition & FD Leak Check
/// Executes ProductionCorpusBuilder::build() repeatedly in a sequential loop (25 iterations),
/// verifying that each build succeeds deterministically, creates valid 100-node SQLite schemas,
/// and that dropping corpora cleanly closes connection pools without leaking file descriptors.
#[tokio::test]
async fn test_stress_production_corpus_builder_repetition_and_fd_cleanliness() {
    let initial_fds = count_open_fds();

    for i in 0..25 {
        let corpus = ProductionCorpusBuilder::build().unwrap_or_else(|e| {
            panic!("Iteration {} failed to build ProductionCorpus: {:?}", i, e)
        });

        // Invariant: each corpus contains exactly 100 ground-truth nodes
        assert_eq!(
            corpus.ground_truth.nodes.len(),
            100,
            "Iteration {} ground truth nodes count mismatch",
            i
        );
        assert!(
            !corpus.queries.queries.is_empty(),
            "Iteration {} queries must not be empty",
            i
        );

        // Invariant: underlying SQLite storage is fully initialized and queryable
        let nodes = corpus
            .storage
            .nodes()
            .list_all()
            .unwrap_or_else(|e| panic!("Iteration {} failed to list nodes: {:?}", i, e));
        assert_eq!(nodes.len(), 100);

        // Explicitly drop corpus and verify cleanup
        drop(corpus);
    }

    let final_fds = count_open_fds();
    // Allow small margin of <= 4 FDs for runtime internal handles
    if initial_fds > 0 && final_fds > 0 {
        assert!(
            final_fds <= initial_fds + 4,
            "File descriptor leak detected across 25 builds! Initial: {}, Final: {}",
            initial_fds,
            final_fds
        );
    }
}

/// Stress Test 1.2: ProductionCorpusBuilder Concurrent Stress & No File Collisions
/// Spawns 8 concurrent threads invoking ProductionCorpusBuilder::build() in parallel,
/// verifying that UUID-based temporary SQLite databases do not collide, corrupt, or block.
#[test]
fn test_stress_production_corpus_builder_concurrent_no_collisions() {
    let thread_count = 8;
    let success_count = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..thread_count)
        .map(|idx| {
            let sc = Arc::clone(&success_count);
            thread::spawn(move || {
                let corpus = ProductionCorpusBuilder::build().unwrap_or_else(|e| {
                    panic!("Thread {} failed to build ProductionCorpus: {:?}", idx, e)
                });

                assert_eq!(corpus.ground_truth.nodes.len(), 100);
                let nodes = corpus.storage.nodes().list_all().unwrap();
                assert_eq!(nodes.len(), 100);

                sc.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();

    for h in handles {
        h.join()
            .expect("Thread panicked during concurrent corpus build");
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        thread_count,
        "All concurrent builds must succeed without collisions"
    );
}

/// Stress Test 2.1: BrainRuntime Startup Error Propagation on Invalid Paths
/// Tests that invalid, missing, or directory-typed database paths return typed Err(BrainError)
/// rather than silently succeeding or panicking.
#[tokio::test]
async fn test_stress_brain_runtime_startup_invalid_paths() {
    // 1. Non-existent deep parent directory
    let invalid_path = "/nonexistent_sandbox_0xdeadbeef/nested_missing_dir/brain.db";
    let res = BrainRuntime::new(invalid_path);
    assert!(
        res.is_err(),
        "BrainRuntime::new with non-existent parent directory must return Err"
    );
    match res.err().unwrap() {
        BrainError::Storage { .. } => {}
        other => panic!("Expected BrainError::Storage, got {:?}", other),
    }

    // 2. Path pointing to a directory instead of a database file
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let dir_path = temp_dir.path().to_str().unwrap();
    let dir_res = BrainRuntime::new(dir_path);
    assert!(
        dir_res.is_err(),
        "BrainRuntime::new with directory path must return Err"
    );

    // 3. Unwritable path / read-only directory
    let ro_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let mut perms = std::fs::metadata(ro_dir.path()).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(true);
    let _ = std::fs::set_permissions(ro_dir.path(), perms);
    let ro_path = ro_dir.path().join("unwritable.db");
    let ro_res = BrainRuntime::new(ro_path.to_str().unwrap());
    assert!(
        ro_res.is_err(),
        "BrainRuntime::new inside read-only directory must return Err"
    );
}

/// Stress Test 2.2: BrainRuntime Schedulers Idempotence & Error Handling
/// Verifies that scheduler start methods are re-entrant and idempotent,
/// and that BrainRuntime cleanly cleans up background tasks upon shutdown.
#[tokio::test]
async fn test_stress_brain_runtime_schedulers_idempotence_and_shutdown() {
    let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    let db_path = temp_file.path().to_str().unwrap().to_string();

    let runtime =
        BrainRuntime::new(&db_path).expect("BrainRuntime::new must succeed with valid path");

    // Invariant: re-invoking reflection_scheduler.start() returns Ok(()) idempotently
    let ref_start_res = runtime.reflection_scheduler().start();
    assert!(
        ref_start_res.is_ok(),
        "reflection_scheduler.start() must be idempotent"
    );

    // Invariant: re-invoking orchestrator.start() returns Ok(()) idempotently
    let orch_start_res = runtime.orchestrator().start();
    assert!(
        orch_start_res.is_ok(),
        "orchestrator.start() must be idempotent"
    );

    // Invariant: shutdown terminates schedulers and returns clean summary
    let summary = runtime.shutdown().expect("runtime shutdown must succeed");
    assert!(summary.duration < std::time::Duration::from_secs(10));
}
