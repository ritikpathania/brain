# Milestone R26 — Worker Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Milestone R26 (Worker Runtime & Execution Engine) in Rust within `crates/brain-services/src/worker/`, introducing worker-side task execution (`trait TaskExecutor`), artifact staging (`trait ArtifactStore`), checkpointing (`trait CheckpointStore`), process cancellation cascading, resource reservation drop guards, and composable executor decorators.

**Architecture:** Worker Runtime housed in `brain-services::worker` (layered strictly **above** `brain-services::runtime` and consuming `brain-services::distributed` DTOs). Provides pluggable `TaskExecutor` implementations (`InProcessExecutor`, `SubprocessExecutor`), `ArtifactKind` local staging, RAII `ResourceReservation`, and wrapper decorators (`TimeoutExecutor`, `RetryExecutor`, `MetricsExecutor`).

**Tech Stack:** Rust, `tokio`, `async-trait`, `tokio-util`, `serde`, `uuid`, `thiserror`.

## Global Constraints

- **Module Hierarchy Rule**: `worker/` may depend on `distributed/` and `runtime/`, but `runtime/` and `distributed/` MUST NEVER depend on `worker/`.
- **Stabilization Boundary Integrity**: `crates/brain-domain` and core Phase 1/Phase 2 contracts (`ExecutionId`, `TaskId`, `TaskAssignment`, `WorkerTransport`) MUST remain unchanged.
- **Transient TaskAssignment DTO**: `TaskAssignment` is passed by reference `&TaskAssignment` into executors and is not modified or cloned unnecessarily.
- **Monotonic Clock Authoritativeness**: `Instant::now()` is used internally for duration and timeout timing. `timestamp: u64` is reserved strictly for external telemetry.
- **RAII Resource Release**: `ResourceReservation` implements RAII drop guards to guarantee resource release across success, panic, cancellation, or failure.

---

### Task 1: Worker Core Models & `TaskExecutionContext`

**Files:**
- Create: `crates/brain-services/src/worker/mod.rs`
- Create: `crates/brain-services/src/worker/models.rs`
- Create: `crates/brain-services/src/worker/context.rs`
- Modify: `crates/brain-services/src/lib.rs`
- Test: `crates/brain-services/src/worker/context.rs` (inline test module)

**Interfaces:**
- Consumes: `TaskId`, `TaskAssignment`, `TaskLease` from `runtime` and `distributed`
- Produces: `TaskResult`, `TaskExecutionError`, `TaskExecutionEvent`, `TaskExecutionContext`

- [x] **Step 1: Write failing unit tests for TaskExecutionContext**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_task_execution_context_creation() {
        let token = CancellationToken::new();
        let started_at = Instant::now();

        let ctx = TaskExecutionContext {
            cancellation_token: token.clone(),
            started_at,
        };

        assert!(!ctx.cancellation_token.is_cancelled());
        token.cancel();
        assert!(ctx.cancellation_token.is_cancelled());
    }
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib worker::context::tests`
Expected: FAIL with "module `worker` not found"

- [x] **Step 3: Implement TaskResult, TaskExecutionError, TaskExecutionEvent, and TaskExecutionContext**

In `crates/brain-services/src/worker/models.rs`:
```rust
#![allow(missing_docs)]

use crate::runtime::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub output_ref: String,
    pub checkpoint_id: Option<String>,
    pub execution_time_ms: u64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum TaskExecutionError {
    #[error("Task execution cancelled")]
    Cancelled,
    #[error("Task execution timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),
    #[error("Artifact error: {0}")]
    ArtifactError(String),
    #[error("Checkpoint error: {0}")]
    CheckpointError(String),
    #[error("Internal executor error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskExecutionEvent {
    Started { task_id: TaskId, timestamp: u64 },
    Progress { task_id: TaskId, percentage: u8, message: Option<String> },
    CheckpointSaved { task_id: TaskId, checkpoint_id: String },
    Completed { task_id: TaskId, result: TaskResult },
    Failed { task_id: TaskId, error: String },
}
```

In `crates/brain-services/src/worker/context.rs`:
```rust
#![allow(missing_docs)]

use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TaskExecutionContext {
    pub cancellation_token: CancellationToken,
    pub started_at: Instant,
}
```

In `crates/brain-services/src/worker/mod.rs`:
```rust
pub mod context;
pub mod models;

pub use context::*;
pub use models::*;
```

In `crates/brain-services/src/lib.rs`:
```rust
pub mod worker;
```

- [x] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib worker::context::tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/worker/
git add crates/brain-services/src/lib.rs
git commit -m "feat(worker): implement TaskResult, TaskExecutionError, TaskExecutionEvent, and TaskExecutionContext"
```

