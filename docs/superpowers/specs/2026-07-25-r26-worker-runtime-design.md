# Milestone R26 — Worker Runtime & Execution Engine Architecture Specification

## Executive Summary

Milestone **R26 (Worker Runtime & Execution Engine)** defines the worker-side execution pipeline for `brain`. Building directly on top of Milestone R25's [`WorkerTransport`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/distributed/transport.rs) and transport-agnostic [`TaskAssignment`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/superpowers/specs/2026-07-25-r25-distributed-runtime-design.md#2-trait-abstractions--interface-boundaries) DTOs, R26 introduces a pluggable execution engine (`trait TaskExecutor`), artifact staging runtime (`trait ArtifactStore`), checkpoint management (`trait CheckpointStore`), resource reservation tracking, and composable executor decorators (`TimeoutExecutor`, `MetricsExecutor`, `RetryExecutor`).

---

## 1. Architecture & Worker Runtime Layering

The worker runtime operates locally on worker nodes. It decouples orchestration/leasing from job execution:

```text
                             WorkerRuntime
                                   │
              ┌────────────────────┼────────────────────┐
              ▼                    ▼                    ▼
     AssignmentReceiver     TaskDispatcher      HeartbeatLoop
                                   │
                           TaskExecutionContext
         ┌─────────────────────────┼─────────────────────────┐
         ▼                         ▼                         ▼
  ArtifactManager          CheckpointManager            TaskLogger
                                   │
                          TaskExecutorFactory
                                   │
                           trait TaskExecutor
             (Composed via Decorator Chain: Metrics ➔ Timeout ➔ Retry)
                ┌──────────────────┴──────────────────┐
                ▼                                     ▼
        InProcessExecutor                     SubprocessExecutor
```

### Component Responsibilities & Ownership Rules
- **`WorkerRuntime`**: Manages node process lifecycle, heartbeats, and worker health over [`WorkerTransport`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/distributed/transport.rs). Owns cancellation tokens and requests. Never imports coordinator-side storage models directly.
- **`TaskDispatcher`**: Receives `TaskAssignment` DTOs, creates `ResourceReservation`, instantiates `TaskExecutionContext`, and requests an executor instance from `TaskExecutorFactory`.
- **`TaskExecutorFactory`**: Selects the appropriate `TaskExecutor` implementation based on assignment metadata, job parameters, capability labels, and resource reservations.
- **`TaskExecutor`**: Executes task logic under isolation bounds (`InProcessExecutor`, `SubprocessExecutor`), observes cancellation tokens, and returns a structured `TaskResult` or `TaskExecutionError`.
- **`ArtifactManager`**: Stages input artifacts locally into `PathBuf` staging locations prior to execution and publishes output artifacts upon completion.

---

## 2. Core Execution Traits & Context

### `TaskExecutionContext` & Execution Metadata
`TaskExecutionContext` is an input value object created by `TaskDispatcher` and passed by reference to `TaskExecutor`:

```rust
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};

pub struct TaskExecutionContext {
    pub cancellation_token: CancellationToken,
    pub artifact_store: Arc<dyn ArtifactStore>,
    pub checkpoint_store: Arc<dyn CheckpointStore>,
    pub logger: Arc<dyn TaskLogger>,
    pub lease: TaskLease,
    pub started_at: Instant,
}
```

### Structured Results & Executor-Agnostic Error Model
```rust
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
```

---

## 3. Artifact Runtime & Reference Staging

