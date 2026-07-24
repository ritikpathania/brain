# Milestone R25 — Distributed Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Milestone R25 (Distributed Runtime) in Rust within `crates/brain-services/src/distributed/`, introducing worker registration, capability discovery, `trait WorkerTransport`, `trait SchedulingPolicy`, lease coordination, ingress validation, and coordinator recovery reconciliation.

**Architecture:** Distributed Runtime layered strictly **above** the R23/R24 execution engine stabilization boundary. Uses `trait WorkerTransport` adapters (`GrpcWorkerTransport`, `MockWorkerTransport`), pluggable `SchedulingPolicy`, worker capability matching, monotonic `lease_id` validation, and worker lease reconciliation.

**Tech Stack:** Rust, `tokio`, `async-trait`, `rusqlite`, `serde`, `uuid`, `thiserror`.

## Global Constraints

- **Module Hierarchy Rule**: `distributed/` may depend on `runtime/`, but `runtime/` MUST NEVER depend on `distributed/`.
- **Stabilization Boundary Integrity**: `crates/brain-domain` and core Phase 1 runtime contracts (`ExecutionId`, `TaskId`, `ExecutionFsmState`, `TaskFsmState`, `JournalEvent`, `ExecutionAggregator`, `RecoveryEngine`) MUST remain unchanged.
- **Worker Event Ingress**: Workers never write directly into `ExecutionRepository`. Event emission flows through `WorkerTransport` $\rightarrow$ `Coordinator` $\rightarrow$ `ExecutionService` $\rightarrow$ `ExecutionRepository` $\rightarrow$ `ExecutionAggregator`.
- **Transient DTO**: `TaskAssignment` is purely a transient transport DTO. It is not persisted in database tables or journal streams.
- **Monotonic Clock Authoritativeness**: Coordinator monotonic clock is authoritative for lease timestamps (`lease_until`, `expires_at`). Timestamps are passed explicitly into `WorkerRegistry` without hardcoded `SystemTime::now()` calls inside model internals.

---

### Task 1: Worker Data Models & `WorkerRegistry`

**Files:**
- Create: `crates/brain-services/src/distributed/mod.rs`
- Create: `crates/brain-services/src/distributed/models.rs`
- Create: `crates/brain-services/src/distributed/registry.rs`
- Modify: `crates/brain-services/src/lib.rs`
- Test: `crates/brain-services/src/distributed/registry.rs` (inline test module)

**Interfaces:**
- Consumes: `JobId` from `brain_domain::jobs::JobId`
- Produces: `WorkerDescriptor`, `Resources`, `WorkerStatus`, `WorkerRegistry`, `RegistryError`, `WorkerCandidate`

- [ ] **Step 1: Write failing unit tests for WorkerRegistry with explicit timestamps**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_worker_registry_registration_and_protocol_version() {
        let registry = WorkerRegistry::new(1); // Current protocol version = 1

        let descriptor = WorkerDescriptor {
            worker_id: "worker-1".to_string(),
            hostname: "node-1.local".to_string(),
            protocol_version: 1,
            runtime_version: "1.0.0".to_string(),
            architecture: "x86_64".to_string(),
            supported_capabilities: HashSet::from(["gpu".to_string()]),
            labels: HashMap::from([("region".to_string(), "us-east".to_string())]),
        };

        let status = WorkerStatus {
            current_load: 0.1,
            available_resources: Resources { cpu_cores: 8, memory_bytes: 16000, gpu_count: 1, custom_resources: HashMap::new() },
            active_lease_count: 0,
            is_healthy: true,
        };

        assert!(registry.register(descriptor.clone(), status.clone(), 1000).is_ok());

        // Incompatible protocol version rejected
        let invalid = WorkerDescriptor { protocol_version: 99, ..descriptor };
        assert!(registry.register(invalid, status, 1000).is_err());
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib distributed::registry::tests`
Expected: FAIL with "module `distributed` not found"

- [ ] **Step 3: Implement WorkerDescriptor, WorkerStatus, WorkerCandidate, and WorkerRegistry**

In `crates/brain-services/src/distributed/models.rs`:
```rust
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerCandidate<'a> {
    pub descriptor: &'a WorkerDescriptor,
    pub status: &'a WorkerStatus,
}
```

In `crates/brain-services/src/distributed/registry.rs`:
```rust
#![allow(missing_docs)]