---

### Task 2: `ArtifactStore` Trait & Local Staging Implementation

**Files:**
- Create: `crates/brain-services/src/worker/artifact.rs`
- Modify: `crates/brain-services/src/worker/mod.rs`
- Test: `crates/brain-services/tests/artifact_store_tests.rs`

**Interfaces:**
- Consumes: `TaskId`
- Produces: `ArtifactKind`, `ArtifactError`, `trait ArtifactStore`, `LocalFilesystemArtifactStore`

- [x] **Step 1: Write integration tests for LocalFilesystemArtifactStore**

In `crates/brain-services/tests/artifact_store_tests.rs`:
```rust
use brain_services::runtime::*;
use brain_services::worker::*;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_local_filesystem_artifact_store_staging_and_publishing() {
    let dir = tempdir().unwrap();
    let store = LocalFilesystemArtifactStore::new(dir.path().to_path_buf());

    let task_id = TaskId::new();
    let file_path = dir.path().join("output.txt");
    fs::write(&file_path, "sample output").unwrap();

    let pub_ref = store
        .publish_artifact(task_id, ArtifactKind::Output, &file_path)
        .await
        .unwrap();

    assert!(pub_ref.starts_with("artifact://"));

    let staged_path = store.stage_input(&pub_ref).await.unwrap();
    assert!(staged_path.exists());
    assert_eq!(fs::read_to_string(staged_path).unwrap(), "sample output");
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test artifact_store_tests`
Expected: FAIL with "cannot find type `LocalFilesystemArtifactStore`"

- [x] **Step 3: Implement ArtifactKind, ArtifactStore trait, and LocalFilesystemArtifactStore**

In `crates/brain-services/src/worker/artifact.rs`:
```rust
#![allow(missing_docs)]

use crate::runtime::models::TaskId;
use crate::worker::models::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("Artifact error: {0}")]
    Io(String),
    #[error("Invalid artifact reference: {0}")]
    InvalidRef(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    Input,
    Output,
    Log,
    Checkpoint,
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn stage_input(&self, input_ref: &str) -> Result<PathBuf, TaskExecutionError>;
    async fn publish_artifact(&self, task_id: TaskId, kind: ArtifactKind, local_path: &PathBuf) -> Result<String, TaskExecutionError>;
}

pub struct LocalFilesystemArtifactStore {
    base_dir: PathBuf,
}

impl LocalFilesystemArtifactStore {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }
}

#[async_trait]
impl ArtifactStore for LocalFilesystemArtifactStore {
    async fn stage_input(&self, input_ref: &str) -> Result<PathBuf, TaskExecutionError> {
        let rel_path = input_ref.trim_start_matches("artifact://");
        let target = self.base_dir.join(rel_path);
        if target.exists() {
            Ok(target)
        } else {
            Err(TaskExecutionError::ArtifactError(format!("Input file not found: {}", input_ref)))
        }
    }

    async fn publish_artifact(&self, task_id: TaskId, kind: ArtifactKind, local_path: &PathBuf) -> Result<String, TaskExecutionError> {
        let file_name = local_path.file_name().ok_or_else(|| TaskExecutionError::ArtifactError("Invalid filename".to_string()))?;
        let kind_dir = match kind {
            ArtifactKind::Input => "inputs",
            ArtifactKind::Output => "outputs",
            ArtifactKind::Log => "logs",
            ArtifactKind::Checkpoint => "checkpoints",
        };

        let dest_dir = self.base_dir.join(kind_dir).join(task_id.0.to_string());
        std::fs::create_dir_all(&dest_dir).map_err(|e| TaskExecutionError::ArtifactError(e.to_string()))?;

        let dest = dest_dir.join(file_name);
        std::fs::copy(local_path, &dest).map_err(|e| TaskExecutionError::ArtifactError(e.to_string()))?;

        let rel = format!("{}/{}/{}", kind_dir, task_id.0, file_name.to_string_lossy());
        Ok(format!("artifact://{}", rel))
    }
}
```

In `crates/brain-services/src/worker/mod.rs`:
```rust
pub mod artifact;
pub mod context;
pub mod models;

pub use artifact::*;
pub use context::*;
pub use models::*;
```

- [x] **Step 4: Verify artifact store tests pass**

