use brain_storage::{init_pool, run_migrations};

#[test]
fn test_init_pool_in_memory() {
    let pool = init_pool(":memory:", 2, true);
    assert!(pool.is_ok(), "Failed to initialize in-memory connection pool");
    let pool = pool.unwrap();

    let conn = pool.get();
    assert!(conn.is_ok(), "Failed to get connection from pool");
}

#[test]
fn test_run_migrations_and_idempotency() {
    let pool = init_pool(":memory:", 1, true).unwrap();
    let mut conn = pool.get().unwrap();

    // First migration run should create all tables
    let result = run_migrations(&mut conn);
    assert!(result.is_ok(), "First migration run failed: {:?}", result);

    // Verify all 5 tables are present
    let tables = vec!["nodes", "edges", "embeddings", "sessions", "config"];
    for table in tables {
        let count: u32 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?;",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "Table '{}' was not created", table);
    }

    // Verify user_version is updated to 1
    let version: u32 = conn
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1, "Expected user_version to be 1, got {}", version);

    // Second run should be idempotent and do nothing without error
    let result2 = run_migrations(&mut conn);
    assert!(result2.is_ok(), "Second migration run failed (idempotency issue): {:?}", result2);

    let version2: u32 = conn
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version2, 1, "Expected user_version to remain 1, got {}", version2);
}
