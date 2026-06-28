# Standalone Memory Companion CLI

A stateful, terminal-based AI assistant interface that acts as a dedicated Relational Memory Engine for external autonomous agents.

## Architecture Overview

```
              ┌──────────────────────────────────────┐
              │             Ratatui TUI              │
              │         (Native Rust Client)         │
              └──────────────────┬───────────────────┘
                                 │ In-Process / UDS Connection
                                 ▼
              ┌──────────────────────────────────────┐
              │             Rust Daemon              │
              │       (IPC Listener & LTM DB)        │
              └──────────────────┬───────────────────┘
                                 │ PyO3 / Maturin FFI
                                 ▼
              ┌──────────────────────────────────────┐
              │           Python Extractor           │
              │       (Heuristic Semantic Parser)    │
              └──────────────────────────────────────┘
```

- **Frontend / CLI (`/crates/brain-tui`)**: Built using Rust and Ratatui. Acts as the user-facing interactive TUI. Implements a modular token-based TUI design system (`crates/brain-tui/src/ui/`) supporting dark/light mode configurations.
- **Backend / Daemon (`/daemon`)**: A high-speed Rust listener managing the volatile Short-Term Memory (STM) sliding window cache and the persistent Long-Term Memory (LTM) SQLite graph database.
- **Semantic Extractor (`/daemon/daemon`)**: A Python module invoked via PyO3 FFI to extract structured nodes and edges from raw conversation logs.

### Core Capabilities
- **Low-Latency Hybrid Retrieval**: Designed for sub-millisecond STM retrieval and low-latency hybrid retrieval, combining BM25, vector search, reciprocal rank fusion (RRF), and graph-based expansion.
- **Real-Time Streaming Protocol**: Multi-stage pipeline streaming response chunks and progress updates incrementally using a structured tagged event protocol (`StreamEvent`). Features include monotonic sequence tracking, forward compatibility, and client-side typewriter queue buffering.
- **Native SQLite Vector Storage**: Native SQLite-backed vector storage without an external vector database, using raw binary BLOB storage and standard float arithmetic.
- **Production-Grade Observability**: Lock-free runtime metrics instrumentation, structured JSON/Text logging, and background syncing of analytical telemetry to DuckDB.

---

## Python Developer Workflow

The Python backend is modernized using high-performance, Rust-based tooling:

