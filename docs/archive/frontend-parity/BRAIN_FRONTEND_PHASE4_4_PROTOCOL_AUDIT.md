# Phase 4.4 Forensic Protocol Audit: Real-time Ingestion & File Watcher Telemetry

> **Document Status**: Forensic Protocol Audit (Complete — Audit Only)  
> **Audited Subsystems**: Filesystem Watcher, Ingestion Pipeline, UDS Subscription (`v1/subscribe`), Metrics (`v1/metrics`), Projections (`v1/projections`), Memory Status  
> **Presentation Shell Status**: `🔒 FROZEN FRONTEND INFRASTRUCTURE` (Zero changes to `components/**` or `types/**`)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 4.4 FORENSIC PROTOCOL AUDIT
================================================================================
AUDIT SCOPE: Real-time Ingestion & File Watcher Telemetry
PROTOCOL STATUS: 
  - Real-time Event Subscription (v1/subscribe): FULLY IMPLEMENTED & OPERATIONAL
  - Ingestion Pipeline & Metrics (v1/ingest, v1/metrics): FULLY IMPLEMENTED & OPERATIONAL
  - Projection Lifecycle Telemetry (v1/projections): FULLY IMPLEMENTED & OPERATIONAL
  - OS-level File Watcher Daemon (in-process inotify/fsevent): NOT IMPLEMENTED IN BACKEND
