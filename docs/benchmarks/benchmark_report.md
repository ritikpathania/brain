# System Benchmark Execution Report

This document records the latency, throughput, memory allocation, and parse overhead metrics for the `brain` relational memory engine.

## 📊 1. Core System Benchmark Results

| Benchmark Metric | Mean Latency | Min Latency | Max Latency | Subsystem / Description |
|---|---|---|---|---|
| **Short-Term Memory Lookup** | `3.08 µs` | `3.06 µs` | `3.10 µs` | In-memory candidate search via fuzzy/exact matching. |
| **Long-Term Memory Query** | `11.44 µs` | `11.34 µs` | `11.56 µs` | Relational SQLite graph index lookup. |
| **Vector Search (384-dims)** | `3.15 µs` | `3.14 µs` | `3.17 µs` | SQLite BLOB cosine similarity nearest neighbors (K=2). |
| **Hybrid Retrieval Pipeline** | `19.07 µs` | `18.99 µs` | `19.18 µs` | Unified BM25 + Vector Search + RRF + 1-Hop Graph Expansion. |
| **Indexing Throughput** | `117.44 µs` | `117.11 µs` | `117.86 µs` | Cost to ingest 100 raw nodes (~850,000+ items/sec). |
| **Cold Startup Initialization** | `3.64 ms` | `3.63 ms` | `3.66 ms` | Time to spin up SQLite storage and memory projections. |
| **Incremental Projection Sync** | `350.18 µs` | `348.96 µs` | `352.30 µs` | Syncing SQLite transactional logs to search projections. |
| **Legacy IPC JSON Parse** | `322.69 ns` | `322.21 ns` | `323.64 ns` | Parsing unversioned raw request structure. |
| **Versioned IPC JSON Parse** | `353.42 ns` | `353.22 ns` | `353.69 ns` | Parsing versioned structured message format. |
| **FFI Python GIL Overhead** | `3.08 µs` | `3.06 µs` | `3.10 µs` | PyO3 transition boundary overhead (Rust direct vs PyO3 GIL). |
| **Memory Growth Simulation** | `74.46 µs` | `74.38 µs` | `74.59 µs` | Allocation & ingestion cost of 1,000 distinct nodes on heap. |



---

## 🖥️ 3. TUI & Daemon Performance Baseline Metrics

Recorded via `./scripts/perf_runner.sh target/perf_baseline.json` and Criterion (`cargo bench -p brain-tui`):

| Category | Benchmark / Metric | Measured Baseline Value | Subsystem / Description |
|---|---|---|---|
| **Daemon Initialization** | `cold_startup_ms` | `497 ms` | Cold daemon process startup and UDS socket readiness. |
| **Daemon Resource** | `sampled_rss_kb` | `16,080 KB` (~16.08 MB) | Peak RSS memory sample under steady state daemon operation. |
| **Daemon Resource** | `idle_cpu_percent` | `0.0 %` | Idle CPU utilization during UDS socket polling. |
| **TUI Rendering** | `frame_draw_empty_120x40` | `108.12 µs` | Criterion benchmark for empty container layout frame render. |
| **TUI Tokenizer** | `tokenizer_feed_chunk` | `516.59 ns` | Criterion benchmark for streaming typewriter chunk parsing. |

