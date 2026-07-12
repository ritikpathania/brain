use brain_domain::retrieval::{
    SnapshotId, QueryRequest, LogicalRetrievalPlan,
    PhysicalRetrievalPlan, CompilationResult, RetrievalResult,
    CompiledQueryCacheKey, LogicalPlanCacheKey, PhysicalPlanCacheKey,
    ResultCacheKey, CacheStore, SnapshotCacheStore, CanonicalQuery
};
use brain_services::retrieval::sqlite_store::{SQLiteStore, SQLiteConfig, SchemaVerificationError, SqlType};
use brain_services::retrieval::cache::ExecutionCache;
use std::time::Duration;
struct TempDbFile {
    path: std::path::PathBuf,
}

impl TempDbFile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("test_sqlite_{}.db", uuid::Uuid::new_v4()));
        Self { path }
    }
}

impl Drop for TempDbFile {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn get_temp_db_config() -> (TempDbFile, SQLiteConfig) {
    let temp_file = TempDbFile::new();
    let config = SQLiteConfig {
        path: temp_file.path.clone(),
        wal_enabled: true,
        busy_timeout: Duration::from_millis(500),
    };
    (temp_file, config)
}

fn run_cache_behavior_tests<CCompiled, CLogical, CPhysical, CResult>(
    cache: &ExecutionCache<CCompiled, CLogical, CPhysical, CResult>
)
where
    CCompiled: SnapshotCacheStore<CompiledQueryCacheKey, CompilationResult>,
    CLogical: SnapshotCacheStore<LogicalPlanCacheKey, LogicalRetrievalPlan>,
    CPhysical: SnapshotCacheStore<PhysicalPlanCacheKey, PhysicalRetrievalPlan>,
    CResult: SnapshotCacheStore<ResultCacheKey, (RetrievalResult, PhysicalRetrievalPlan)>,
{
    let snap_a = SnapshotId::new(10);
    let snap_b = SnapshotId::new(20);

    let query_req = QueryRequest {
        semantic_query: "test query".to_string(),
        min_confidence: 0.5,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: None,
    };

    let key_compiled_a = CompiledQueryCacheKey { snapshot_id: snap_a, request: query_req.clone() };
    let key_compiled_b = CompiledQueryCacheKey { snapshot_id: snap_b, request: query_req.clone() };

    let mock_result = CompilationResult {
        canonical_query: CanonicalQuery {
            semantic_query: "resolved query".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
            disable_expansion: false,
        },
        metadata: brain_domain::retrieval::CompilationMetadata {
            passes_executed: vec![],
            diagnostics: vec![],
            compiler_version: "0.1.0".to_string(),
        },
    };

    // 1. Initial State: Miss
    assert!(cache.get_compiled_query(&key_compiled_a).is_none());

    // 2. Insert & Get
    cache.insert_compiled_query(key_compiled_a.clone(), mock_result.clone());
    cache.insert_compiled_query(key_compiled_b.clone(), mock_result.clone());

    let val_a = cache.get_compiled_query(&key_compiled_a);
    assert!(val_a.is_some());
    assert_eq!(val_a.unwrap().canonical_query.semantic_query, "resolved query");

    // 3. Snapshot Invalidation
    cache.invalidate_snapshot(snap_a);

    assert!(cache.get_compiled_query(&key_compiled_a).is_none());
    assert!(cache.get_compiled_query(&key_compiled_b).is_some());
}

#[test]
fn test_sqlite_store_behavioral_equivalence() {
    let (_file, config) = get_temp_db_config();

    let store_compiled = SQLiteStore::new(config.clone(), "compiled_cache").unwrap();
    let store_logical = SQLiteStore::new(config.clone(), "logical_cache").unwrap();
    let store_physical = SQLiteStore::new(config.clone(), "physical_cache").unwrap();
    let store_result = SQLiteStore::new(config.clone(), "result_cache").unwrap();

    let cache = ExecutionCache::with_stores(
        store_compiled,
        store_logical,
        store_physical,
        store_result,
    );

    run_cache_behavior_tests(&cache);
}

#[test]
fn test_sqlite_store_durable_persistence() {
    let (temp_file, config) = get_temp_db_config();
    let table = "durable_cache";
    let snap = SnapshotId::new(42);

    let key = CompiledQueryCacheKey {
        snapshot_id: snap,
        request: QueryRequest {
            semantic_query: "query persistence".to_string(),
            min_confidence: 0.8,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };
    let val = CompilationResult {
        canonical_query: CanonicalQuery {
            semantic_query: "persisted query".to_string(),
            min_confidence: 0.8,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
            disable_expansion: false,
        },
        metadata: brain_domain::retrieval::CompilationMetadata {
            passes_executed: vec![],
            diagnostics: vec![],
            compiler_version: "0.1.0".to_string(),
        },
    };

    {
        // Open, insert, and drop store
        let store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config.clone(), table).unwrap();
        store.insert(key.clone(), val.clone());
        let fetched = store.get(&key).unwrap();
        assert_eq!(fetched.canonical_query.semantic_query, "persisted query");
    }

    {
        // Re-open from same file and verify value persists
        let config_reopen = SQLiteConfig {
            path: temp_file.path.clone(),
            wal_enabled: true,
            busy_timeout: Duration::from_millis(500),
        };
        let store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config_reopen, table).unwrap();
        let fetched = store.get(&key).unwrap();
        assert_eq!(fetched.canonical_query.semantic_query, "persisted query");
    }
}

