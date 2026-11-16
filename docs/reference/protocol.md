---
status: active
owner: protocol
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# IPC & Observability Protocol

## 1. Unix Domain Socket JSON IPC

The daemon reads and writes newline-delimited JSON over a Unix Domain Socket. `BRAIN_SOCKET_PATH` overrides the socket location. Otherwise the socket is `daemon.sock` under `BRAIN_CONFIG_DIR`, which defaults to `~/.brain`. The transport supports legacy requests and versioned request and streaming frames.

### A. Client Request Formats

The daemon automatically deserializes requests using untagged Serde enum mapping:

#### 1. Versioned Request Frame
Used by modern clients to support request correlation IDs:
```json
{
  "version": "1.0",
  "type": "Request",
  "id": 42,
  "action": "query",
  "body": "request payload / query text",
  "workspace_context": ["/path/to/workspace"]
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

> [!NOTE]
> **Legacy Compatibility & Deprecation Lifecycle**:
> * **v1.x**: Legacy and Versioned envelopes are both fully supported.
> * **v2.0**: Legacy envelopes are deprecated (clients should migrate to Versioned envelopes).
> * **v3.0**: Legacy envelopes will be completely removed from the IPC parser.

---

### B. Server Response Formats

The server responds with either flat legacy envelopes, structured versioned envelopes, or a sequence of streaming events:

#### 1. Legacy Response Envelope
*Emitted only in response to Legacy Requests.*
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
  "message": "Projection sync completed"
}
```

#### 5. Search Streaming Response Events (`StreamEvent`)
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

### C. Search Client Resiliency & Validation

1. **Client-Side Sequence Verification & Invariants**: The client enforces strict stream invariants and logs warnings to stderr without crashing the UI:
   - **Sequence Mismatch**: If the sequence number skips expected order, logs `[Protocol Warning] Stream sequence mismatch: expected X, got Y`.
   - **Sequence Regression**: If the sequence number decreases or is duplicated, logs `[Protocol Warning] Stream sequence regressed: expected greater than X, got Y`.
   - **Unterminated Streams**: If a new stream starts before the previous one terminates, logs `[Protocol Warning] Stream "X" was not terminated before starting stream "Y"`.
   - **Reconnect Resurrection / Terminated Check**: If a packet arrives for a stream that was already terminated or stale, logs `[Protocol Warning] Received packet for already terminated stream "X"`.
2. **Forward Compatibility**: The client is forward-compatible. If the daemon sends an unknown/new event type (e.g., `stream_metric`), the client logs a warning including the `streamId` (e.g., `[Protocol Warning] Ignored unknown stream event "stream_metric" for stream "stream-101"`) and continues processing subsequent stream events.

---

### D. First-party Frontend Handshake

The frontend calls `v1/frontend/handshake` after connecting and before enabling Brain-backed actions. Send a versioned `Request` with a numeric `id` and a JSON-encoded string in `body`. The decoded request body is shaped as follows.

```json
{
  "protocol": "brain.frontend",
  "minimumProtocolVersion": 1,
  "maximumProtocolVersion": 1,
  "frontendName": "brain-frontend",
  "frontendVersion": "0.1.0",
  "frontendBuildId": null,
  "requiredCapabilities": ["sessions.v1", "generation.stream.v1", "generation.cancel.v1"],
  "optionalCapabilities": []
}
```

The daemon echoes the request ID. A successful `Response` has a JSON-encoded body with `selectedProtocolVersion`, `backendName`, `backendVersion`, `daemonState`, `supportedCapabilities`, and `acceptedOptionalCapabilities`. The frontend requires version 1 and all three capabilities in the example. A versioned `Error` body contains `code` and `message`. The handshake error codes are `malformed_handshake_request`, `unsupported_frontend_mode`, `unsupported_protocol_version`, and `missing_required_capability`.

### E. First-party Session and Generation Stream

Start or load an authoritative session before sending `v1/generation/stream`. Send the Brain session ID, a generation ID created by the frontend, and the messages in the versioned request's JSON-encoded `body`. The frontend can also specify `model`. An example decoded body follows.

```json
{
  "sessionId": "07d4e283-2288-4c8c-8555-89106504b00a",
  "generationId": "6c96dc69-df9e-41d9-a6d3-8b5593dddfb1",
  "messages": [{"role": "user", "content": "What did we decide about indexing?"}]
}
```

- Each generation response frame carries `type`, `session_id`, `generation_id`, and a contiguous `sequence` beginning at zero. Requests rejected before the daemon can parse the generation payload can receive a sequence-zero error without correlation fields. The frontend rejects frames for another generation, session, or connection epoch.
- A started stream begins with `stream_start` and can carry `thinking_start`, `thinking_delta`, `thinking_end`, `token`, `tool_use`, `tool_permission_requested`, `tool_result`, or `tool_denied`. Successful generation can include `stream_end` with the assembled response and metadata, followed by `finished` with status `completed`. The frontend also treats an `error` frame or a failed or cancelled settlement as terminal for the active turn.
- Send `v1/generation/cancel` with both IDs on a separate request connection. Its direct `{"type":"cancelled","status":"ok"}` acknowledgement has a separate lifecycle from the generation stream's `finished` settlement.

When the daemon assembles retrieval context successfully, `stream_start` and a corresponding `stream_end` carry the same `metadata.brain_context` snapshot. Its version-1 shape is as follows.

```json
{
  "version": 1,
  "retrieval_epoch_id": "0d5cdcb0-a537-4834-86e4-d2d9c8532122",
  "retrieval_count": 0,
  "retrieval_channels": [],
  "context_token_count": 0,
  "source_refs": []
}
```

`retrieval_count` equals the number of included opaque `source_refs`. Allowed channel values are `lexical`, `semantic`, `graph`, and `hybrid`. `context_token_count` measures assembled retrieval context separately from provider token usage. The snapshot excludes memory content, labels, ranking scores, and storage properties. If context assembly fails, generation can continue with the snapshot absent. The frontend distinguishes absence from a supplied snapshot with zero results.

---

## 2. HTTP Diagnostics & Metrics

An HTTP diagnostics server runs on `http://127.0.0.1:8080` for telemetry checkup.

* **`GET /health`**: Returns basic health status.
  ```json
  {"status":"ok"}
  ```
* **`GET /ready`**: Confirms daemon readiness to handle incoming requests.
  ```json
  {"status":"ready"}
  ```
* **`GET /status`**: Returns overall runtime and subsystem status JSON.
* **`GET /diagnostics`**: Returns environment and dependency diagnostics checkup.
* **`GET /metrics`**: Returns Prometheus text exposition format metrics (`text/plain; version=0.0.4`).
* **`GET /metrics/json`**: Returns raw latency statistics, cache hit rates, and worker queue depths in JSON format:
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