- **Dependency Management**: Powered by [uv](https://github.com/astral-sh/uv) (Astral's fast Python package installer and resolver).
- **Linting & Formatting**: Handled by [Ruff](https://github.com/astral-sh/ruff) (replaces `black`, `isort`, `flake8`).
- **Type Checking**: Handled by [ty](https://github.com/astral-sh/ty) (Astral's Rust-based static type checker, replaces `mypy`).
- **Editor Diagnostics**: Integrated with [Pyrefly](https://github.com/meta-internal/pyrefly) (Meta's fast type checker and language server).

### Useful Development Commands

Run these commands inside the `daemon/` directory:

1. **Install and Sync Dependencies**
   Installs all dev dependencies (`ruff`, `ty`, `pyrefly`, `maturin`) and builds the local package in editable mode:
   ```bash
   uv sync
   ```

2. **Lock Dependencies**
   Generates or updates the `uv.lock` file:
   ```bash
   uv lock
   ```

3. **Run Code Formatter & Linter**
   Formats the source files and auto-fixes lint errors:
   ```bash
   uv run ruff format .
   uv run ruff check . --fix
   ```

4. **Static Type Checking**
   Runs `ty`'s type checker over the codebase:
   ```bash
   uv run ty check
   ```

5. **Editor Diagnostics & Checking**
   Runs Pyrefly's diagnostics:
   ```bash
   uv run pyrefly check
   ```

6. **Rust maturins builds**
   Compile and install the PyO3 Rust extension module directly into your virtualenv:
   ```bash
   uv run maturin develop
   ```

---

## Getting Started

To run the entire system locally:

1. **Sync all project environments**:
   ```bash
   cd daemon && uv sync
   ```
2. **Start the Rust daemon / interactive app**:
   ```bash
   PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo run --package brain-v2
   ```

---

## TUI Verification & Testing

To test the Ratatui TUI correctness and profile its rendering performance:

1. **Run Automated Parity & Stress Tests**:
   ```bash
   PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-tui
   ```
---

## Production-Grade Observability (Phase 5)

The Memory Companion Daemon includes built-in observability features with zero-overhead instrumentation, structured logging, and non-blocking telemetry.

### Logging Configurations
The daemon supports runtime logging configurations via environment variables:
- **`LOG_FORMAT`**: Toggle between `json` (ideal for aggregators) and `text` (human-readable formatting). Defaults to `text`.
- **`LOG_LEVEL`**: Specify logging verbosity (`error`, `warn`, `info`, `debug`, `trace`). Defaults to `info`.

Example (running daemon with JSON logs):
```bash
LOG_FORMAT=json LOG_LEVEL=debug make run-daemon
```

### Health, Readiness, Metrics & Analytics Endpoints
A lightweight HTTP server is spawned on port `8080` (separated from the Unix Domain Socket database queries) for operational health checks and analytical insights:
- **Liveness Probe**: `GET http://127.0.0.1:8080/health` (Returns `{"status":"ok"}`)
- **Readiness Probe**: `GET http://127.0.0.1:8080/ready` (Returns `{"status":"ready"}`)
- **Telemetry Metrics**: `GET http://127.0.0.1:8080/metrics`
- **Analytics Summary**: `GET http://127.0.0.1:8080/analytics/summary` (Returns high-level analytical volumes and query latencies)
- **Graph Insights**: `GET http://127.0.0.1:8080/analytics/insights` (Returns degree centrality rankings and node type distributions)
- **Node Similarity**: `GET http://127.0.0.1:8080/analytics/similarity` (Returns similarity reports of node pairs sharing connections)
- **Slow Queries**: `GET http://127.0.0.1:8080/analytics/slow-queries` (Returns latency quantiles p50/p95/p99 and lists the slowest queries)

The telemetry endpoint (`/metrics`) returns JSON containing real-time, lock-free metrics:
- **`cache_hit_rate`**: The hit rate of queries served directly from volatile STM.
- **`cache_hits` / `cache_misses`**: Total volatile cache hits and misses.
- **`total_queries` / `total_ingests`**: Total read/write operations processed.
- **`active_workers`**: Number of concurrent active worker threads handling requests.
- **`queue_depth`**: Active short-term memory sliding window cache size.
- **`avg_query_latency_us`**: Average client query processing time (in microseconds).
- **`avg_ingest_latency_us`**: Average client ingestion processing time (in microseconds).
- **`avg_extraction_latency_us`**: Average Python PyO3 FFI semantic extraction time (in microseconds).
- **`avg_sqlite_latency_us`**: Average SQLite database commit time (in microseconds).
- **`avg_ipc_latency_us`**: Average Unix Domain Socket round-trip latency (in microseconds).

---

## 📈 Performance & Benchmarks

The daemon runs real-world micro-benchmarks built using Criterion. Actual performance measurements on macOS (Apple Silicon runtime):

| Operation / Benchmark Scenario | P50 (Average) | Lower Bound | Upper Bound | Notes / Throughput Details |
| :--- | :--- | :--- | :--- | :--- |
| **Short-Term Memory Query** | `3.08 µs` | `3.07 µs` | `3.09 µs` | In-memory token/fuzzy scan over active session window. |
| **Long-Term Memory Query** | `11.44 µs` | `11.34 µs` | `11.56 µs` | Relational SQLite graph index lookup. |
| **Vector Search (384-dims)** | `3.15 µs` | `3.14 µs` | `3.17 µs` | SQLite BLOB cosine similarity nearest neighbors (K=2). |
| **Hybrid Retrieval Pipeline** | `19.07 µs` | `18.99 µs` | `19.18 µs` | Unified BM25 + Vector Search + RRF + 1-Hop Graph Expansion. |
| **Indexing Throughput** | `117.44 µs` | `117.11 µs` | `117.86 µs` | Ingests 100 raw nodes (~850,000+ items/sec). |
| **Cold Startup Initialization** | `3.64 ms` | `3.63 ms` | `3.66 ms` | Time to spin up both SQLite and DuckDB backends. |
| **Incremental OLAP Sync** | `350.18 µs` | `348.96 µs` | `352.30 µs` | Syncing SQLite transactional logs to DuckDB analytical core. |
| **FFI Python GIL Overhead** | `3.08 µs` | `3.06 µs` | `3.10 µs` | PyO3 transition boundary overhead (Rust direct vs PyO3 GIL). |

For detailed analytics and FFI/IPC overhead breakdowns, read the full [Benchmark Report](file:///Users/ritikpathania/Developer/PyCharm/brain/benchmark_report.md).