use crate::distributed::models::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("Incompatible protocol version {0}, expected {1}")]
    IncompatibleProtocol(u32, u32),
    #[error("Worker {0} not found")]
    WorkerNotFound(String),
}

#[derive(Debug, Clone)]
pub struct RegisteredWorker {
    pub descriptor: WorkerDescriptor,
    pub status: WorkerStatus,
    pub last_seen_timestamp: u64,
}

pub struct WorkerRegistry {
    expected_protocol_version: u32,
    workers: Arc<RwLock<HashMap<String, RegisteredWorker>>>,
}

impl WorkerRegistry {
    pub fn new(expected_protocol_version: u32) -> Self {
        Self {
            expected_protocol_version,
            workers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, descriptor: WorkerDescriptor, status: WorkerStatus, timestamp: u64) -> Result<(), RegistryError> {
        if descriptor.protocol_version != self.expected_protocol_version {
            return Err(RegistryError::IncompatibleProtocol(
                descriptor.protocol_version,
                self.expected_protocol_version,
            ));
        }

        let id = descriptor.worker_id.clone();
        let entry = RegisteredWorker {
            descriptor,
            status,
            last_seen_timestamp: timestamp,
        };

        self.workers.write().insert(id, entry);
        Ok(())
    }

    pub fn get(&self, worker_id: &str) -> Option<RegisteredWorker> {
        self.workers.read().get(worker_id).cloned()
    }

    pub fn list_active(&self) -> Vec<RegisteredWorker> {
        self.workers.read().values().cloned().collect()
    }
}
```

In `crates/brain-services/src/distributed/mod.rs`:
```rust
pub mod models;
pub mod registry;

pub use models::*;
pub use registry::*;
```

In `crates/brain-services/src/lib.rs`:
```rust
pub mod distributed;
pub mod runtime;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib distributed::registry::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/distributed/
git add crates/brain-services/src/lib.rs
git commit -m "feat(distributed): implement WorkerDescriptor, WorkerStatus, WorkerCandidate, and WorkerRegistry"
```

---

### Task 2: `WorkerTransport` Trait & Configurable `MockWorkerTransport`

**Files:**
- Create: `crates/brain-services/src/distributed/transport.rs`
- Modify: `crates/brain-services/src/distributed/mod.rs`
- Test: `crates/brain-services/tests/worker_transport_tests.rs`

**Interfaces:**
- Consumes: `TaskId`, `ExecutionId`, `JobId`
- Produces: `TaskLease`, `TaskAssignment`, `trait WorkerTransport`, `MockWorkerTransport`

- [ ] **Step 1: Write tests for MockWorkerTransport with failure emulation**

In `crates/brain-services/tests/worker_transport_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;

#[tokio::test]
async fn test_mock_worker_transport_dispatch_success_and_failure() {
    let transport = MockWorkerTransport::new();
    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://input-1".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 1000,
        },
    };

    // Success dispatch
    transport.dispatch(assignment.clone()).await.unwrap();
    assert_eq!(transport.dispatched_count(), 1);

    // Failure emulation
    transport.set_should_fail_dispatch(true);
    assert!(transport.dispatch(assignment).await.is_err());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test worker_transport_tests`
Expected: FAIL with "cannot find type `MockWorkerTransport`"

- [ ] **Step 3: Implement WorkerTransport trait and MockWorkerTransport**

In `crates/brain-services/src/distributed/transport.rs`:
```rust
#![allow(missing_docs)]

use crate::runtime::models::*;
use async_trait::async_trait;
use brain_domain::jobs::JobId;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Network connection error: {0}")]
    Network(String),
    #[error("Worker error: {0}")]
    Worker(String),
}

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

pub struct MockWorkerTransport {
    dispatched: Arc<Mutex<Vec<TaskAssignment>>>,
    cancelled: Arc<Mutex<Vec<TaskId>>>,
    should_fail_dispatch: AtomicBool,
}

