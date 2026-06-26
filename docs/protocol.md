# IPC & Observability Protocol

## 1. Unix Domain Socket JSON IPC

Communication with the daemon happens over a Unix Domain Socket (default: `~/.brain/daemon.sock`) using newline-delimited JSON frames. The transport supports both legacy commands and a rich versioned/streaming protocol.

### A. Client Request Formats

The daemon automatically deserializes requests using untagged Serde enum mapping:

#### 1. Versioned Request Frame
Used by modern clients to support request correlation IDs:
```json
{
  "version": "1.0",
  "type": "Request",
  "id": 42,
  "action": "query" | "ingest",
  "body": "request payload / query text"
}
```

#### 2. Legacy Request Frame
Retained for backwards compatibility with simple CLI invocations:
```json
{
  "action": "ingest" | "query",
  "payload": "request payload / query text"
}
```

---

### B. Server Response Formats

The server responds with either flat legacy envelopes, structured versioned envelopes, or a sequence of streaming events:

#### 1. Legacy Response Envelope
```json
{
  "status": "ok" | "error",
  "message": "formatted response output text"
}
```

#### 2. Versioned Response Envelope
```json
{
  "version": "1.0",
  "type": "Response",
  "id": 42,
  "status": "success" | "error",
  "body": "formatted response output text"
}
```

#### 3. Versioned Error Envelope
```json
{
  "version": "1.0",
  "type": "Error",
  "id": 42,
  "status": "error",
  "body": "error details message"
}
```

#### 4. Versioned Notification / Event Envelopes
Used for out-of-band updates:
```json
{
  "version": "1.0",
  "type": "Notification",
  "notification_type": "sync_complete",
  "message": "DuckDB sync completed"
}
```

#### 5. Streaming Response Events (`StreamEvent`)
When executing a query, the daemon streams the results as sequential tagged JSON frames.

- **Monotonic Sequence Numbers**: Sequence numbers (`sequence`) start at `1` and increment monotonically across all event types within a stream (Start -> Progress -> Chunk -> End).
- **Universal Metadata Block**: Every stream event variant includes an extensible `metadata` block (defaults to `{}`) for telemetry, latency stats, or query tracing.

##### Start Event (`stream_start`)
Marks the initialization of a streaming session.
```json
{
  "type": "stream_start",
  "streamId": "stream-101",
  "metadata": {}
}
```

##### Progress Event (`stream_progress`)
Emitted periodically for long-running phases (e.g. running embeddings or graph lookups).
```json
{
  "type": "stream_progress",
  "streamId": "stream-101",
  "sequence": 1,
  "progress": 0.42,
  "message": "Running hybrid retrieval...",
  "metadata": {}
}
```

##### Chunk Event (`stream_chunk`)
Contains search results or output text.
```json
{
  "type": "stream_chunk",
  "streamId": "stream-101",
  "sequence": 2,
  "content": "Found matches via Hybrid Retrieval:",
  "metadata": {}
}
```

##### End Event (`stream_end`)
Marks successful completion of the stream.
```json
{
  "type": "stream_end",
  "streamId": "stream-101",
  "sequence": 3,
  "metadata": {}
}
```

##### Cancelled Event (`stream_cancelled`)
Indicates the stream was interrupted (e.g. due to client cancellation request).
```json
{
  "type": "stream_cancelled",
  "streamId": "stream-101",
  "sequence": 3,
  "metadata": {}
}
```

---

### C. Client Resiliency & Validation

1. **Client-Side Sequence Verification**: The client tracks the expected sequence number internally. If a skipped, duplicate, or decreasing sequence is received, it logs a `[Protocol Warning] Stream sequence mismatch: expected X, got Y` warning without interrupting the output.
2. **Forward Compatibility**: The client is forward-compatible. If the daemon sends an unknown/new event type (e.g., `stream_metric`), the client logs a warning including the `streamId` (e.g., `[Protocol Warning] Ignored unknown stream event "stream_metric" for stream "stream-101"`) and continues processing subsequent stream events.

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