Executors never interact directly with raw `artifact://...` URIs or remote cloud APIs. `ArtifactManager` stages files into local `PathBuf` staging locations:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    Input,
    Output,
    Log,
    Checkpoint,
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn stage_input(&self, input_ref: &str) -> Result<PathBuf, ArtifactError>;
    async fn publish_artifact(&self, task_id: TaskId, kind: ArtifactKind, local_path: &PathBuf) -> Result<String, ArtifactError>;
}
```
* **Note**: Executors receive local `PathBuf` references, keeping S3, SQLite blob, or local filesystem implementations fully decoupled from task execution logic.

---

## 4. Checkpointing & Cooperative Cancellation

### Checkpoint Storage Interface
```rust
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    async fn save_checkpoint(&self, task_id: TaskId, payload: &[u8]) -> Result<String, CheckpointError>;
    async fn load_checkpoint(&self, checkpoint_id: &str) -> Result<Vec<u8>, CheckpointError>;
}
```
* **Future Evolution**: Streaming checkpoint APIs (`tokio::io::AsyncRead`) will be introduced in future milestones for multi-gigabyte checkpoints; `Vec<u8>` is authoritative for R26.

### Cancellation Policies & Process Signals
```rust
pub struct CancellationPolicy {
    pub grace_period: std::time::Duration,
}
```
- **Cancellation Ownership**: `WorkerRuntime` owns cancellation requests and token instances. `TaskExecutor` observes tokens but never creates or propagates cancellation independently.
- **`InProcessExecutor`**: Listens directly to `ctx.cancellation_token.cancelled()`.
- **`SubprocessExecutor`**: Listens to `cancellation_token.cancelled()`, sends `SIGTERM` to the child process PID, waits up to `grace_period` (default 5s), and escalates to `SIGKILL` if the subprocess hangs.

---

## 5. Resource Reservation & Decorator Composition

### Resource Reservation Lifecycle
Before invoking task execution, `TaskDispatcher` allocates an explicit `ResourceReservation`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReservation {
    pub reservation_id: u64,
    pub requested_cpu: u32,
    pub reserved_cpu: u32,
    pub requested_memory_bytes: u64,
    pub reserved_memory_bytes: u64,
    pub requested_gpu: u32,
    pub reserved_gpu: u32,
}
```
* **RAII Resource Guarantee**: Resource reservations implement RAII drop guards. Resource release (`reserve` $\rightarrow$ `execute` $\rightarrow$ `release`) is strictly guaranteed across normal completion, panic, timeout, cancellation, or worker failure.

### Composable Decorator Chain
Executors compose via wrapper decorators:

```text
TaskDispatcher ──► RetryExecutor ──► TimeoutExecutor ──► MetricsExecutor ──► InProcessExecutor / SubprocessExecutor
```
* **Ordering Semantics**: Decorators are composable. Wrapping `RetryExecutor` around `TimeoutExecutor` retries timed-out attempts, whereas wrapping `TimeoutExecutor` around `RetryExecutor` bounds the total execution window across all attempts.

---

## 6. Real-Time Task Execution Events & Ownership Pipeline

Executors emit granular task progress events back to `WorkerRuntime` for telemetry and live progress tracking:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskExecutionEvent {
    Started { task_id: TaskId, timestamp: u64 },
    Progress { task_id: TaskId, percentage: u8, message: Option<String> },
    CheckpointSaved { task_id: TaskId, checkpoint_id: String },
    Completed { task_id: TaskId, result: TaskResult },
    Failed { task_id: TaskId, error: String },
}
```
* **Time Semantics**: Durations and intervals internally use monotonic `Instant` timing. `timestamp: u64` (epoch seconds) is used strictly for external telemetry and reporting.
* **Event Pipeline Ownership**: `TaskExecutor` emits `TaskExecutionEvent` $\rightarrow$ `WorkerRuntime` receives $\rightarrow$ `WorkerTransport` transmits $\rightarrow$ `Coordinator` ingests into event journal.

---

## Verification & Test Plan

1. **Unit Tests (`crates/brain-services/src/worker/`)**:
   - `InProcessExecutor` execution and `CancellationToken` graceful shutdown tests.
   - `SubprocessExecutor` process execution, stdout/stderr capture, and `SIGTERM` termination tests.
   - Decorator composition tests (`TimeoutExecutor` timing out hanging tasks, `RetryExecutor` retrying failed attempts).
   - `ResourceReservation` allocation and drop guard release tests.
2. **Integration Tests (`crates/brain-services/tests/r26_worker_runtime_tests.rs`)**:
   - End-to-end task staging (`ArtifactStore`), execution (`InProcessExecutor`), checkpointing, and `TaskResult` generation.
   - Subprocess cancellation timeout test: verify process receives `SIGTERM` and escalates to `SIGKILL`.