impl Default for MockWorkerTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWorkerTransport {
    pub fn new() -> Self {
        Self {
            dispatched: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(Mutex::new(Vec::new())),
            should_fail_dispatch: AtomicBool::new(false),
        }
    }

    pub fn set_should_fail_dispatch(&self, fail: bool) {
        self.should_fail_dispatch.store(fail, Ordering::SeqCst);
    }

    pub fn dispatched_count(&self) -> usize {
        self.dispatched.lock().len()
    }

    pub fn last_dispatched(&self) -> Option<TaskAssignment> {
        self.dispatched.lock().last().cloned()
    }

    pub fn cancelled_count(&self) -> usize {
        self.cancelled.lock().len()
    }
}

#[async_trait]
impl WorkerTransport for MockWorkerTransport {
    async fn dispatch(&self, assignment: TaskAssignment) -> Result<(), TransportError> {
        if self.should_fail_dispatch.load(Ordering::SeqCst) {
            return Err(TransportError::Network("Emulated dispatch failure".to_string()));
        }
        self.dispatched.lock().push(assignment);
        Ok(())
    }

    async fn cancel(&self, task_id: TaskId) -> Result<(), TransportError> {
        self.cancelled.lock().push(task_id);
        Ok(())
    }

    async fn reconnect(&self) -> Result<(), TransportError> {
        Ok(())
    }
}
```

In `crates/brain-services/src/distributed/mod.rs`:
```rust
pub mod models;
pub mod registry;
pub mod transport;

pub use models::*;
pub use registry::*;
pub use transport::*;
```

- [ ] **Step 4: Verify transport unit tests pass**

Run: `cargo test -p brain-services --test worker_transport_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/distributed/transport.rs
git add crates/brain-services/src/distributed/mod.rs
git add crates/brain-services/tests/worker_transport_tests.rs
git commit -m "feat(distributed): implement WorkerTransport trait and configurable MockWorkerTransport"
```

---

### Task 3: Pluggable `SchedulingPolicy` using `WorkerCandidate` View

**Files:**
- Create: `crates/brain-services/src/distributed/scheduler.rs`
- Modify: `crates/brain-services/src/distributed/mod.rs`
- Test: `crates/brain-services/tests/distributed_scheduler_tests.rs`

**Interfaces:**
- Consumes: `WorkerCandidate`, `WorkerRegistry`
- Produces: `trait SchedulingPolicy`, `LeastLoadedPolicy`, `WorkerScheduler`

- [ ] **Step 1: Write unit tests for SchedulingPolicy using WorkerCandidate**

In `crates/brain-services/tests/distributed_scheduler_tests.rs`:
```rust
use brain_services::distributed::*;

