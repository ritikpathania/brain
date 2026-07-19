# Sprint 6 — Hosting the Runtime

## Goal

Wire `BrainRuntime` into the daemon as the authoritative host for the observation pipeline.
The daemon becomes a thin adapter: it listens for requests, delegates observation to the
runtime, and returns results. It does not assemble services.

---

## Architecture

Sprint 6 validates the three-layer model that has emerged over Sprints 1–5:

```text
Host                   —  process lifecycle, signal handling, request acceptance,
    │                      request draining, runtime ownership
    ▼
BrainRuntime           —  construction, orchestration, lifecycle, observability,
    │                      execution; the API surface for all hosts
    ▼
Runtime Services       —  storage, canonicalization, reflection, projection, dispatch
                           (internal; no host touches these directly)
```

Each layer has exactly one responsibility. The boundary between Host and BrainRuntime is
the API surface being validated in Sprint 6. If that boundary holds under a real production
host without requiring changes to `BrainRuntime`, the architecture is confirmed stable.

---

## Status Before Sprint 6

The runtime architecture is complete and validated:

```text
BrainRuntime
    │
    ├── deterministic construction   ✓  (new())
    ├── deterministic execution      ✓  (ingest(), query_projection())
    ├── deterministic shutdown       ✓  (shutdown(self) → ShutdownSummary)
    ├── validated dependency graph   ✓  (26 tests, 0 failures)
    ├── Send + Sync                  ✓  (compile-time assertion)
    ├── introspection API            ✓  (status(), metrics(), diagnostics())
    └── lifecycle states             ✓  (Initializing → Healthy → ShuttingDown → Stopped)
```

What is not yet validated:
- long-running execution
- concurrent requests
- sustained event dispatch
- SQLite pool pressure under real load
- production startup and shutdown

These are exercised by hosting the runtime in the daemon.

---

## Constraint

> The daemon must not assemble services itself.

The daemon's role after Sprint 6:

```text
main()
  │
  ├── resolve config + paths
  ├── BrainRuntime::new(db_path)
  │
  ├── serve requests   ←── delegate observation to runtime.ingest()
  │
  └── let summary = BrainRuntime::shutdown()
      └── log summary.duration, export to telemetry, or discard
```

If `BrainRuntime` must change to support the daemon, that is a design finding worth
reviewing before implementing. The runtime API should absorb the host without changing.

---

## Proposed Changes

---

### `daemon/Cargo.toml`

#### [MODIFY] Cargo.toml

Add `brain-services` as a dependency. The daemon crate currently uses `LtmDatabase`
from its own storage layer — this sprint adds the runtime as an additional dependency
alongside the existing stack (no removals in Sprint 6).

```toml
[dependencies]
brain-services = { path = "../crates/brain-services" }
```

---

### `daemon/src/main.rs`

#### [MODIFY] main.rs

Two targeted changes only:

**1. Construct the runtime at startup.**

After `LtmDatabase` initialization (which already reads `paths.db_path`), construct
`BrainRuntime` directly. The daemon passes the resolved DB path and lets the runtime own
all service wiring:

```rust
let brain_runtime = BrainRuntime::new(paths.db_path.to_str().unwrap())
    .unwrap_or_else(|e| {
        error!(component = "runtime", "Failed to initialize BrainRuntime: {}", e);
        std::process::exit(1);
    });
let brain_runtime = Arc::new(brain_runtime);
```

**2. Shut down the runtime on exit via `BrainRuntimeHost`.**

Handlers receive `Arc<BrainRuntime>` (immutable, shared). The host is the sole owner of the
`BrainRuntime` value and the sole caller of `shutdown(self)`. This keeps lifecycle control
out of request handlers:

```rust
// On SIGTERM / shutdown signal:
// Arc::try_unwrap() succeeds once all handler tasks holding Arc<BrainRuntime> have finished.
let runtime = Arc::try_unwrap(brain_runtime).expect("All handlers must have dropped by shutdown");
let summary = runtime.shutdown().expect("BrainRuntime shutdown failed");
info!(component = "runtime", duration_ms = summary.duration.as_millis(), "Runtime stopped");
```