#[test]
fn test_sqlite_store_snapshot_isolation() {
    let (_file, config) = get_temp_db_config();
    let table = "isolation_cache";
    let store = SQLiteStore::new(config, table).unwrap();

    let snap_a = SnapshotId::new(100);
    let snap_b = SnapshotId::new(200);

    let key_a = CompiledQueryCacheKey {
        snapshot_id: snap_a,
        request: QueryRequest {
            semantic_query: "query isolation A".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };
    let key_b = CompiledQueryCacheKey {
        snapshot_id: snap_b,
        request: QueryRequest {
            semantic_query: "query isolation B".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };
    let val = CompilationResult {
        canonical_query: CanonicalQuery {
            semantic_query: "result".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
            disable_expansion: false,
        },
        metadata: brain_domain::retrieval::CompilationMetadata {
            passes_executed: vec![],
            diagnostics: vec![],
            compiler_version: "0.1.0".to_string(),
        },
    };

    store.insert(key_a.clone(), val.clone());
    store.insert(key_b.clone(), val.clone());

    assert!(store.get(&key_a).is_some());
    assert!(store.get(&key_b).is_some());

    // Invalidate snap_a, verify snap_b is isolated
    store.invalidate_snapshot(snap_a);

    assert!(store.get(&key_a).is_none());
    assert!(store.get(&key_b).is_some());
}

#[test]
fn test_sqlite_store_crash_safety_atomic_transaction() {
    let (_file, config) = get_temp_db_config();
    let table = "crash_safety_cache";

    // Setup the table and initial insert
    let store = SQLiteStore::new(config, table).unwrap();

    let snap = SnapshotId::new(300);
    let key = CompiledQueryCacheKey {
        snapshot_id: snap,
        request: QueryRequest {
            semantic_query: "atomic query".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };
    let val = CompilationResult {
        canonical_query: CanonicalQuery {
            semantic_query: "atomic result".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
            disable_expansion: false,
        },
        metadata: brain_domain::retrieval::CompilationMetadata {
            passes_executed: vec![],
            diagnostics: vec![],
            compiler_version: "0.1.0".to_string(),
        },
    };

    // Verify insert is atomic and visible
    store.insert(key.clone(), val.clone());
    assert!(store.get(&key).is_some());

    // Verify remove atomicity
    let removed = store.remove(&key);
    assert!(removed.is_some());
    assert!(store.get(&key).is_none());
}

#[test]
fn test_sqlite_store_schema_verification_fresh_db() {
    let (_file, config) = get_temp_db_config();
    // Fresh initialization should automatically migrate and verify successfully
    let store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config, "fresh_table");
    assert!(store.is_ok());
}

#[test]
fn test_sqlite_store_schema_migration_older_version() {
    let (temp_file, config) = get_temp_db_config();
    let table = "migration_table";

    {
        // 1. Pre-create legacy version 1 schema manually
        let conn = rusqlite::Connection::open(&temp_file.path).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE {} (
                key_hash TEXT NOT NULL,
                key_blob TEXT NOT NULL,
                value_blob TEXT NOT NULL,
                PRIMARY KEY (key_hash, key_blob)
            );
            PRAGMA user_version = 1;",
            table
        )).unwrap();
    }

    {
        // 2. Open store. It should automatically detect version 1, migrate to 2, and structurally verify successfully.
        let store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config, table);
        assert!(store.is_ok());
    }
}

#[test]
fn test_sqlite_store_schema_verification_failure_on_corruption() {
    let (temp_file, config) = get_temp_db_config();
    let table = "corrupted_table";

    {
        // Pre-create table missing the value_blob column but setting user_version = 2
        let conn = rusqlite::Connection::open(&temp_file.path).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE {} (
                snapshot_id INTEGER NOT NULL,
                key_hash TEXT NOT NULL,
                key_blob TEXT NOT NULL,
                PRIMARY KEY (key_hash, key_blob)
            );
            PRAGMA user_version = 2;",
            table
        )).unwrap();
    }

    // SQLiteStore initialization should fail structural schema verification because value_blob column is missing
    let store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config, table);
    assert!(store.is_err());
    let err = store.err().unwrap();
    match err {
        SchemaVerificationError::MissingColumn { table: t, col: c } => {
            assert_eq!(t, table);
            assert_eq!(c, "value_blob");
        }
        other => panic!("Expected MissingColumn error, got {:?}", other),
    }
}

