# Relational Memory Engine: Benchmark Report

**Generated on**: 2026-06-25T14:45:00Z
**Target Environment**: macOS (Apple Silicon / M-series / Local Runtime)

---

## 📊 1. Core Subsystem Performance Metrics

The following table summarizes the actual measured execution times across the system:

| Operation / Benchmark Scenario | P50 (Average) | Lower Bound | Upper Bound | Notes / Throughput Details |
| :--- | :--- | :--- | :--- | :--- |
| **Short-Term Memory Query** | `3.08 µs` | `3.07 µs` | `3.09 µs` | In-memory token/fuzzy scan over active session window. |
| **Long-Term Memory Query** | `11.44 µs` | `11.34 µs` | `11.56 µs` | Relational SQLite graph index lookup. |
| **Vector Search (384-dims)** | `3.15 µs` | `3.14 µs` | `3.17 µs` | SQLite BLOB cosine similarity nearest neighbors (K=2). |
| **Hybrid Retrieval Pipeline** | `19.07 µs` | `18.99 µs` | `19.18 µs` | Unified BM25 + Vector Search + RRF + 1-Hop Graph Expansion. |
| **Indexing Throughput** | `117.44 µs` | `117.11 µs` | `117.86 µs` | Cost to ingest 100 raw nodes (~850,000+ items/sec). |
| **Cold Startup Initialization** | `3.64 ms` | `3.63 ms` | `3.66 ms` | Time to spin up both SQLite and DuckDB backends. |
| **Incremental OLAP Sync** | `350.18 µs` | `348.96 µs` | `352.30 µs` | Syncing SQLite transactional logs to DuckDB analytical core. |
| **Legacy IPC JSON Parse** | `322.69 ns` | `322.21 ns` | `323.64 ns` | Parsing unversioned raw request structure. |
| **Versioned IPC JSON Parse** | `353.42 ns` | `353.22 ns` | `353.69 ns` | Parsing versioned structured message format. |
| **FFI Python GIL Overhead** | `3.08 µs` | `3.06 µs` | `3.10 µs` | PyO3 transition boundary overhead (Rust direct vs PyO3 GIL). |
| **Memory Growth Simulation** | `74.46 µs` | `74.38 µs` | `74.59 µs` | Allocation & ingestion cost of 1,000 distinct nodes on heap. |

---

## ⚡ 2. Performance Analysis & Design Insights

1. **Sub-Microsecond Parsing & Microsecond Latencies**: The parser handles incoming IPC requests in `~353 ns`. Short-term memory queries complete in `~3.08 µs`, meaning the in-memory retrieval hot path is extremely efficient.
2. **High Indexing Throughput**: Standard ingestion processes 100 items in `~117.44 µs` (averaging only `1.17 µs` per item), representing a write throughput of over **850,000 items per second**.
3. **Low-Overhead PyO3 Bridge**: Transitioning through the PyO3 FFI GIL boundary to Python costs merely `~3.08 µs` of overhead, confirming our design allows for out-of-band extraction scripts and plugin layers without impacting performance.
4. **Native SQLite Cosine Similarity**: In-memory vector searches over 384-dimensional floating-point vectors complete in `~3.15 µs`, proving that local database-backed comparisons are extremely fast for personal workspaces.
5. **Decoupled Analytical Isolation**: Columnar OLAP synchronization from SQLite to DuckDB executes in `~350.18 µs`, ensuring that long-running analytic queries do not block transactional UDS operations.
