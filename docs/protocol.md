# IPC & Observability Protocol

## 1. Unix Domain Socket JSON IPC

Communication with the daemon happens over a Unix Domain Socket (default: `~/.brain/daemon.sock`) using newline-delimited JSON frames.

### Client Request Frame
```json
{
  "action": "ingest" | "query",
  "payload": "transcripts string or query text"
}
```

### Server Response Frame
```json
{
  "status": "ok" | "error",
  "message": "formatted response output text"
}
```

---

## 2. HTTP Diagnostics & Metrics

An HTTP diagnostics server runs on `http://127.0.0.1:8080` for telemetry checkup.

* **`GET /health`**: Returns basic status check.
  ```json
  {"status":"ok"}
  ```
* **`GET /ready`**: Confirms readiness to handle requests.
  ```json
  {"status":"ready"}
  ```
* **`GET /metrics`**: Returns raw latency statistics and queue depth.
  ```json
  {
    "cache_hit_rate": 0.85,
    "cache_hits": 17,
    "cache_misses": 3,
    "total_queries": 20,
    "total_ingests": 42,
    "active_workers": 2,
    "queue_depth": 0,
    "avg_query_latency_us": 1450.0,
    "avg_ingest_latency_us": 230.5,
    "avg_extraction_latency_us": 450000.0,
    "avg_sqlite_latency_us": 3200.0,
    "avg_ipc_latency_us": 1800.0
  }
  ```
* **`GET /analytics/summary`**: Fetch DuckDB graph summary stats.
* **`GET /analytics/insights`**: Node type distribution and degree centrality.
* **`GET /analytics/similarity`**: Similarity recommendations.
* **`GET /analytics/slow-queries`**: Benchmarks and percentile stats.
