use brain_services::{BrainRuntime, CapabilityState};
use std::sync::Arc;

#[tokio::test]
async fn test_capabilities_discovery_and_stability() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_capabilities.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

    // Discover capabilities
    let caps = runtime.discover_capabilities();

    // Verify 4 default capabilities are returned
    assert_eq!(caps.len(), 4);

    // Verify each descriptor fields stability and values
    for cap in &caps {
        assert!(!cap.name.is_empty());
        assert!(!cap.description.is_empty());
        assert_eq!(cap.version, 1);
        assert_eq!(cap.state, CapabilityState::Active);
        assert!(cap.is_enabled);
        assert!(!cap.is_experimental);
    }

    runtime.shutdown().expect("Clean shutdown");
}

#[tokio::test]
async fn test_capabilities_alphabetical_ordering() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_ordering.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

    let caps = runtime.discover_capabilities();

    // Expected list in strict alphabetical order:
    // "evolution" < "projection" < "storage" < "subscription"
    assert_eq!(caps[0].name, "evolution");
    assert_eq!(caps[1].name, "projection");
    assert_eq!(caps[2].name, "storage");
    assert_eq!(caps[3].name, "subscription");

    runtime.shutdown().expect("Clean shutdown");
}

#[tokio::test]
async fn test_capabilities_concurrent_read_safety() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_concurrency.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = Arc::new(BrainRuntime::new(db_str).expect("Failed to construct runtime"));

    let mut threads = Vec::new();
    for _ in 0..10 {
        let runtime_clone = Arc::clone(&runtime);
        threads.push(std::thread::spawn(move || {
            for _ in 0..50 {
                let caps = runtime_clone.discover_capabilities();
                assert_eq!(caps.len(), 4);
                assert_eq!(caps[0].name, "evolution");
            }
        }));
    }

    for t in threads {
        t.join().expect("Thread joined successfully");
    }

    // Since runtime is in an Arc, we consume it by using Arc::try_unwrap
    let runtime_owned = Arc::try_unwrap(runtime).ok().expect("Arc unwrap succeeds");
    runtime_owned.shutdown().expect("Clean shutdown");
}