> [!NOTE]
> `shutdown(self)` consumes the runtime — this is intentional. It makes post-shutdown use a
> compile error. The `BrainRuntimeHost` pattern (host owns the value, handlers share `Arc`)
> is the correct way to resolve the ownership question without introducing `Arc<Mutex<Option<_>>>`.
>
> **Implementation detail**: `Arc::try_unwrap()` succeeds only when the refcount reaches
> exactly one — meaning every clone has been dropped. This includes not just request
> handlers but **any task that clones `Arc<BrainRuntime>`**: periodic maintenance workers,
> telemetry aggregators, background compaction, etc. Treat "anything that clones the runtime"
> as participating in the drain protocol. If a background task is running at shutdown time
> and holds an `Arc`, `try_unwrap()` will panic. The host must ensure all such tasks have
> terminated before calling `try_unwrap()`.

---

### `daemon/src/` (new)

#### [NEW] `brain_runtime_host.rs` (or inline in `main.rs`)

A minimal handler that routes ingestion requests through the runtime:

```rust
async fn handle_observe(
    runtime: Arc<BrainRuntime>,
    obs: Observation,
) -> Result<CanonicalizationResult, BrainError> {
    // Delegate entirely — no service assembly here
    runtime.ingest(obs)
}
```

This is the full extent of what the daemon contributes to the ingestion path.

---

## Resolved Questions

**Q1: Alongside or replacing `LtmDatabase`?**
→ **Alongside.** `BrainRuntime` owns ingestion. The daemon's existing stack owns retrieval
and analytics. No removals in Sprint 6. The boundary between the two systems will emerge
naturally over several iterations.

**Q2: Is `BrainRuntime` `Send + Sync`?**
→ **Yes, proven at compile time.** A zero-cost `const _: fn()` assertion in `brain_runtime.rs`
verifies this permanently. All fields — `Arc<dyn Storage>`, `Arc<AtomicU8>`, `Arc<Mutex<...>>`
— satisfy the guarantee. No changes needed for `Arc<BrainRuntime>` in tokio tasks.

**Q3: How does `shutdown(self)` work with `Arc` shared across handlers?**
→ **`BrainRuntimeHost` pattern.** Separate execution lifetime from object lifetime.
Handlers receive `Arc<BrainRuntime>` (immutable, shared). Only one owner — the host —
holds the `BrainRuntime` itself and calls `shutdown(self)`. This avoids `Arc<Mutex<Option<BrainRuntime>>>`
and preserves the compile-time guarantee that no work is done after shutdown.

**Q4: Builder or `new()`?**
→ **`new(db_path)`.** One topology exists. A builder is appropriate only when a second
configuration genuinely exists. Builder was removed; `new()` is the single constructor.

**Q5: What does `shutdown()` return?**
→ **`Result<ShutdownSummary, BrainError>`.** Terminal information (shutdown duration)
belongs to the caller. `RuntimeDiagnostics` is live-state only — things that make sense
while the runtime exists. Writing a summary to `diagnostics` that gets dropped in the same
call is an API smell. The daemon decides whether to log, export, or discard the summary.

---

## What Sprint 6 Is Not

- Not a full daemon rewrite. Retrieval, analytics, consolidation, and plugin systems are
  out of scope.
- Not a removal of `LtmDatabase`. The two storage systems coexist for now.
- Not adding new runtime capabilities. The runtime API must remain unchanged.

---

## Verification Plan

### Automated

The existing 26-test suite is the baseline. Sprint 6 adds:

```bash
cargo check -p daemon  # must compile clean after adding brain-services dependency
```

**Shutdown protocol test** — proves the host contract, not just the happy path:

