//! Query execution caching structures and pluggable storage backend traits.
//!
//! # Backend Capability Matrix
//!
//! | Backend | Persistence | Durability | Thread-Safe | Concurrency Scaling | Target Environment |
//! | :--- | :--- | :--- | :--- | :--- | :--- |
//! | **`InMemoryStore`** | No | Volatile | Yes | Serialized (Single lock) | Baseline developer runtime / tests. |
//! | **`DashMapStore`** | No | Volatile | Yes | High concurrent reads (Striped lock) | High-throughput in-memory runtime. |
//! | **`SQLiteStore`** | Yes (Disk) | Durable | Yes | Good reads, serialized writes (WAL Mode) | Desktop and local agent deployments. |
//! | **`MmapStore`** | Yes (Disk) | Durable (file-backed) | Read-Mostly | Shared memory / OS page caching | Large local read-only caches. |
//! | **`RedisStore`** | Yes (Network) | Configurable | Yes | High concurrent throughput (network-bound) | Distributed server-side environments. |
//!
//! # Expected Operational Complexity
//!
//! | Backend | Lookup | Insert | Snapshot Invalidation |
//! | :--- | :--- | :--- | :--- |
//! | **`InMemoryStore`** | Expected \(O(1)\) | Expected \(O(1)\) | \(O(N)\) key scan (retaining map) |
//! | **`DashMapStore`** | Expected \(O(1)\) | Expected \(O(1)\) | Implementation-dependent (e.g. key iteration) |
//! | **`SQLiteStore`** | Index-dependent \(O(\log N)\) | Index-dependent \(O(\log N)\) | Index-dependent \(O(\log N)\) (parameterized query) |
//! | **`MmapStore`** | Index-dependent \(O(\log N)\) | High write overhead | Index-dependent |
//! | **`RedisStore`** | Expected \(O(1)\) | Expected \(O(1)\) | Secondary-index dependent |
//!
//! # Deployment & Selection Flow
//!
//! * **Need zero dependencies?**
//!   → `InMemoryStore`
//! * **Need high-throughput single-process execution?**
//!   → `DashMapStore`
//! * **Need durable local persistence?**
//!   → `SQLiteStore`
//! * **Need distributed shared cache?**
//!   → `RedisStore`
//! * **Need very large read-mostly datasets?**
//!   → `MmapStore`

use crate::retrieval::models::{CanonicalQuery, LogicalRetrievalPlan, QueryRequest, SnapshotId};

/// Cache key for the query compilation layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CompiledQueryCacheKey {
    /// Target execution snapshot version.
    pub snapshot_id: SnapshotId,
    /// natural query request.
    pub request: QueryRequest,
}

/// Cache key for the logical planning layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LogicalPlanCacheKey {
    /// Target execution snapshot version.
    pub snapshot_id: SnapshotId,
    /// Canonical synonym-resolved query.
    pub query: CanonicalQuery,
}

/// Cache key for the physical planning layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PhysicalPlanCacheKey {
    /// Target execution snapshot version.
    pub snapshot_id: SnapshotId,
    /// Unoptimized logical plan.
    pub plan: LogicalRetrievalPlan,
}

/// Cache key for the execution result layer, keying on the natural query.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ResultCacheKey {
    /// Target execution snapshot version.
    pub snapshot_id: SnapshotId,
    /// The incoming query request.
    pub request: QueryRequest,
}

/// Simple metrics counter tracking hit/miss statistics.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LayerStats {
    /// Total successful cache hits.
    pub hits: u64,
    /// Total cache misses requiring evaluation.
    pub misses: u64,
}

/// Composed execution stats across all layers of the retrieval pipeline cache.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionCacheStats {
    /// Statistics for compiled query cache.
    pub compiled: LayerStats,
    /// Statistics for logical plan cache.
    pub logical: LayerStats,
    /// Statistics for physical plan cache.
    pub physical: LayerStats,
    /// Statistics for retrieval result cache.
    pub result: LayerStats,
    /// Aggregate statistics across all cache tiers.
    pub aggregate: LayerStats,
}

/// Base storage operations for key-value caches.
pub trait CacheStore<K, V>: Send + Sync {
    /// Retrieves a cloned value for a given key, if it exists.
    fn get(&self, key: &K) -> Option<V>;
    /// Inserts a key-value entry into the store.
    fn insert(&self, key: K, value: V);
    /// Removes a key-value entry from the store, returning the value if present.
    fn remove(&self, key: &K) -> Option<V>;
    /// Clears all entries from the store.
    fn clear(&self);
}

/// Snapshot invalidation extension for caches.
pub trait SnapshotCacheStore<K, V>: CacheStore<K, V> {
    /// Invalidates all entries associated with the specified snapshot_id, making them unobservable.
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId);
}
