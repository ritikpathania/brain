# Sprint 7 — Runtime Operational Validation

## Objective

Answer a single question with production-quality evidence:

> **Can we demonstrate, with telemetry and parity metrics, that `BrainRuntime` is ready
> to become the authoritative ingestion engine?**

This is not an implementation sprint. It is a measurement sprint. No new runtime APIs.
No storage changes. No migration decisions.

---

## Context

Sprint 6 established dual ingestion:

```text
session.ingest(payload)     ← legacy-first: authoritative
        ↓
runtime.ingest(obs)         ← alongside: non-fatal, currently invisible
```

The runtime call currently produces log output only. No counter tracks how often it
succeeds, fails, or how long it takes relative to the legacy path. Without that data,
the ingress authority decision (Phase 4 in the integration roadmap) is a judgment call.
Sprint 7 makes it an engineering decision.

---

## Architecture

No changes to `BrainRuntime`. All instrumentation is in the daemon layer:

```text
Daemon (instrumentation layer)
    │
    ├── DaemonMetrics            ← add 4 parity counters
    ├── handlers.rs              ← record counters around runtime.ingest()
    └── /metrics HTTP endpoint   ← expose parity counters alongside legacy metrics

BrainRuntime                    ← unchanged
```

The governing principle from Sprint 6 applies: hosts adapt to the runtime, not the reverse.
Sprint 7 observes the runtime — it does not modify it.

---

## Proposed Changes

### `daemon/src/telemetry/`

#### [MODIFY] `DaemonMetrics`

Add four parity counters alongside the existing ingestion counters:

```rust
pub struct DaemonMetrics {
    // existing
    pub total_ingests: AtomicU64,
    pub sum_ingest_latency_us: AtomicU64,
    pub active_workers: AtomicU64,
    pub total_queries: AtomicU64,

    // Sprint 7 — runtime parity
    pub runtime_ingest_attempts: AtomicU64,
    pub runtime_ingest_successes: AtomicU64,
    pub runtime_ingest_failures: AtomicU64,
    pub runtime_ingest_latency_us: AtomicU64,  // sum, same unit as legacy
}
```

---

### `daemon/src/server/handlers.rs`

#### [MODIFY] `ingest` arm — instrument `runtime.ingest()`

Replace the current silent match with a timed, counted call:

```rust
{
    let rt_start = Instant::now();
    metrics.runtime_ingest_attempts.fetch_add(1, Ordering::Relaxed);

    match brain_runtime.ingest(obs) {
        Ok(result) => {
            let rt_elapsed = rt_start.elapsed().as_micros() as u64;
            metrics.runtime_ingest_successes.fetch_add(1, Ordering::Relaxed);
            metrics.runtime_ingest_latency_us.fetch_add(rt_elapsed, Ordering::Relaxed);
            info!(
                component = "runtime",
                epoch = result.epoch.0,
                entities = result.affected_entities.len(),
                latency_us = rt_elapsed,
                "BrainRuntime ingestion succeeded"
            );
        }
        Err(e) => {
            metrics.runtime_ingest_failures.fetch_add(1, Ordering::Relaxed);
            warn!(
                component = "runtime",
                error = %e,
                "BrainRuntime ingestion failed (non-fatal — STM path succeeded)"
            );
        }
    }
}
```

---

### `daemon/src/server/tcp.rs`

#### [MODIFY] `/metrics` response — add derived parity values

Extend the existing metrics response:

```text
# existing
total_ingests          N
avg_ingest_latency_us  N

# Sprint 7 additions
runtime_ingest_attempts      N
runtime_ingest_successes     N
runtime_ingest_failures      N
runtime_ingest_success_rate  N%        (successes / attempts)
runtime_avg_latency_us       N
legacy_avg_latency_us        N         (existing counter, renamed for clarity)
runtime_latency_ratio        N.Nx     (runtime / legacy — 1.0x = parity)
```

> **Snapshot consistency**: metrics are sampled from independent atomic counters.
> Individual scrapes may capture an in-flight request and therefore represent a transient
> snapshot. Long-term trends and rates — not single samples — should be used for
> migration decisions.

---

### `scripts/` or `daemon/tests/`

#### [NEW] Soak test harness

A script or integration test that:

1. Connects to a running daemon via UDS socket.
2. Sends N observations over a sustained period (e.g., 1000 over 60 seconds).
3. Queries `/metrics` on completion.
4. Prints a parity summary:

```text
Soak Test Results (1000 observations, 60s)
──────────────────────────────────────────
Legacy ingests:          1000 / 1000  (100.0%)
Runtime attempts:        1000 / 1000  (100.0%)
Runtime successes:        997 / 1000  (99.7%)
Runtime failures:           3 / 1000   (0.3%)

Latency (avg)
  Legacy path:    420 µs
  Runtime path:   680 µs
  Ratio:          1.62x

Divergence:       not yet measured (requires projection comparison)
```

---

## Stretch Goal — Divergence Comparison (Phase 2+)

Divergence between the two ingestion paths cannot be answered by counters alone. The
long-term approach is a canonical digest comparison:

```text
Legacy projection
        │
        ▼
  canonical digest

Runtime projection
        │
        ▼
  canonical digest

      compare
```

A deterministic digest of a canonical entity representation scales to millions of
observations without structural comparison overhead. It also degrades gracefully —
a hash mismatch is a signal to inspect, not a test assertion.

This is explicitly Phase 2+ work. Sprint 7 records divergence as "not yet measured"
in the soak summary and defers the comparison mechanism until the reliability and
latency questions are answered first.

---

## Explicit Scope Exclusions

The following are out of scope until this sprint's evidence justifies them:

- New `BrainRuntime` APIs or internal changes
- Storage schema changes or migration work
- Making runtime ingestion authoritative
- Per-stage timing breakdown (canonicalize / reflect / dispatch) — Phase 2
- Reliability hardening: shutdown timeout, handler registry, cancellation — Phase 3

---

## Success Criteria

Sprint 7 succeeds when all three questions can be answered with data:

| Question | How answered |
|---|---|
| **Is the runtime reliable?** | `runtime_ingest_success_rate` measured under soak load |
| **Is it slower?** | `runtime_latency_ratio` measured; acceptable delta defined |
| **Does it diverge from legacy?** | No errors at minimum; projection comparison as stretch goal |

A fourth optional criterion: the `/metrics` endpoint returns parity data in a format
suitable for a dashboard or log aggregator — no manual log parsing required.

---

## Verification Plan

### Automated

```bash
cargo check -p brain-daemon     # must compile clean after counter additions
cargo test -p brain-services    # existing suite must remain passing
```

### Manual

1. Start daemon: `brain daemon run`
2. Send 100 observations via UDS socket
3. Query `GET /metrics` — verify all four parity counters are non-zero
4. Run soak test harness for 60s; review printed parity summary

---

## Integration Roadmap Position

```text
✅ Architecture Phase    Sprints 1–6
── Integration Phase ───────────────────────────────────────────────────
→  Sprint 7             Operational validation & parity metrics     ← HERE
   Sprint 8+            Observability improvements (per-stage timing)
   Sprint 9+            Reliability hardening (timeouts, cancellation)
   TBD                  Ingress authority decision (evidence-driven)
   TBD                  Storage convergence (ADR + migration project)
   TBD                  Additional hosts (TUI, SDK, MCP, A2A, HTTP)
```