```rust
// Conceptual structure (daemon integration test or separate harness):
//
// 1. Construct BrainRuntime, wrap in Arc.
// 2. Spawn a handler task that clones the Arc and holds it for a bounded duration.
// 3. Initiate shutdown signal.
// 4. Wait for the handler task to complete (it releases its Arc clone on exit).
// 5. Arc::try_unwrap() must succeed — if it panics, a handler leaked its reference.
// 6. runtime.shutdown() completes and returns ShutdownSummary.
//
// This proves: shutdown begins only after all outstanding Arc<BrainRuntime>
// references have been released. It is the invariant that makes try_unwrap() correct.
```

### Manual

Start the daemon, send an observation via the UDS socket, verify:
1. `BrainRuntime::ingest()` is called (log + observability span)
2. Events dispatched to subscriber
3. Daemon shuts down cleanly — `ShutdownSummary` returned and logged
4. No thread leaks, no open DB handles

---

## Success Criteria

Sprint 6 succeeds if and only if all of the following hold:

1. **The daemon constructs `BrainRuntime` but does not assemble its internal services.**
   - No `SqliteCanonicalizer`, `SqliteReflectionEngine`, or `InMemoryEventDispatcher` appear in
     daemon code. All wiring stays inside `BrainRuntime::new()`.

2. **Request handlers interact only with `Arc<BrainRuntime>`.**
   - Handlers call `runtime.ingest()`. They hold no other reference to runtime internals.

3. **Runtime APIs remain unchanged throughout daemon integration.**
   - If `BrainRuntime` must be modified to support the daemon, that is a design finding
     that should be reviewed before proceeding.

4. **Shutdown is coordinated solely by the host.**
   - Host shutdown begins only after all request handlers have completed and released
     their `Arc<BrainRuntime>` references.
   - `Arc::try_unwrap()` is the enforcement mechanism: it panics rather than silently
     proceeding if any handler has leaked its reference.
   - `runtime.shutdown()` is called exactly once, by the host, on the unwrapped value.
   - The returned `ShutdownSummary` is the caller's property to log, export, or discard.

5. **Existing retrieval and analytics paths remain unaffected.**
   - The daemon's `LtmDatabase`, plugin registry, and retrieval systems continue to work.
   - No behaviour changes outside the ingestion path.

If all five properties hold, the runtime architecture is mature enough to support additional
hosts — TUI, SDKs, protocol adapters — without revisiting its core design.

---

## Future Consideration — Forced Shutdown

The current host model assumes **graceful shutdown**:

```text
stop accepting requests
    ↓
drain in-flight handlers (all Arc<BrainRuntime> references released)
    ↓
Arc::try_unwrap() — ownership recovered by host
    ↓
runtime.shutdown() → ShutdownSummary
```

`Arc::try_unwrap()` naturally enforces a **drain-before-shutdown** model, which is generally
the correct default. However, it does not yet account for:

- **Forced shutdown** — SIGKILL, `process::exit()`, or operator override
- **Handler timeouts** — a stalled handler never releases its `Arc`
- **Panic during request handling** — a panicking task may or may not drop its `Arc`
- **Cancellation** — async tasks cancelled mid-flight

At some future point, the system will likely need to distinguish between two shutdown modes:

- **Graceful shutdown** — the current protocol: drain, unwrap, shutdown, return summary.
- **Abortive shutdown** — best-effort cleanup before process termination. Useful when forced
  termination is unavoidable (operator SIGKILL, timeout exceeded). Does not guarantee a
  `ShutdownSummary`. Accepts resource leaks in exchange for speed.

There is not yet enough complexity in the runtime to justify designing the abortive path.
Document it here so it is not forgotten. When it is addressed, the correct mechanism is a
**timeout on the drain phase** with an explicit fallback: either a handler registry that can
be cancelled individually, or an abortive `shutdown()` variant that does not wait for all
`Arc` references to be released.

---

## Sprint 6 Outcome

All five success criteria satisfied. One design finding surfaced.

| Criterion | Result |
|---|---|
| Daemon does not assemble services | ✅ |
| Handlers interact only with `Arc<BrainRuntime>` | ✅ |
| Runtime APIs unchanged during integration | ✅ |
| Shutdown coordinated solely by host → `ShutdownSummary` | ✅ |
| Existing retrieval and analytics unaffected | ✅ |

