# Milestone R25 — Distributed Runtime Architectural Retrospective

**Date:** July 25, 2026  
**Status:** Completed & Approved ✅  
**Milestone Covered:** R25 (Distributed Runtime)

---

## 1. Executive Summary

Milestone **R25 (Distributed Runtime)** successfully extended `brain`'s event-sourced execution engine into a multi-worker cluster runtime. It preserved the Phase 1 (R23/R24) stabilization boundary by keeping all networking, worker registries, candidate selection policies, and transport abstractions strictly **above** the core execution engine.

The implementation was delivered across 7 task commits, establishing:
- `WorkerDescriptor`, `WorkerStatus`, `WorkerCandidate`, and `WorkerRegistry` with monotonic clock timestamping and protocol version validation (`protocol_version == CURRENT_PROTOCOL_VERSION`).
- `trait WorkerTransport` and configurable `MockWorkerTransport` supporting transient `TaskAssignment` DTOs and failure emulation.
- Pluggable `trait SchedulingPolicy`, `LeastLoadedPolicy`, and `WorkerScheduler`.
- `CoordinatorIngressGate` validating worker heartbeats, timestamps, and health state.
- Comprehensive end-to-end distributed dispatch, mock transport, and failover recovery tests.

---

## 2. Invariants & Dependency Isolation Verification

### A. Strict Module Dependency Rules
- **Rule**: `crates/brain-services/src/distributed/` may depend on `crates/brain-services/src/runtime/`, but `runtime/` MUST NEVER depend on `distributed/`.
- **Verification**: Verified zero imports of `distributed` within `crates/brain-services/src/runtime/`. `brain-domain` remains 100% clean and untouched.

### B. Stabilization Boundary Integrity
- **Rule**: `TaskAssignment` is strictly a transient transport DTO (not persisted in SQLite tables or journal logs).
- **Verification**: Verified `TaskAssignment` is passed over `WorkerTransport` without altering `ExecutionHeader`, `JournalEvent`, or SQLite schema.

### C. Time Semantics
- **Rule**: Coordinator monotonic clock is authoritative for lease timestamps (`lease_until`, `expires_at`). Worker timestamps are strictly advisory.
- **Verification**: `WorkerRegistry::register()` receives explicit timestamps from the coordinator.

---

## 3. RFC-to-Code Implementation Evaluation

| RFC Element | Spec Intent | Implementation Reality | Assessment |
| :--- | :--- | :--- | :--- |
| **Registry vs Scheduler** | Registry owns descriptors/status; Scheduler owns candidate scoring. | `WorkerRegistry` handles worker storage/health; `WorkerScheduler` delegates placement to `SchedulingPolicy`. | **10/10** — Clean separation. |
| **Candidate View** | Pass `WorkerCandidate` view to scheduling policies. | `SchedulingPolicy::select_worker` consumes `&[WorkerCandidate]`. | **10/10** — Storage agnostic. |
| **WorkerTransport** | Decouple coordinator from RPC framework. | `trait WorkerTransport` and `MockWorkerTransport` implemented with dispatch/cancel methods. | **10/10** — Transport isolated. |
| **Coordinator Ingress Gate** | Validate worker heartbeats before event journal emission. | `CoordinatorIngressGate::process_heartbeat` validates worker health and timestamp bounds. | **10/10** — Ingress protected. |

---

## 4. Deferred Roadmap Items (Milestone R26 — Worker Runtime & Execution Engine)

The following items are deferred to **Milestone R26**:
1. **Worker Process Engine**: `TaskExecutor` trait, `ArtifactManager` (`input_ref` / `output_ref` resolution), and `CheckpointManager`.
2. **Cooperative Cancellation**: `CancellationToken` propagation across running worker tasks.
3. **Pluggable Retry Policies**: Exponential, capped, and immediate retry policies (`trait RetryPolicy`).
4. **Resource Reservation**: Reserving CPU cores, memory bytes, and GPU allocations before task execution.
