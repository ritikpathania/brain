# Milestone R25 — Distributed Runtime Architecture Specification

## Executive Summary

Milestone **R25 (Distributed Runtime)** extends `brain`'s event-sourced Execution Runtime from single-process scheduling into a multi-worker cluster runtime. It preserves the **Phase 1 (R23/R24) Stabilization Boundary** ([`docs/architecture/runtime_api_stability.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/runtime_api_stability.md)) by placing distributed coordination, worker leasing, and network transport adapters strictly **above** the core execution engine.

---

## 1. Architecture & Layering Model

The distributed runtime maintains strict layer separation:

```text
                        Cluster Coordinator
                                 │
                 WorkerScheduler (SchedulingPolicy)
                                 │
                          WorkerRegistry
                                 │
                          LeaseCoordinator
                                 │
                        ExecutionRepository
                ┌────────────────┴────────────────┐
                ▼                                 ▼
      trait LeadershipProvider          trait WorkerTransport
                │                                 │
     ┌──────────┴──────────┐           ┌──────────┴──────────┐
     │ SingleProcess       │           │ GrpcWorkerTransport │
     │ SqliteLeadership    │           │ UdsWorkerTransport  │
     │ OpenRaftLeadership  │           │ A2aWorkerTransport  │
     └─────────────────────┘           └─────────────────────┘
```

### Strict Component Responsibilities
* **`WorkerRegistry`**: Owns worker registration, protocol version validation, health tracking, descriptors, and status updates. Does **not** make scheduling or placement decisions.
* **`WorkerScheduler`**: Owns task placement, candidate scoring, resource matching, fairness, locality, and priorities via pluggable `SchedulingPolicy` strategies.
* **`LeaseCoordinator`**: Owns lease duration calculations, heartbeat tracking, and lease expiration logic. Calls `ExecutionRepository` for database persistence.
* **`ExecutionService` (Ingress Gateway)**: Validates worker `lease_id` tokens, lease ownership, and message ordering before passing facts to `ExecutionRepository` and `ExecutionAggregator`.

### Delivery & Worker Execution Guarantees
* **`TaskAssignment` DTO Contract**: `TaskAssignment` is purely a transient transport DTO between the coordinator and workers. It is not persisted and is not part of the event-sourced execution state.
* **Coordinator Monotonic Clock Authoritativeness**: All lease expiration timestamps (`lease_until`, `expires_at`) are computed exclusively by the coordinator using a monotonic clock source. Worker timestamps are strictly advisory and are not authoritative for lease expiration.
* **At-Least-Once Task Dispatch**: Task dispatch from coordinator to worker is at-least-once.
* **Idempotent Worker Execution**: Worker processes must treat `TaskAssignment` as strictly idempotent. Receiving duplicate assignments for an already executing/completed `task_id` must never trigger duplicate job handler execution.
* **Reference-Based Payloads**: Payloads use references (`input_ref`, `output_ref`, `checkpoint_id`) rather than embedding binary byte blobs in the WAL log or RPC frames.

---

## 2. Trait Abstractions & Interface Boundaries

### `WorkerTransport` Trait
```rust
use async_trait::async_trait;
use brain_domain::jobs::JobId;
use crate::runtime::models::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLease {
    pub lease_id: u64,
    pub lease_owner: String,
    pub lease_until: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: TaskId,
    pub execution_id: ExecutionId,
    pub job_id: JobId,
    pub input_ref: String,
    pub lease: TaskLease,
}

#[async_trait]
pub trait WorkerTransport: Send + Sync {
    async fn dispatch(&self, assignment: TaskAssignment) -> Result<(), TransportError>;
    async fn cancel(&self, task_id: TaskId) -> Result<(), TransportError>;
    async fn reconnect(&self) -> Result<(), TransportError>;
}
```

### `SchedulingPolicy` Trait
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerCandidate<'a> {
    pub descriptor: &'a WorkerDescriptor,
    pub status: &'a WorkerStatus,
}

pub trait SchedulingPolicy: Send + Sync {
    fn select_worker<'a>(&self, task_priority: u32, candidates: &'a [WorkerCandidate<'a>]) -> Option<&'a WorkerCandidate<'a>>;
}
```

### `LeadershipProvider` Trait
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadershipLease {
    pub leader_id: String,
    pub lease_token: String,
    pub expires_at: u64,
}

#[async_trait]
pub trait LeadershipProvider: Send + Sync {
    async fn acquire_leadership(&self) -> Result<LeadershipLease, LeadershipError>;
    async fn renew_leadership(&self, lease: &LeadershipLease) -> Result<LeadershipLease, LeadershipError>;
    async fn release_leadership(&self, lease: LeadershipLease) -> Result<(), LeadershipError>;
}
```

---

## 3. Worker Registration, Capability Discovery & Registry

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDescriptor {
    pub worker_id: String,
    pub hostname: String,
    pub protocol_version: u32,
    pub runtime_version: String,
    pub architecture: String,
    pub supported_capabilities: HashSet<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub cpu_cores: u32,
    pub memory_bytes: u64,
    pub gpu_count: u32,
    pub custom_resources: HashMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub current_load: f32,
    pub available_resources: Resources,
    pub active_lease_count: u32,
    pub is_healthy: bool,
}
```

---

## 4. Atomic Lease Management & Ingress Validation

### Persistence through `ExecutionRepository`
All lease state changes are executed through `ExecutionRepository` rather than direct raw SQL in service layers:
```rust
pub trait ExecutionRepository: Send + Sync {
    // Phase 1 methods...
    fn acquire_task_lease(&self, task_id: TaskId, worker_id: &str, duration_secs: u64) -> Result<Option<TaskLease>, RepositoryError>;
    fn validate_lease_token(&self, task_id: TaskId, lease_id: u64) -> Result<bool, RepositoryError>;
}
```

### Batch Heartbeat Protocol
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLeaseItem {
    pub task_id: TaskId,
    pub lease_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub timestamp: u64,
    pub active_leases: Vec<TaskLeaseItem>,
    pub status: WorkerStatus,
}
```

### Coordinator Ingress Gate
When a worker emits a completion or event:
```text
Worker ──► GrpcWorkerTransport ──► Coordinator Ingress ──► ExecutionService
                                                                │
                                              Validates (lease_id == task.lease_id)
                                                                │
                                                                ▼
                                                       ExecutionRepository
```

---

## 5. Coordinator Failover & Lease Reconciliation

```text
1. Coordinator Process Restarts
       │
       ▼
2. Startup Recovery Engine Replays Journal from SQLite WAL
       │
       ▼
3. WorkerRegistry Reconciliation Pass:
   Coordinator queries active_leases from connected workers.
       │
       ▼
4. Categorize Leases Deterministically:
   • LeaseMatches     ──► Retain active task lease in memory.
   • LeaseExpired     ──► Emit LeaseExpired event; reschedule task as Ready.
   • UnknownLease     ──► Send Cancel signal to worker.
   • MissingLease     ──► Re-assign lease to worker if repository valid.
   • OrphanedTask     ──► Mark task Ready for new worker assignment.
```

---

## 6. Implementation Sequencing Strategy

1. `WorkerDescriptor`, `WorkerStatus`, and `WorkerRegistry`
2. `WorkerTransport` trait + Mock Transport
3. `LeaseCoordinator` & `ExecutionRepository` lease methods
4. `TaskAssignment` protocol & Worker Idempotency rules
5. `ExecutionService` Ingress Validation
6. `SchedulingPolicy` & `WorkerScheduler`
7. `GrpcWorkerTransport` implementation
8. Coordinator Startup Reconciliation
9. Network Partition & Failure Injection Integration Tests
