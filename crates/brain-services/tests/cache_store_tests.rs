use brain_domain::retrieval::{
    CacheStore, CanonicalQuery, CompilationResult, CompiledQueryCacheKey, LogicalPlanCacheKey,
    LogicalRetrievalPlan, PhysicalPlanCacheKey, PhysicalRetrievalPlan, QueryRequest,
    ResultCacheKey, RetrievalResult, SnapshotCacheStore, SnapshotId,
};
use brain_services::retrieval::cache::{ExecutionCache, InMemoryStore};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Mock store wrapper that counts operations for equivalence checks
struct MockStore<K, V> {
    inner: InMemoryStore<K, V>,
    gets: Arc<AtomicUsize>,
    inserts: Arc<AtomicUsize>,
    invalidations: Arc<AtomicUsize>,
}

impl<K, V> MockStore<K, V> {
    fn new(
        gets: Arc<AtomicUsize>,
        inserts: Arc<AtomicUsize>,
        invalidations: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner: InMemoryStore::new(),
            gets,
            inserts,
            invalidations,
        }
    }
}

impl<K, V> CacheStore<K, V> for MockStore<K, V>
where
    K: Eq + std::hash::Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn get(&self, key: &K) -> Option<V> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.get(key)
    }

    fn insert(&self, key: K, value: V) {
        self.inserts.fetch_add(1, Ordering::SeqCst);
        self.inner.insert(key, value);
    }

    fn remove(&self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    fn clear(&self) {
        self.inner.clear();
    }
}

impl<V> SnapshotCacheStore<CompiledQueryCacheKey, V> for MockStore<CompiledQueryCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        self.invalidations.fetch_add(1, Ordering::SeqCst);
        self.inner.invalidate_snapshot(snapshot_id);
    }
}

impl<V> SnapshotCacheStore<LogicalPlanCacheKey, V> for MockStore<LogicalPlanCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        self.invalidations.fetch_add(1, Ordering::SeqCst);
        self.inner.invalidate_snapshot(snapshot_id);
    }
}

impl<V> SnapshotCacheStore<PhysicalPlanCacheKey, V> for MockStore<PhysicalPlanCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        self.invalidations.fetch_add(1, Ordering::SeqCst);
        self.inner.invalidate_snapshot(snapshot_id);
    }
}

impl<V> SnapshotCacheStore<ResultCacheKey, V> for MockStore<ResultCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        self.invalidations.fetch_add(1, Ordering::SeqCst);
        self.inner.invalidate_snapshot(snapshot_id);
    }
}

// Shared Behavioral Runner confirming invariant guarantees for any cache implementation
fn run_cache_behavior_tests<CCompiled, CLogical, CPhysical, CResult>(
    cache: &ExecutionCache<CCompiled, CLogical, CPhysical, CResult>,
) where
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

    let key_compiled_a = CompiledQueryCacheKey {
        snapshot_id: snap_a,
        request: query_req.clone(),
    };
    let key_compiled_b = CompiledQueryCacheKey {
        snapshot_id: snap_b,
        request: query_req.clone(),
    };

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
    assert_eq!(
        val_a.unwrap().canonical_query.semantic_query,
        "resolved query"
    );

    // 3. Snapshot Invalidation: Purged snapshot is unobservable, unaffected snapshot remains available
    cache.invalidate_snapshot(snap_a);

    assert!(cache.get_compiled_query(&key_compiled_a).is_none());
    assert!(cache.get_compiled_query(&key_compiled_b).is_some());
}

#[test]
fn test_in_memory_store_behavior() {
    let cache = ExecutionCache::new();
    run_cache_behavior_tests(&cache);
}

#[test]
fn test_mock_store_behavior() {
    let gets = Arc::new(AtomicUsize::new(0));
    let inserts = Arc::new(AtomicUsize::new(0));
    let invalidations = Arc::new(AtomicUsize::new(0));

    let store_compiled = MockStore::new(gets.clone(), inserts.clone(), invalidations.clone());
    let store_logical = MockStore::new(gets.clone(), inserts.clone(), invalidations.clone());
    let store_physical = MockStore::new(gets.clone(), inserts.clone(), invalidations.clone());
    let store_result = MockStore::new(gets.clone(), inserts.clone(), invalidations.clone());

    let cache =
        ExecutionCache::with_stores(store_compiled, store_logical, store_physical, store_result);

    run_cache_behavior_tests(&cache);

    // Assert operations were recorded correctly through the mock wrapper
    assert_eq!(gets.load(Ordering::SeqCst), 4); // 2 misses, 2 hits
    assert_eq!(inserts.load(Ordering::SeqCst), 2);
    assert_eq!(invalidations.load(Ordering::SeqCst), 4); // invalidate_snapshot calls invalidation on each layer
}

#[test]
fn test_cross_backend_equivalence() {
    let gets_mock = Arc::new(AtomicUsize::new(0));
    let inserts_mock = Arc::new(AtomicUsize::new(0));
    let invalidations_mock = Arc::new(AtomicUsize::new(0));

    let cache_in_memory = ExecutionCache::new();

    let cache_mock = ExecutionCache::with_stores(
        MockStore::new(
            gets_mock.clone(),
            inserts_mock.clone(),
            invalidations_mock.clone(),
        ),
        MockStore::new(
            gets_mock.clone(),
            inserts_mock.clone(),
            invalidations_mock.clone(),
        ),
        MockStore::new(
            gets_mock.clone(),
            inserts_mock.clone(),
            invalidations_mock.clone(),
        ),
        MockStore::new(
            gets_mock.clone(),
            inserts_mock.clone(),
            invalidations_mock.clone(),
        ),
    );

    let snap = SnapshotId::new(42);
    let query_req = QueryRequest {
        semantic_query: "equiv query".to_string(),
        min_confidence: 0.9,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: None,
    };
    let key = CompiledQueryCacheKey {
        snapshot_id: snap,
        request: query_req,
    };
    let val = CompilationResult {
        canonical_query: CanonicalQuery {
            semantic_query: "equiv output".to_string(),
            min_confidence: 0.9,
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

    // Equivalence Step 1: Initial miss
    let res_mem_1 = cache_in_memory.get_compiled_query(&key);
    let res_mock_1 = cache_mock.get_compiled_query(&key);
    assert_eq!(res_mem_1.is_none(), res_mock_1.is_none());

    // Equivalence Step 2: Insertion
    cache_in_memory.insert_compiled_query(key.clone(), val.clone());
    cache_mock.insert_compiled_query(key.clone(), val.clone());

    // Equivalence Step 3: Hit retrieval value comparison
    let res_mem_2 = cache_in_memory.get_compiled_query(&key).unwrap();
    let res_mock_2 = cache_mock.get_compiled_query(&key).unwrap();
    assert_eq!(
        res_mem_2.canonical_query.semantic_query,
        res_mock_2.canonical_query.semantic_query
    );

    // Equivalence Step 4: Statistics equivalence
    let stats_mem = cache_in_memory.stats();
    let stats_mock = cache_mock.stats();
    assert_eq!(stats_mem.compiled.hits, stats_mock.compiled.hits);
    assert_eq!(stats_mem.compiled.misses, stats_mock.compiled.misses);

    // Equivalence Step 5: Invalidation observable behavior
    cache_in_memory.invalidate_snapshot(snap);
    cache_mock.invalidate_snapshot(snap);

    let res_mem_3 = cache_in_memory.get_compiled_query(&key);
    let res_mock_3 = cache_mock.get_compiled_query(&key);
    assert_eq!(res_mem_3.is_none(), res_mock_3.is_none());
}
