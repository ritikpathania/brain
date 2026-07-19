# Sprint 10 — Architectural Performance Attribution Report

This report presents a side-by-side execution analysis of the legacy ingestion path vs. the transactional `BrainRuntime` ingestion path, isolating where time is spent, why it is spent there, and what choices are available to the architecture moving forward.

---

## 1. Functional Ingestion Execution Map

The diagram below maps the execution flow of the two paths:

```mermaid
graph TD
    subgraph Legacy Path [Legacy Ingestion (In-Memory)]
        L1[Ingestion Handler] --> L2[Acquire State Write Lock]
        L2 --> L3[Insert TempNode in VecDeque]
        L3 --> L4[Insert tokens in in-memory Index]
        L4 --> L5[Release Lock]
    end

    subgraph Runtime Path [BrainRuntime Ingestion (Transactional/Durable)]
        R1[Ingestion Handler] --> R2[Structural & Semantic Validation]
        R2 --> R3[Payload Hashing & UUID Generation]
        R3 --> R4[Begin DB Transaction]
        R4 --> R5[DB Lookup: current epoch & existing node]
        R5 --> R6[DB Write: Nodes & Config tables]
        R6 --> R7[DB Commit: fsync to journal]
        R7 --> R8[Ontology Reflection: Ontario engine]
        R8 --> R9[Event Dispatch to Subscribers]
    end
```

### Functional Differences Table

| Capability | Legacy Ingest Path | BrainRuntime Ingest Path |
| :--- | :--- | :--- |
| **Durability** | ❌ Volatile (lost on process exit) |  Durable (fully persisted to SQLite disk journal) |
| **Transaction Isolation** | ❌ None (concurrency managed by global Mutex) |  ACID (SQLite transaction boundaries) |
| **Ontology Realignment** | ❌ None |  Ontological validation and weights adjustments |
| **Event Propagation** | ❌ None |  Decoupled subscriber event notification system |
| **Query Indexing** | In-memory token splits | SQL FTS5 indexing / relational join keys |

---

## 2. Ingest Cost Attribution Table

The table below attributes measured and inferred latencies based on Sprint 9 benchmarks (100 observations soak test).

### Measured Costs (Scenario 2: Caching + `synchronous = NORMAL`)

| Ingestion Step | Baseline Latency | Contribution (%) | Guarantees Provided |
| :--- | :--- | :--- | :--- |
| **UDS Protocol Overhead** | ~138 µs | - | Client-server communication boundary (applies to both paths) |
| **Validation** | ~0 µs | 0.0% | Structural sanity checking |
| **Hashing & Allocations** | ~2–5 µs | ~0.5% | Deterministic node identity generation |
| **DB Lookup** | ~25–54 µs | ~5.0% | Idempotency & monotonic epoch increment constraints |
| **DB Write** | ~55–146 µs | ~12.0% | Relational schema persistence |
| **DB Commit** | ~154–341 µs | ~35.0% | SQLite transaction ACID durability boundary |
| **Ontology Reflection** | ~190–363 µs | ~28.6% | Graph semantic consistency and weights calibration |
| **Event Dispatch** | ~0.5–0.9 µs | 0.1% | Asynchronous projection updating trigger |
| **Other / Overhead** | ~12–25 µs | ~2.0% | Internal runtime processing and stats tracking |
| **Total Ingest (Server-Side)** | **~931.3 µs** | **100%** | **Durable, canonicalized, and reflected ingestion** |

### Inferred Costs (Isolating Disk Synchronization)

> [!NOTE]
> By contrasting Phase A (`synchronous = FULL`) and Phase B (`synchronous = NORMAL`), we isolate the latency contribution of full fsync disk synchronization:
> - **With FULL synchronization**: Average commit latency is **~400–1125 µs**.
> - **With NORMAL synchronization**: Average commit latency drops to **~154–341 µs**.
> - **Inference**: Full process/hardware crash durability guarantees add an unavoidable **~250–800 µs** overhead per commit transaction due to hardware platter/SSD write latency limits.