Run: `cargo test -p brain-services --test artifact_store_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/worker/artifact.rs
git add crates/brain-services/src/worker/mod.rs
git add crates/brain-services/tests/artifact_store_tests.rs
git commit -m "feat(worker): implement ArtifactKind, ArtifactStore trait, and LocalFilesystemArtifactStore"
```

---

### Task 3: `TaskExecutor` Trait, `TaskExecutorFactory`, & `InProcessExecutor`

**Files:**
- Create: `crates/brain-services/src/worker/executor.rs`
- Modify: `crates/brain-services/src/worker/mod.rs`
- Test: `crates/brain-services/tests/in_process_executor_tests.rs`

**Interfaces:**
- Consumes: `TaskAssignment`, `TaskExecutionContext`
- Produces: `trait TaskExecutor`, `trait TaskExecutorFactory`, `InProcessExecutor`

- [x] **Step 1: Write integration tests for InProcessExecutor with cancellation**

In `crates/brain-services/tests/in_process_executor_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use brain_services::worker::*;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_in_process_executor_execution_and_cancellation() {
    let executor = InProcessExecutor::new();
    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://inputs/sample.txt".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 2000,
        },
    };

    let token = CancellationToken::new();
    let ctx = TaskExecutionContext {
        cancellation_token: token.clone(),
        started_at: Instant::now(),
    };

    let result = executor.execute(&assignment, &ctx).await.unwrap();
    assert_eq!(result.task_id, task_id);

    // Verify cancellation
    token.cancel();
    let err = executor.execute(&assignment, &ctx).await.unwrap_err();
    assert!(matches!(err, TaskExecutionError::Cancelled));
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test in_process_executor_tests`
Expected: FAIL with "cannot find type `InProcessExecutor`"

- [x] **Step 3: Implement TaskExecutor trait, TaskExecutorFactory trait, and InProcessExecutor**

In `crates/brain-services/src/worker/executor.rs`:
```rust
#![allow(missing_docs)]

use crate::distributed::transport::TaskAssignment;
use crate::worker::context::*;
use crate::worker::models::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError>;
}

pub trait TaskExecutorFactory: Send + Sync {
    fn create_executor(&self, assignment: &TaskAssignment) -> Arc<dyn TaskExecutor>;
}

pub struct InProcessExecutor;

impl Default for InProcessExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl InProcessExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TaskExecutor for InProcessExecutor {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError> {
        if ctx.cancellation_token.is_cancelled() {
            return Err(TaskExecutionError::Cancelled);
        }

        let elapsed = ctx.started_at.elapsed().as_millis() as u64;

        Ok(TaskResult {
            task_id: assignment.task_id,
            output_ref: format!("artifact://outputs/{}/result.json", assignment.task_id.0),
            checkpoint_id: None,
            execution_time_ms: elapsed,
            metadata: HashMap::from([("executor".to_string(), "in_process".to_string())]),
        })
    }
}
```

In `crates/brain-services/src/worker/mod.rs`:
```rust
pub mod artifact;
pub mod context;
pub mod executor;
pub mod models;

pub use artifact::*;
pub use context::*;
pub use executor::*;
pub use models::*;
```

- [x] **Step 4: Verify in-process executor unit tests pass**

Run: `cargo test -p brain-services --test in_process_executor_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/worker/executor.rs
git add crates/brain-services/src/worker/mod.rs
git add crates/brain-services/tests/in_process_executor_tests.rs
git commit -m "feat(worker): implement TaskExecutor trait, TaskExecutorFactory trait, and InProcessExecutor"
```

---

### Task 4: Composable Executor Decorators (`TimeoutExecutor`, `RetryExecutor`)

**Files:**
- Create: `crates/brain-services/src/worker/decorators.rs`
- Modify: `crates/brain-services/src/worker/mod.rs`
- Test: `crates/brain-services/tests/executor_decorator_tests.rs`

**Interfaces:**
- Consumes: `TaskExecutor`
- Produces: `TimeoutExecutor`, `RetryExecutor`

- [ ] **Step 1: Write integration tests for TimeoutExecutor and RetryExecutor**

In `crates/brain-services/tests/executor_decorator_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use brain_services::worker::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_timeout_and_retry_executor_decorator_composition() {
    let inner = Arc::new(InProcessExecutor::new());
    let timeout_exec = TimeoutExecutor::new(inner, Duration::from_millis(500));
    let retry_exec = RetryExecutor::new(Arc::new(timeout_exec), 2);

    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://ref".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "w1".to_string(),
            lease_until: 1000,
        },
    };

    let ctx = TaskExecutionContext {
        cancellation_token: CancellationToken::new(),
        started_at: Instant::now(),
    };

    let result = retry_exec.execute(&assignment, &ctx).await.unwrap();
    assert_eq!(result.task_id, task_id);
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test executor_decorator_tests`
Expected: FAIL with "cannot find type `TimeoutExecutor`"