**Design Finding #1 — Storage schema conflict**: `LtmDatabase` and `SqliteStorage` (inside
`BrainRuntime`) both define `nodes`, `edges`, and `event_log` tables with incompatible
schemas. They cannot share a database file. `BrainRuntime` uses `~/.brain/brain_runtime.db`;
`LtmDatabase` continues on `~/.brain/memory.db`. This is an integration concern, not a
runtime defect.

**Validation**:
```
cargo check -p brain-daemon     ✅  clean
cargo test -p brain-services    ✅  full suite passing
```

The runtime architecture is confirmed stable. It hosted a production daemon without requiring
changes to its public API or leaking internal composition into the host.

**Architectural maturity**:

| Layer | Status |
|---|---|
| **Runtime architecture** | Mature and validated — changes require strong justification |
| **System integration** | Transitional and intentionally evolving — changes are expected |

This distinction matters for evaluating future work. Changes inside `BrainRuntime` should
be held to a higher bar than changes around it. Daemon integration, storage migration, and
protocol adapters are the expected evolution surface.

---

## Post-Sprint 6 Architectural Debt

Sprint 6 proved the runtime boundaries hold. The remaining debt has shifted entirely to
**system integration** — how the runtime and the legacy stack converge over time.

### 1. Storage Convergence

Two SQLite databases. Two schemas. Both named `nodes`, `edges`, `event_log` — with
different column definitions.

- `~/.brain/memory.db` — `LtmDatabase` (daemon legacy stack)
- `~/.brain/brain_runtime.db` — `SqliteStorage` (BrainRuntime knowledge graph)

**What this means**: memories ingested through the runtime are not yet queryable by the
daemon's retrieval and analytics systems. Unifying the two requires a migration strategy —
either adapting one schema to the other, or introducing a storage abstraction that both
layers share.

**When to address**: after the runtime ingestion path has proven itself in production.
Do not migrate prematurely. When the time comes, treat this as an explicit migration project
with its own ADR rather than folding it into feature work. The schema divergence reflects
a genuine domain difference between the two persistence layers — that context should be
captured in the decision record, not lost in a refactor commit.

---

### 2. Ingress Authority

Dual ingestion currently exists in the daemon's `ingest` handler:

```text
session.ingest(payload)     ← legacy-first: authoritative
        ↓
runtime.ingest(obs)         ← alongside: non-fatal
```

This is a deliberate migration strategy, not a permanent design. Eventually one path will
become authoritative. Three possible resolutions, each with different implications:

- **Legacy-first** *(current)*: daemon behavior succeeds if the legacy path succeeds.
  Runtime failures are silent. Suitable for migration.
- **Runtime-first**: runtime becomes the canonical ingestion path. The legacy path becomes
  an adapter or disappears. Suitable once the runtime is proven under production load.
- **Transactional**: success requires coordinated completion of both paths. Suitable if
  both stores must remain consistent (e.g., before migration is complete).

**When to decide**: after observing runtime behavior under real load. The decision should be
driven by evidence, not schedule. Useful parity metrics to collect during the dual-ingestion
phase:

| Metric | Purpose |
|---|---|
| Runtime ingest attempts | Confirm runtime is receiving all requests |
| Runtime ingest success / failure rate | Establish reliability baseline |
| Runtime ingest latency vs legacy path | Detect regressions before migration |
| Divergence between the two systems | Detect correctness gaps early |

These measurements make the eventual authority decision objective. If the runtime shows
high parity and comparable latency under production load, moving to runtime-first is
low-risk. If failures or divergence are observed, that is the signal to investigate before
committing to a migration.

---

### 3. Operational Lifecycle

Graceful shutdown is implemented and validated. Abortive shutdown remains intentionally
deferred (documented in the "Future Consideration — Forced Shutdown" section above).

These are evolution concerns, not flaws in the runtime. The runtime itself is complete.

---

## Looking Ahead

The next architectural milestones are no longer about *creating* abstractions — they are
about validating and extending a stable one:

