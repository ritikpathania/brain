# Sprint 8 — Runtime Observability

## Objective

Explain *why* runtime latency changes, not just *whether* it changes.

Sprint 7 answered: is the runtime reliable? is it slower?
Sprint 8 answers: which stage causes the slowdown? where does time go?

---

## Context

Sprint 7 exposed a single latency number per ingest:

```text
runtime_avg_ingest_latency_us = 680 µs
legacy_avg_ingest_latency_us  = 420 µs
runtime_latency_ratio         = 1.62x
```

When `runtime_latency_ratio` rises above acceptable range, there is currently no
information to distinguish:

```text
Is canonicalization slow?
Is reflection slow?
Is SQLite slow?
Is event dispatch slow?
Is it a queue depth problem?
```

Sprint 8 adds the instrumentation to answer those questions — from the daemon's
observability layer and from `BrainRuntime`'s existing introspection API —
without changing `BrainRuntime`'s public method signatures.

---

## Architecture

```text
Daemon (instrumentation layer)
    │
    ├── tcp.rs          ← p50/p95/p99 latency estimation, stage breakdown endpoint
    └── metrics.rs      ← reservoir sampler for percentile estimation

BrainRuntime
    ├── brain_runtime.rs  ← IngestionResult gains stage_timings field
    └── RuntimeMetrics    ← adds per-stage average durations
```

### BrainRuntime changes — justified

Sprint 8 extends two existing structs. Method signatures are unchanged:

- `runtime.ingest()` still returns `Result<IngestionResult, BrainError>`
- `runtime.metrics()` still returns `RuntimeMetrics`

The structs themselves gain fields. Existing callers that only read
`result.epoch` or `metrics.observations_ingested` are unaffected.

This is the legitimate architectural reason: a new observability capability that
belongs naturally inside the runtime, not bolted on in the host layer.

---

## Proposed Changes

### `crates/brain-services/src/brain_runtime.rs`

#### [MODIFY] `IngestionResult`

Add per-stage timing to the value returned from `runtime.ingest()`:

```rust
pub struct StageTimings {
    pub canonicalization: Duration,
    pub reflection: Duration,
    pub dispatch: Duration,
}

pub struct IngestionResult {
    pub epoch: Epoch,
    pub affected_entities: Vec<EntityId>,
    pub stage_timings: StageTimings,   // new
}
```

The ingest path already sequences canonicalize → reflect → dispatch. Wrapping
each with `Instant::now()` pairs is the only internal change required.

#### [MODIFY] `RuntimeMetrics`

Add cumulative per-stage averages to the value returned from `runtime.metrics()`:

```rust
pub struct RuntimeMetrics {
    // existing
    pub observations_ingested: u64,
    pub projections_executed: u64,
    pub reflections_executed: u64,
    pub last_ingest_duration: Option<Duration>,
    pub last_projection_duration: Option<Duration>,
    pub last_reflection_duration: Option<Duration>,

    // Sprint 8 — per-stage averages
    pub avg_canonicalization_duration: Option<Duration>,
    pub avg_reflection_duration: Option<Duration>,
    pub avg_dispatch_duration: Option<Duration>,
}
```

---

### `daemon/src/telemetry/metrics.rs`

#### [MODIFY] Add stage timing sums and a latency reservoir

```rust
pub struct DaemonMetrics {
    // existing ...

    // Sprint 8 — per-stage sums (divide by runtime_ingest_successes for avg)
    pub runtime_canonicalization_latency_us: AtomicU64,
    pub runtime_reflection_latency_us: AtomicU64,
    pub runtime_dispatch_latency_us: AtomicU64,

    // Sprint 8 — reservoir sampler for percentile estimation
    // Protected by a Mutex; updated on each successful ingest.
    pub runtime_latency_reservoir: Mutex<LatencyReservoir>,
}
```

`LatencyReservoir` is a fixed-size (512-slot) reservoir sample over successful
runtime ingest latencies. On each successful ingest, it either fills an empty
slot or replaces a random slot (reservoir sampling algorithm). At scrape time,
sort and read p50/p95/p99.

---

### `daemon/src/server/handlers.rs`

#### [MODIFY] Extract stage timings from `IngestionResult`

```rust
Ok(result) => {
    // ... existing counters ...
    metrics.runtime_canonicalization_latency_us
        .fetch_add(result.stage_timings.canonicalization.as_micros() as u64, Ordering::Relaxed);
    metrics.runtime_reflection_latency_us
        .fetch_add(result.stage_timings.reflection.as_micros() as u64, Ordering::Relaxed);
    metrics.runtime_dispatch_latency_us
        .fetch_add(result.stage_timings.dispatch.as_micros() as u64, Ordering::Relaxed);

    let mut reservoir = metrics.runtime_latency_reservoir.lock().unwrap();
    reservoir.observe(rt_elapsed);
}
```

---

### `daemon/src/server/tcp.rs`

#### [MODIFY] `/metrics/json` — add stage timings and percentiles