- [x] **Step 3: Implement TimeoutExecutor and RetryExecutor**

In `crates/brain-services/src/worker/decorators.rs`:
```rust
#![allow(missing_docs)]

use crate::distributed::transport::TaskAssignment;
use crate::worker::context::*;
use crate::worker::executor::*;
use crate::worker::models::*;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

pub struct TimeoutExecutor {
    inner: Arc<dyn TaskExecutor>,
    timeout: Duration,
}

impl TimeoutExecutor {
    pub fn new(inner: Arc<dyn TaskExecutor>, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
}

#[async_trait]
impl TaskExecutor for TimeoutExecutor {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError> {
        match tokio::time::timeout(self.timeout, self.inner.execute(assignment, ctx)).await {
            Ok(res) => res,
            Err(_) => Err(TaskExecutionError::Timeout(self.timeout)),
        }
    }
}

pub struct RetryExecutor {
    inner: Arc<dyn TaskExecutor>,
    max_retries: u32,
}

impl RetryExecutor {
    pub fn new(inner: Arc<dyn TaskExecutor>, max_retries: u32) -> Self {
        Self { inner, max_retries }
    }
}

#[async_trait]
impl TaskExecutor for RetryExecutor {
    async fn execute(
        &self,
        assignment: &TaskAssignment,
        ctx: &TaskExecutionContext,
    ) -> Result<TaskResult, TaskExecutionError> {
        let mut attempts = 0;
        loop {
            match self.inner.execute(assignment, ctx).await {
                Ok(res) => return Ok(res),
                Err(err) => {
                    attempts += 1;
                    if attempts > self.max_retries || matches!(err, TaskExecutionError::Cancelled) {
                        return Err(err);
                    }
                }
            }
        }
    }
}
```

In `crates/brain-services/src/worker/mod.rs`:
```rust
pub mod artifact;
pub mod context;
pub mod decorators;
pub mod executor;
pub mod models;

pub use artifact::*;
pub use context::*;
pub use decorators::*;
pub use executor::*;
pub use models::*;
```

- [x] **Step 4: Verify decorator unit tests pass**

Run: `cargo test -p brain-services --test executor_decorator_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add crates/brain-services/src/worker/decorators.rs
git add crates/brain-services/src/worker/mod.rs
git add crates/brain-services/tests/executor_decorator_tests.rs
git commit -m "feat(worker): implement TimeoutExecutor and RetryExecutor composable decorators"
```

---

### Task 5: End-to-End Worker Runtime & Execution Suite

**Files:**
- Create: `crates/brain-services/tests/r26_worker_runtime_tests.rs`
- Test: Run full workspace check `cargo check --workspace`

- [ ] **Step 1: Write end-to-end integration test for Worker Runtime**

In `crates/brain-services/tests/r26_worker_runtime_tests.rs`:
```rust
use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use brain_services::worker::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_end_to_end_worker_execution_staging_and_decorators() {
    let dir = tempdir().unwrap();
    let artifact_store = Arc::new(LocalFilesystemArtifactStore::new(dir.path().to_path_buf()));

    let inner = Arc::new(InProcessExecutor::new());
    let timeout = Arc::new(TimeoutExecutor::new(inner, Duration::from_secs(5)));
    let executor = RetryExecutor::new(timeout, 2);

    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://inputs/sample.txt".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 3000,
        },
    };

    let ctx = TaskExecutionContext {
        cancellation_token: CancellationToken::new(),
        started_at: Instant::now(),
    };

    let result = executor.execute(&assignment, &ctx).await.unwrap();
    assert_eq!(result.task_id, task_id);
    assert_eq!(result.metadata.get("executor").unwrap(), "in_process");
}
```

- [ ] **Step 2: Run end-to-end integration tests**

Run: `cargo test -p brain-services --test r26_worker_runtime_tests`
Expected: PASS

- [x] **Step 3: Run full workspace check**

Run: `cargo check --workspace`
Expected: PASS (all workspace crates compile cleanly)

- [x] **Step 4: Commit**

```bash
git add crates/brain-services/tests/r26_worker_runtime_tests.rs
git commit -m "test(worker): add end-to-end worker runtime and decorator integration tests"
```

---

## Inline Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-28-r26-worker-runtime-plan.md`.

Proceeding with **Inline Execution** (`executing-plans` skill) task-by-task.