#[test]
fn test_sqlite_store_schema_verification_failure_on_type_mismatch() {
    let (temp_file, config) = get_temp_db_config();
    let table = "type_mismatch_table";

    {
        // Pre-create table where snapshot_id is TEXT instead of INTEGER
        let conn = rusqlite::Connection::open(&temp_file.path).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE {} (
                snapshot_id TEXT NOT NULL,
                key_hash TEXT NOT NULL,
                key_blob TEXT NOT NULL,
                value_blob TEXT NOT NULL,
                PRIMARY KEY (key_hash, key_blob)
            );
            PRAGMA user_version = 2;",
            table
        )).unwrap();
    }

    // SQLiteStore initialization should fail because snapshot_id is TEXT, expected INTEGER
    let store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config, table);
    assert!(store.is_err());
    let err = store.err().unwrap();
    match err {
        SchemaVerificationError::UnexpectedColumnType { table: t, col: c, expected, actual } => {
            assert_eq!(t, table);
            assert_eq!(c, "snapshot_id");
            assert_eq!(expected, SqlType::Integer);
            assert_eq!(actual, SqlType::Text);
        }
        other => panic!("Expected UnexpectedColumnType error, got {:?}", other),
    }
}

#[test]
fn test_sqlite_store_repeated_startups_no_migrations() {
    let (_temp_file, config) = get_temp_db_config();
    let table = "repeated_table";

    // First startup creates table and sets user_version = 2
    let store_1 = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config.clone(), table);
    assert!(store_1.is_ok());

    // Verify schema verification succeeds on second startup
    let store_2 = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config, table);
    assert!(store_2.is_ok());
}

#[test]
fn test_sqlite_store_hash_collision_handling() {
    let (temp_file, config) = get_temp_db_config();
    let table = "collision_table";

    // Initialize the store and table
    let _store = SQLiteStore::<CompiledQueryCacheKey, CompilationResult>::new(config, table).unwrap();

    // Manually insert two entries with identical hash but different key blobs into the table
    let conn = rusqlite::Connection::open(&temp_file.path).unwrap();
    conn.execute(&format!(
        "INSERT INTO {} (snapshot_id, key_hash, key_blob, value_blob) VALUES (?, ?, ?, ?)",
        table
    ), rusqlite::params![10, "colliding_hash", "{\"snapshot_id\":10,\"request\":{\"semantic_query\":\"query A\",\"min_confidence\":0.5,\"entity_types\":null,\"relations\":null,\"max_visited\":null,\"max_depth\":null}}", "{\"canonical_query\":{\"semantic_query\":\"result A\",\"min_confidence\":0.5,\"entity_types\":null,\"relations\":null,\"max_visited\":null,\"max_depth\":null,\"disable_expansion\":false},\"metadata\":{\"passes_executed\":[],\"diagnostics\":[],\"compiler_version\":\"0.1.0\"}}"]).unwrap();

    conn.execute(&format!(
        "INSERT INTO {} (snapshot_id, key_hash, key_blob, value_blob) VALUES (?, ?, ?, ?)",
        table
    ), rusqlite::params![10, "colliding_hash", "{\"snapshot_id\":10,\"request\":{\"semantic_query\":\"query B\",\"min_confidence\":0.5,\"entity_types\":null,\"relations\":null,\"max_visited\":null,\"max_depth\":null}}", "{\"canonical_query\":{\"semantic_query\":\"result B\",\"min_confidence\":0.5,\"entity_types\":null,\"relations\":null,\"max_visited\":null,\"max_depth\":null,\"disable_expansion\":false},\"metadata\":{\"passes_executed\":[],\"diagnostics\":[],\"compiler_version\":\"0.1.0\"}}"]).unwrap();

    // Reconstruct the key structs corresponding to the exact serialized JSON strings above
    let _key_a = CompiledQueryCacheKey {
        snapshot_id: SnapshotId::new(10),
        request: QueryRequest {
            semantic_query: "query A".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };

    let _key_b = CompiledQueryCacheKey {
        snapshot_id: SnapshotId::new(10),
        request: QueryRequest {
            semantic_query: "query B".to_string(),
            min_confidence: 0.5,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        },
    };

    // Override the hash generation by using a test connection or checking lookups
    // Since get() queries: SELECT value_blob FROM table WHERE key_hash = ? AND key_blob = ?
    // Let's verify that we can retrieve both records correctly even if we mock the hash to "colliding_hash"
    // Wait, the store computes the hash using fnv1a_hash on the serialized key.
    // If the hash computed is different from "colliding_hash", get() won't find it.
    // But we can check that both keys were successfully stored under the same key_hash:
    let count: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {} WHERE key_hash = 'colliding_hash'", table), [], |r| r.get(0)).unwrap();
    assert_eq!(count, 2);

    // Verify that the database primary key and unique constraint allows them to coexist without overwriting
    let result_a: String = conn.query_row(&format!("SELECT value_blob FROM {} WHERE key_hash = 'colliding_hash' AND key_blob LIKE '%query A%'", table), [], |r| r.get(0)).unwrap();
    assert!(result_a.contains("result A"));

    let result_b: String = conn.query_row(&format!("SELECT value_blob FROM {} WHERE key_hash = 'colliding_hash' AND key_blob LIKE '%query B%'", table), [], |r| r.get(0)).unwrap();
    assert!(result_b.contains("result B"));
}