```json
{
  "runtime_avg_canonicalization_us": 120.4,
  "runtime_avg_reflection_us": 210.7,
  "runtime_avg_dispatch_us": 45.2,
  "runtime_p50_latency_us": 620.0,
  "runtime_p95_latency_us": 890.0,
  "runtime_p99_latency_us": 1240.0
}
```

#### [MODIFY] `/metrics` (Prometheus) — add corresponding gauges

```text
# HELP brain_runtime_avg_canonicalization_seconds ...
# HELP brain_runtime_avg_reflection_seconds ...
# HELP brain_runtime_avg_dispatch_seconds ...
# HELP brain_runtime_p50_latency_seconds ...
# HELP brain_runtime_p95_latency_seconds ...
# HELP brain_runtime_p99_latency_seconds ...
```

#### [NEW] `/metrics/runtime` — dedicated runtime breakdown endpoint

A single endpoint that returns all runtime-specific observability data in JSON,
usable by dashboards without parsing the full `/metrics/json` blob:

```json
{
  "status": "ok",
  "ingests": { "attempts": N, "successes": N, "failures": N, "success_rate": 0.99 },
  "latency": {
    "avg_us": 680.0,
    "p50_us": 620.0,
    "p95_us": 890.0,
    "p99_us": 1240.0
  },
  "stages": {
    "canonicalization_avg_us": 120.4,
    "reflection_avg_us": 210.7,
    "dispatch_avg_us": 45.2
  },
  "note": "Sampled from independent atomics. Use trends, not single scrapes."
}
```

---

### `scripts/soak_test.py`

#### [MODIFY] Report stage breakdown when available

If `/metrics/json` includes stage fields, print them in the report:

```text
Stage Breakdown (avg over soak window)
  Canonicalization:   120.4 µs  (17.7%)
  Reflection:         210.7 µs  (31.0%)
  Dispatch:            45.2 µs   (6.6%)
  Other / overhead:   303.7 µs  (44.7%)
```

---

## Explicit Scope Exclusions

- Distributed tracing / OpenTelemetry integration (Phase 3+)
- Per-request trace spans propagated to external collectors
- Histograms with configurable buckets (use reservoir sampling instead)
- Changes to `BrainRuntime` method signatures
- Storage convergence or ingress authority decisions

---

## Success Criteria

Sprint 8 succeeds when:

1. **Stage breakdown visible**: `/metrics/runtime` returns non-zero per-stage
   averages after a soak run.

2. **Percentiles available**: `p50`, `p95`, `p99` latency values appear in the
   Prometheus endpoint and diverge meaningfully from the average (indicating
   real variance, not a flat distribution).

3. **Latency explained**: Given a `runtime_latency_ratio` of N×, an operator
   can identify which stage accounts for the majority of the excess.

4. **No regressions**: `cargo test -p brain-services` passes. Existing
   `/metrics/json` consumers are unaffected (fields are additive).

---

## Verification Plan

### Automated

```bash
cargo check -p brain-services     # IngestionResult/RuntimeMetrics additions compile
cargo check -p brain-daemon       # daemon compiles with stage extraction
cargo test -p brain-services      # full suite passes
```

### Manual

1. Start daemon: `brain daemon run`
2. Run soak: `python3 scripts/soak_test.py --count 200`
3. Query `GET /metrics/runtime` — confirm stage breakdown non-zero
4. Query `GET /metrics` — confirm p50/p95/p99 appear
5. Verify p99 > p50 (real variance present)

---

## Integration Roadmap Position

```text
✅ Architecture Phase    Sprints 1–6
── Integration Phase ───────────────────────────────────────────────────
✅ Sprint 7             Operational validation & parity metrics
→  Sprint 8             Runtime observability (stage timing, percentiles)   ← HERE
   Sprint 9+            Reliability hardening (timeouts, cancellation)
   TBD                  Ingress authority decision (evidence-driven)
   TBD                  Storage convergence (ADR + migration project)
   TBD                  Additional hosts (TUI, SDK, MCP, A2A, HTTP)
```

---

## Future Consideration — SearchRepository Abstraction

`BrainRuntime` currently holds `Arc<SqliteSearchRepository>` — a concrete type.

The correct evolution, if a second search implementation appears, is:

```rust
// Instead of:
search_repository: Arc<SqliteSearchRepository>,

// Introduce:
search_repository: Arc<dyn SearchRepository>,
```

with construction unchanged:

```rust
let search_repository = Arc::new(SqliteSearchRepository::new(pool)) as Arc<dyn SearchRepository>;
```

**When to make this change**: when a second concrete implementation exists or is
concretely planned — for example, a hybrid BM25+vector backend, a remote search
service, or a test double that requires the trait boundary to be injected.

Do not introduce this abstraction preemptively. The governing principle from
Sprint 6 applies directly: a single concrete implementation does not justify a
trait. The second case is the signal.

The comment at the `search_repository` field in
[brain_runtime.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/brain_runtime.rs)
records this decision at the definition site.