---

## 3. Operations Classification Matrix

We evaluate each operation against correctness, durability, and async potential:

| Operation | Essential? | Currently Sync? | Movable? | Trade-offs of Asynchrony / Deferral |
| :--- | :--- | :--- | :--- | :--- |
| **Validation** | Yes | Yes | ❌ No | Ingestion must fail early on invalid schemas to prevent corruption. |
| **Hashing** | Yes | Yes | ❌ No | Identity must be generated before node can be searched or saved. |
| **DB Lookup** | Yes | Yes | ❌ No | Necessary for transaction isolation and serializability. |
| **DB Write** | Yes | Yes | ❌ No | Must happen inside transaction boundaries. |
| **DB Commit** | Yes | Yes | ⚠️ Policy | Durability is policy-dependent. `NORMAL` is fast and safe against application crashes, whereas `FULL` protects against physical power failures. |
| **Reflection** | Yes | Yes |  Yes | **Movable to background worker thread**. Doing so reduces critical path latency by ~30%, but introduces a race condition where immediate subsequent queries do not observe updated weights. |
| **Event Dispatch**| Yes | Yes | ❌ No | Dispatch itself is negligible (<1 µs); subscriber queues are already async. |

---

## 4. Actionable Decision Matrix

| Finding / Discovery | Evidence Source | Category | Strategic Recommendation |
| :--- | :--- | :--- | :--- |
| **SQLite commit Durability overhead dominates** | Phase A vs Phase B comparisons | Unavoidable I/O | **Accept as fundamental cost.** Treat durability levels as operational policy rather than architectural invariants. Do not expect strict parity with volatile in-memory deques. |
| **Ontario reflection takes ~30% of latency** | Sub-stage timing logs | CPU/Memory | **Keep synchronous for semantic deterministic serializability.** Moving reflection to background queue risks query inconsistency immediately post-ingestion. |
| **UDS serialization/deserialization is stable** | Telemetry logs | Network IPC | **Maintain current protocol.** Protocol overhead is identical between the legacy and runtime UDS handlers. |
| **Prepared statement caching works** | Sprint 9 Phase A benchmarks | CPU/Memory | **Retain caching.** Avoids compile-on-request overhead inside connection threads. |

---

## 5. Answers to Key Architectural Questions

1. **Is the remaining latency fundamentally due to additional guarantees?**
   Yes. The legacy path does not write to disk, guarantee isolation, run reflection rules, or perform validation. The evidence strongly suggests that the remaining latency is primarily attributable to those additional guarantees (durability, transactional safety, validation, and ontology alignment).
2. **Which synchronous operations could become asynchronous without violating invariants?**
   Only ontology reflection and event dispatch could physically run on a background queue. However, under the current runtime consistency model, synchronous reflection preserves immediate post-ingest semantic consistency and is therefore the preferred default.
3. **Is strict latency parity with the legacy path still an appropriate objective?**
   No. Given the current architecture and durability guarantees, strict latency parity with the legacy in-memory path is not a realistic engineering objective. The target should be revised to operational budget thresholds (e.g. keeping average ingestion < 1.5ms).
4. **What future optimization opportunities remain?**
   If further speedups are required, we could explore transaction batching for high-throughput clients, but this changes single-request isolation behavior and should be evaluated on a case-by-case basis.

---

## 6. Future Decision Log

| Topic | Status | Trade-off / Notes |
| :--- | :--- | :--- |
| **Prepared statement caching** |  Adopted | Eliminates duplicate compilation overhead inside UDS handlers. |
| **`synchronous = NORMAL`** |  Deployment policy | High performance in WAL mode; trade-off is minor power-loss durability. |
| **Async reflection** | ❌ Deferred | Preserves immediate post-ingest semantic consistency over minor speedup. |
| **Transaction batching** | ⚠️ Candidate | Future evaluation target for bulk-ingest pipelines. |
| **Storage engine replacement** | ❌ Out of scope | SqliteStorage satisfies current hosting and capability targets. |
