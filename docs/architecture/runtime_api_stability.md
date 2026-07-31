---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Execution Runtime API Stability & Contract Classification

**Status:** Active Governance Document  
**Milestone Scope:** Phase 1 (R23/R24 Engine)  
**Target Audience:** `brain` core contributors & subagent workers

---

## 1. Governance Overview

This document classifies the **Execution Runtime** components, APIs, and data models into explicit stability tiers. The contracts established in Phase 1 (R23/R24) serve as the **Stabilization Boundary** for the platform. 

Downstream features and future milestones (e.g., Milestone R25 Distributed Runtime) must treat **Stable** contracts as immutable boundaries. Additive evolution (introducing new event variants or storage adapters) is strictly preferred over mutating established contracts.

---

## 2. API & Component Stability Classification

| Component / Subsystem | Stability Tier | Evolution & Change Governance Rules |
| :--- | :--- | :--- |
| **Runtime Models** (`ExecutionId`, `TaskId`, `ExecutionHeader`, identity fields) | **Stable** | Structural signatures are frozen. Identity headers support nested tracing. |
| **Finite State Machines** (`ExecutionFsmState`, `TaskFsmState`) | **Stable** | Allowed transitions and terminal state checks are immutable. |
| **Journal Format & Schema** (`JournalEvent`, `SequenceNo`, `ExecutionVersion`) | **Stable** | Immutable past-tense fact schema. `sequence_no` and `version` validation rules are frozen. |
| **Repository Boundary** (`trait ExecutionRepository`) | **Stable** | Core interface for persistence. Implementations must comply with contract tests. |
| **Aggregator Projection** (`ExecutionAggregator`) | **Stable** | Pure, side-effect-free projection engine. Deterministic replay contract is immutable. |
| **Recovery Protocol** (`RecoveryEngine`) | **Stable** | Checkpoint snapshot + incremental journal sequence replay protocol is frozen. |
| **SQLite Schema & DDL** (`SqliteExecutionRepository`) | **Internal** | Table layout, SQL indexes, and query optimizations may be tuned internally. |
| **Transport Telemetry Events** (Ephemeral UI/Stream Events) | **Additive** | Ephemeral event stream variants may be expanded freely without polluting the WAL. |
| **Scheduler & Lease Engine** | **Evolvable** | Operational worker assignment, priority queues, and lease expiry strategies evolve into R25. |

---

## 3. Immutable Contracts Summary

### A. Runtime & Identity Contract
- `Execution` owns a Directed Acyclic Graph (DAG) of schedulable `Task` vertices.
- Tasks never reference other tasks directly; graph edges are managed independently.
- `brain-domain` entities (`Job`, `JobId`) remain 100% pure and free of runtime orchestration logic.

### B. Event-Sourced Journal Contract
- `sequence_no` is strictly monotonic per execution instance.
- Events are strictly append-only; historical entries are never modified or reordered.
- Event payloads use references (`output_ref`, `checkpoint_id`) rather than embedding binary blobs in the WAL log.

### C. Aggregator Contract
- `ExecutionAggregator` is a pure projection over `JournalEvent` streams.
- Replay verifies `event.version == expected_version`. The aggregator makes zero side effects or network calls.

### D. Recovery Contract
- Recovery sequence: Load checkpoint snapshot sequence $N$ $\rightarrow$ replay journal events where `sequence_no > N` $\rightarrow$ reconstruct projection. No heuristics or best-effort fallbacks.

---

## 4. Architectural Layering for R25 Distributed Execution

All future multi-node distributed capabilities (Milestone R25) sit strictly **above** the runtime engine as external adapters and lease coordinators:

```text
                       Milestone R25 Layer
                 ┌──────────────────────────────┐
                 │     Cluster Coordinator      │
                 │   Remote Worker Scheduler    │
                 │   Lease Coordination Layer   │
                 └──────────────┬───────────────┘
                                │ (Consumes)
                                ▼
                       Stabilization Boundary
                 ┌──────────────────────────────┐
                 │     ExecutionRepository      │
                 │  Execution Runtime (Engine)  │
                 │ SQLite WAL & Journal Stream  │
                 └──────────────────────────────┘
```

This ensures that distributed coordination complexity never leaks into the core single-node execution engine.