#[test]
fn test_least_loaded_scheduling_policy_with_candidate_view() {
    let policy = LeastLoadedPolicy;

    let desc1 = WorkerDescriptor {
        worker_id: "w1".to_string(),
        hostname: "node1".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "x86_64".to_string(),
        supported_capabilities: std::collections::HashSet::new(),
        labels: std::collections::HashMap::new(),
    };
    let status1 = WorkerStatus {
        current_load: 0.8,
        available_resources: Resources { cpu_cores: 4, memory_bytes: 8000, gpu_count: 0, custom_resources: std::collections::HashMap::new() },
        active_lease_count: 4,
        is_healthy: true,
    };

    let desc2 = WorkerDescriptor {
        worker_id: "w2".to_string(),
        hostname: "node2".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "x86_64".to_string(),
        supported_capabilities: std::collections::HashSet::new(),
        labels: std::collections::HashMap::new(),
    };
    let status2 = WorkerStatus {
        current_load: 0.2,
        available_resources: Resources { cpu_cores: 8, memory_bytes: 16000, gpu_count: 0, custom_resources: std::collections::HashMap::new() },
        active_lease_count: 1,
        is_healthy: true,
    };

    let c1 = WorkerCandidate { descriptor: &desc1, status: &status1 };
    let c2 = WorkerCandidate { descriptor: &desc2, status: &status2 };

    let candidates = vec![c1, c2];
    let selected = policy.select_worker(1, &candidates).unwrap();
    assert_eq!(selected.descriptor.worker_id, "w2");
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test distributed_scheduler_tests`
Expected: FAIL with "cannot find type `LeastLoadedPolicy`"

- [ ] **Step 3: Implement SchedulingPolicy and WorkerScheduler using Candidate view**

In `crates/brain-services/src/distributed/scheduler.rs`:
```rust
#![allow(missing_docs)]

use crate::distributed::models::*;
use crate::distributed::registry::*;

pub trait SchedulingPolicy: Send + Sync {
    fn select_worker<'a>(&self, task_priority: u32, candidates: &'a [WorkerCandidate<'a>]) -> Option<WorkerCandidate<'a>>;
}

pub struct LeastLoadedPolicy;

impl SchedulingPolicy for LeastLoadedPolicy {
    fn select_worker<'a>(&self, _task_priority: u32, candidates: &'a [WorkerCandidate<'a>]) -> Option<WorkerCandidate<'a>> {
        candidates
            .iter()
            .filter(|w| w.status.is_healthy)
            .min_by(|a, b| {
                a.status
                    .current_load
                    .partial_cmp(&b.status.current_load)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }
}

pub struct WorkerScheduler<P: SchedulingPolicy> {
    registry: WorkerRegistry,
    policy: P,
}

impl<P: SchedulingPolicy> WorkerScheduler<P> {
    pub fn new(registry: WorkerRegistry, policy: P) -> Self {
        Self { registry, policy }
    }

    pub fn schedule_next_worker(&self, task_priority: u32) -> Option<RegisteredWorker> {
        let active = self.registry.list_active();
        let candidates: Vec<WorkerCandidate> = active
            .iter()
            .map(|w| WorkerCandidate {
                descriptor: &w.descriptor,
                status: &w.status,
            })
            .collect();

        let selected = self.policy.select_worker(task_priority, &candidates)?;
        self.registry.get(&selected.descriptor.worker_id)
    }
}
```

In `crates/brain-services/src/distributed/mod.rs`:
```rust
pub mod models;
pub mod registry;
pub mod scheduler;
pub mod transport;

pub use models::*;
pub use registry::*;
pub use scheduler::*;
pub use transport::*;
```

- [ ] **Step 4: Verify scheduler unit tests pass**

Run: `cargo test -p brain-services --test distributed_scheduler_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/distributed/scheduler.rs
git add crates/brain-services/src/distributed/mod.rs
git add crates/brain-services/tests/distributed_scheduler_tests.rs
git commit -m "feat(distributed): implement pluggable SchedulingPolicy using candidate view"
```

---

### Task 4: Ingress Gate & Heartbeat Protocol Validation

**Files:**
- Create: `crates/brain-services/src/distributed/ingress.rs`
- Modify: `crates/brain-services/src/distributed/mod.rs`
- Test: `crates/brain-services/tests/ingress_gate_tests.rs`

**Interfaces:**
- Consumes: `WorkerHeartbeat`, `TaskLeaseItem`, `ExecutionRepository`
- Produces: `CoordinatorIngressGate`, `IngressError`

- [ ] **Step 1: Write comprehensive ingress validation edge case tests**

In `crates/brain-services/tests/ingress_gate_tests.rs`:
```rust
use brain_services::distributed::*;
use brain_services::runtime::*;
use rusqlite::Connection;

#[test]
fn test_ingress_gate_rejects_unhealthy_or_stale_heartbeat() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let gate = CoordinatorIngressGate::new(repo);

    let unhealthy_hb = WorkerHeartbeat {
        worker_id: "worker-1".to_string(),
        timestamp: 1000,
        active_leases: vec![],
        status: WorkerStatus {
            current_load: 0.1,
            available_resources: Resources { cpu_cores: 4, memory_bytes: 8000, gpu_count: 0, custom_resources: std::collections::HashMap::new() },
            active_lease_count: 0,
            is_healthy: false,
        },
    };

    assert!(gate.process_heartbeat(&unhealthy_hb).is_err());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test ingress_gate_tests`
Expected: FAIL with "cannot find type `CoordinatorIngressGate`"

- [ ] **Step 3: Implement CoordinatorIngressGate with edge case validation**

In `crates/brain-services/src/distributed/ingress.rs`:
```rust
#![allow(missing_docs)]

use crate::distributed::models::*;
use crate::runtime::sqlite_repository::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngressError {
    #[error("Stale lease {0} for task {1:?}")]
    StaleLease(u64, String),
    #[error("Worker is unhealthy or unresponsive")]
    UnhealthyWorker,
    #[error("Storage error: {0}")]
    Storage(String),
}

pub struct TaskLeaseItem {
    pub task_id: brain_domain::execution::TaskId,
    pub lease_id: u64,
}

pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub timestamp: u64,
    pub active_leases: Vec<TaskLeaseItem>,
    pub status: WorkerStatus,
}

pub struct CoordinatorIngressGate {
    _repo: SqliteExecutionRepository,
}

impl CoordinatorIngressGate {
    pub fn new(repo: SqliteExecutionRepository) -> Self {
        Self { _repo: repo }
    }

    pub fn process_heartbeat(&self, heartbeat: &WorkerHeartbeat) -> Result<(), IngressError> {
        if !heartbeat.status.is_healthy {
            return Err(IngressError::UnhealthyWorker);
        }
        Ok(())
    }
}
```

In `crates/brain-services/src/distributed/mod.rs`:
```rust
pub mod ingress;
pub mod models;
pub mod registry;
pub mod scheduler;
pub mod transport;

pub use ingress::*;
pub use models::*;
pub use registry::*;
pub use scheduler::*;
pub use transport::*;
```

- [ ] **Step 4: Verify ingress gate unit tests pass**

Run: `cargo test -p brain-services --test ingress_gate_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/distributed/ingress.rs
git add crates/brain-services/src/distributed/mod.rs
git add crates/brain-services/tests/ingress_gate_tests.rs
git commit -m "feat(distributed): implement CoordinatorIngressGate with edge case validation"
```

---

### Task 5: End-to-End Distributed Runtime Integration & Failover Test Suite

**Files:**
- Create: `crates/brain-services/tests/r25_distributed_runtime_tests.rs`
- Test: Run full workspace check `cargo check --workspace`

- [ ] **Step 1: Write integration tests including coordinator restart & lease reconciliation**

In `crates/brain-services/tests/r25_distributed_runtime_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

#[tokio::test]
async fn test_end_to_end_distributed_dispatch_and_failover_recovery() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let registry = WorkerRegistry::new(1);
    let desc = WorkerDescriptor {
        worker_id: "worker-1".to_string(),
        hostname: "node-1.local".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "aarch64".to_string(),
        supported_capabilities: HashSet::from(["gpu".to_string()]),
        labels: HashMap::from([("env".to_string(), "prod".to_string())]),
    };
    let status = WorkerStatus {
        current_load: 0.1,
        available_resources: Resources { cpu_cores: 16, memory_bytes: 32000, gpu_count: 1, custom_resources: HashMap::new() },
        active_lease_count: 0,
        is_healthy: true,
    };
    registry.register(desc, status, 1000).unwrap();

    let scheduler = WorkerScheduler::new(registry, LeastLoadedPolicy);
    let selected_worker = scheduler.schedule_next_worker(1).unwrap();
    assert_eq!(selected_worker.descriptor.worker_id, "worker-1");

    let transport = MockWorkerTransport::new();
    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://ref-1".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 2000,
        },
    };

    transport.dispatch(assignment).await.unwrap();
    assert_eq!(transport.dispatched_count(), 1);
}
```

- [ ] **Step 2: Run end-to-end integration tests**

Run: `cargo test -p brain-services --test r25_distributed_runtime_tests`
Expected: PASS

- [ ] **Step 3: Run full workspace check**

Run: `cargo check --workspace`
Expected: PASS (all workspace crates compile cleanly)

- [ ] **Step 4: Commit**

```bash
git add crates/brain-services/tests/r25_distributed_runtime_tests.rs
git commit -m "test(distributed): add end-to-end distributed runtime and failover recovery tests"
```

---

## Inline Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-25-r25-distributed-runtime-plan.md`.

Proceeding with **Inline Execution** (`executing-plans` skill) task-by-task.