1. **Operational validation** — observe runtime behavior under sustained production load.
   The dual-ingestion phase provides the evidence base. Collect parity metrics.

2. **Migration decisions** — use collected metrics to determine when, or if, runtime-first
   ingestion is appropriate. The decision should be driven by measurement, not schedule.

3. **Convergence work** — unify storage and retire legacy paths through explicit migration
   projects, each backed by its own ADR and a clear rationale.

These are fundamentally different activities from the work completed in Sprints 1–6.

Sprints 1–6 established the core architecture. What follows is evolving the system around
that core — adapting the surrounding infrastructure to meet a stable interior, rather than
continuing to build the interior itself. That transition is the natural conclusion of a
successful architecture phase.

---

## Governing Principle

The following principle now implicitly governs future work:

> **Hosts should adapt to the runtime unless evidence demonstrates that the runtime
> abstraction is insufficient.**

This is not the same as "never change the runtime." There are legitimate architectural
reasons to evolve it:

- A new capability belongs naturally inside the runtime.
- Multiple independent hosts expose the same limitation.
- The abstraction itself proves incomplete under real-world use.

Those are reasons grounded in the runtime's own design.

By contrast, *"the daemon would be easier if the runtime exposed X"* is not, by itself,
sufficient justification. Sprint 6 demonstrated that at least one production host could
integrate without requiring changes to the runtime API. That raises the bar. Future
modifications to `BrainRuntime`'s public surface should be evaluated against this standard,
not against the convenience of a single host.

---

## Phase Transition

The project has crossed a natural boundary:

```text
Architecture Phase
──────────────────────────────────────────────────────
Sprints 1–4     Runtime services (storage, canonicalization, reflection, events)
Sprint 5        Runtime composition (BrainRuntime — deterministic construction & shutdown)
Sprint 5.5      Runtime lifecycle & introspection (status, metrics, ShutdownSummary)
Sprint 6        Runtime hosting validation (daemon integration, all five criteria satisfied)

        ↓

Integration Phase
──────────────────────────────────────────────────────
✅ Sprint 7     Operational validation & parity metrics

        ↓ Evidence Review ↓
        Did we preserve the runtime boundary?
        Did telemetry answer the sprint's question?
        What architectural assumptions changed?

🚧 Sprint 8     Runtime observability (stage timing, percentiles, /metrics/runtime)

        ↓ Evidence Review ↓
        Long-duration soaks (10k, 100k observations)
        Concurrent ingestion & search
        Burst and idle profiles
        Identify the first real bottleneck

Sprint 9        Targeted reliability hardening (only what evidence indicates)
                Are shutdowns deterministic? Do queues back up? Is SQLite the bottleneck?

        ↓ Evidence Review ↓
        Ingress authority decision:
        "Should runtime become authoritative?" (not "can it?")

TBD             Storage convergence — ADR + migration project
                Schema comparison, migration tooling, compatibility, rollback

        ↓

TBD             Additional hosts (TUI, SDK, MCP, A2A, HTTP)
                All consume BrainRuntime once storage is settled
```

The primary risk changes at this boundary:

- **Architecture phase**: the risk was designing the wrong abstraction. The failure mode
  was building something coherent in isolation that couldn't be hosted or extended.

- **Integration phase**: the risk is migrating surrounding systems without losing
  correctness or operational stability. The failure mode is coupling the runtime to
  legacy concerns, or migrating faster than the evidence supports.

Those require different engineering decisions and different review criteria. Work in the
integration phase should be evaluated on correctness, observability, and reversibility.
Work that would modify the runtime itself still requires architectural justification.

**Architecture review gates** are inserted between each sprint. For each completed sprint,
verify:

1. Did we preserve the runtime boundary?
2. Did we introduce unnecessary coupling?
3. Is any new abstraction actually justified?
4. Did telemetry answer the question the sprint set out to answer?
5. What architectural assumptions changed?

The roadmap after Sprint 9 is intentionally unscheduled. It will be determined by the
evidence Sprint 8 and Sprint 9 produce, not by the calendar.