VERDICT: GAP — BACKEND FILE WATCHER DOES NOT EXIST; UDS SUBSCRIPTION & TELEMETRY ARE FULLY OPERATIONAL
RUST MODIFICATIONS REQUIRED (for File Watcher): YES (Requires dedicated watcher crate or external push)
RUST MODIFICATIONS REQUIRED (for Telemetry & Ingestion Subscription): NO (Existing v1/subscribe & v1/metrics suffice)
FROZEN SHELL MODIFICATIONS: ZERO (0 lines changed in components/** or types/**)
================================================================================
```

---

## 1. Executive Verdict

### **GAP — BACKEND FILE WATCHER DOES NOT EXIST / EXISTING TELEMETRY PROTOCOL SUFFICIENT**

1. **Ingestion & Telemetry Pipeline**: **PASS**. The backend already possesses an event bus (`EventPublisher`, `EventSubscriber`, `EventLog`), real-time UDS streaming (`v1/subscribe`), runtime metrics (`v1/metrics`), projection statuses (`v1/projections`), and ingestion API (`v1/ingest`).
2. **OS Filesystem Watcher**: **GAP**. The Brain Rust backend does not currently embed an in-process filesystem watcher daemon (e.g. `notify-rs`, `fsevent`, `kqueue`). File changes must either be ingested via the existing `v1/ingest` route by external CLI tools / IDE hooks or added in a future backend milestone.
3. **Presentation Shell Compatibility**: **PASS**. The frozen frontend (`StatusLine.tsx`, `Messages.tsx`, `PresentationState.footer.memoryStatus`, `PresentationState.timeline`) is 100% capable of consuming and rendering live telemetry without altering any frozen components or types.

---

## 2. Capability Matrix

| Capability | Exists in Backend | Existing Protocol Route | Frontend Can Consume | Status / Gap |
|---|---|---|---|---|
| **OS File Watcher** | ❌ No | None | N/A | **GAP**: No OS file watcher crate in backend. Ingestion is push-based (`v1/ingest`). |
| **Ingestion Pipeline** | ✅ Yes | `v1/ingest` | Yes (via UDS) | **PASS**: Accepts `IngestionEnvelope` containing `Text`, `Observation`, or `Trace`. |
| **Real-time Event Stream** | ✅ Yes | `v1/subscribe` | Yes (`BrainUdsClient`) | **PASS**: Streams `StreamMessage::Event` (`TaskProgress`, `ProjectionInvalidated`, `RelationshipEvent`). |
| **Telemetry & Metrics** | ✅ Yes | `v1/metrics`, `v1/status` | Yes (`footer.memoryStatus`) | **PASS**: Reports `observations_ingested`, `projections_executed`, `reflections_executed`, durations. |
| **Projection Lifecycle** | ✅ Yes | `v1/projections` | Yes (`timeline` / status) | **PASS**: Lists epoch, status (`idle`, `active`, `rebuilding`, `failed`), and error info. |
| **Memory Graph Telemetry** | ✅ Yes | `v1/reflect/status`, `v1/compile/status` | Yes (`footer.memoryStatus`) | **PASS**: Real-time status for memory graph compiler and reflection engine. |

---

## 3. Exact Source Trace

```text
1. Ingestion Dispatch:
   source file:    crates/brain-application/src/application.rs (lines 135–165)
   function/type:  BrainApplication::ingest(envelope: IngestionEnvelope, context: &ExecutionContext)
   runtime owner:  BrainRuntime -> KnowledgeRuntime -> SqliteStorage
   transport route: "v1/ingest" (daemon/src/transport/uds/router.rs line 40)
   frontend consumer: BrainUdsClient.ingest() / background tool pipelines

2. Real-time Event Subscription:
   source file:    daemon/src/transport/uds/handlers.rs (lines 437–468)
   function/type:  ApplicationResponse::Subscribe(mut stream) -> tokio::spawn writer loop
   runtime owner:  BrainApplication::subscribe(after_sequence) -> SubscriptionManager
   transport route: "v1/subscribe" (daemon/src/transport/uds/router.rs line 54)
   frontend consumer: BrainUdsClient.subscribe() -> BrainFrontendAdapter.handleStreamEvent()

3. Ingestion & Graph Telemetry:
   source file:    crates/brain-application/src/application.rs (lines 398–419)
   function/type:  BrainApplication::metrics() -> v1::Metrics
   runtime owner:  BrainRuntime -> MetricsCollector
   transport route: "v1/metrics" (daemon/src/transport/uds/router.rs line 20)
   frontend consumer: BrainFrontendAdapter.setMemoryStatus("synced: <N> obs")
```

---

## 4. Existing Wire Contracts

### 1. `v1/subscribe` (Live Push Stream)
- **Request Frame**:
  ```json
  {
    "version": "1.0",
    "type": "Request",
    "id": 1,
    "action": "v1/subscribe",
    "body": ""
  }
  ```
- **Response Stream (JSON Lines)**:
  ```json
  {
    "version": "1.0",
    "msg_type": "Event",
    "event_name": "StreamMessage",
    "payload": {
      "msg_type": "event",
      "sequence": 105,
      "event": {
        "type": "task_progress",
        "payload": {
          "operation_id": "ingest_obs_01",
          "correlation_id": "corr_99",
          "state": "Processing",
          "source": "KnowledgeRuntime",
          "sequence": 105
        }
      }
    }
  }
  ```

### 2. `v1/metrics` (Polling / Refresh)
- **Request Frame**:
  ```json
  {
    "version": "1.0",
    "type": "Request",
    "id": 2,
    "action": "v1/metrics",
    "body": ""
  }
  ```
- **Response Frame**:
  ```json
  {
    "version": "1.0",
    "type": "Response",
    "id": 2,
    "status": "success",
    "body": "{\"observations_ingested\":142,\"canonicalization_successes\":140,\"canonicalization_failures\":2,\"reflections_executed\":12,\"projections_executed\":88,\"retrieval_queries\":35,\"last_ingest_duration_ms\":14,\"last_projection_duration_ms\":6,\"avg_canonicalization_duration_ms\":8,\"avg_reflection_duration_ms\":24,\"avg_dispatch_duration_ms\":3}"
  }
  ```

---

## 5. Event Lifecycle Trace

```text
[Observation / Text]
         │
         ▼
[v1/ingest API or Internal Worker]
         │
         ▼
[BrainRuntime::ingest()]
         │
         ├─► [SqliteEventLog::append()] ──► WAL Sequence Assigned
         │
         ├─► [SubscriptionManager::publish()]
         │           │
         │           ▼
         │   [v1/subscribe UDS Stream]
         │           │
         │           ▼
         │   [BrainUdsClient.ts]
         │           │
         │           ▼
         │   [BrainFrontendAdapter.ts]
         │           │
         │           ▼
         │   [PresentationState.footer.memoryStatus ("indexing" / "active: 142 obs")]
         │
         ▼
[Knowledge Compiler & Projections Update]
```

---

## 6. Frontend Presentation Compatibility

- `PresentationState.footer.memoryStatus` is a `string` property passed directly to `StatusLine.tsx`.
- Status strings such as `"active"`, `"indexing (obs: 142)"`, `"syncing..."`, or `"idle"` require **zero layout or component modifications**.
- System notifications regarding finished ingestion passes or reflection cycles can be pushed via `adapter.injectSystemMessage()`.
- **Zero changes** to `packages/brain-frontend/src/components/**` or `packages/brain-frontend/src/types/**` are necessary.

---

## 7. Rust Modification Assessment

- **Rust modifications required for Telemetry & Ingestion Events**: **NO (0 lines)**. The existing `v1/subscribe`, `v1/metrics`, and `v1/status` routes are completely implemented and operational.
- **Rust modifications required for in-daemon File Watcher**: **YES**. If an automated file watcher (e.g. `notify` crate watching the workspace directory) is required to run inside the daemon process, a background watcher worker must be added in `brain-services`.

---

## 8. Operational & Performance Risk Assessment

1. **UDS Stream Backpressure**: The daemon implements a 2-second write timeout (`tokio::time::timeout(2s, write_fut)`) with slow-consumer detection in `handlers.rs` line 452.
2. **Event Buffer Overflow**: Channels have a 1,000-element buffer. If consumption falls behind, `ReplayTruncated` control messages are emitted.
3. **Frontend Rendering Latency**: `BrainFrontendAdapter` batches status updates to avoid UI frame-rate thrashing in React/Ink.

---

## 9. Explicit Stop Condition

```text
================================================================================
PHASE 4.4 AUDIT VERDICT
================================================================================

VERDICT: GAP — BACKEND FILE WATCHER DOES NOT EXIST; UDS SUBSCRIPTION & TELEMETRY ARE FULLY OPERATIONAL

IMPLEMENTATION STATUS: NOT STARTED (AUDIT ONLY)

RUST MODIFICATIONS: 0
PRESENTATION SHELL MODIFICATIONS: 0
PROTOCOL INVENTION: 0

AWAITING APPROVAL
================================================================================
```
