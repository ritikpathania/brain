# Design Specification: Projection Runtime (Phase 3)

## 1. Executive Summary & Goals

The **Projection Runtime** provides a pure, deterministic, event-driven state reduction and catch-up replay framework over domain events (`DomainEvent`, `FactEvent`, `EventEnvelope`). It manages projection registration, event routing, catch-up rebuilding, versioned checkpoint persistence, lifecycle transitions, and failure recovery.

### Architectural Invariants & Core Rules
- **Zero External Dependencies in Domain**: `brain-domain::projection` contains `ProjectionId`, `ProjectionVersion`, `Watermark`, `Checkpoint`, `ProjectionLifecycle`, `ProjectionReducer` trait, and typed `ProjectionError` hierarchy with zero async runtimes, storage drivers, or database engines.
- **Single Logical Writer Guarantee**: Every projection acts as a pure, serialized state machine over an ordered sequence of events (`State(n) + Event(n+1) -> State(n+1)`).
- **State Ownership Separation**: The `ProjectionReducer` owns local read-model state; the `ProjectionRuntime` owns execution lifecycle, scheduling, watermarks, and checkpoint persistence.
- **Strict Event Total Ordering Invariant**: Every projection observes bitwise-identical event sequence ordering during live processing and catch-up replay.
- **Atomic Transactional Checkpoints**: Projection state update and checkpoint watermark persistence commit atomically within the same transaction (`Projection State Commit AND Checkpoint Commit`).
- **Replay Transparent Reducers**: Reducers process events via `apply_event(event)` regardless of whether events are live, replayed, or catch-up. Replay mode is transparently managed by the `ReplayEngine`.
- **Pluggable Scheduler Abstraction**: Execution scheduling is insulated behind `ProjectionScheduler`. Phase 3 implements `SequentialProjectionScheduler`, leaving room for future `BatchProjectionScheduler` or `ParallelProjectionScheduler` without altering projection contracts or APIs.

---

## 2. Architecture & Data Flow Pipeline

```text
                           Domain Event Log / Stream
                                       │
                                       ▼
                               ProjectionRuntime
                    (brain-services::projection::runtime)
                                       │
                                       ├── ProjectionRegistry
                                       ├── ReplayEngine
                                       ├── CheckpointStore
                                       └── SequentialScheduler
                                                │
                                                ▼
                                       ProjectionInstance
                        ├── ProjectionReducer (State Owner)
                        ├── ProjectionLifecycle State
                        ├── Checkpoint & Watermark
                        └── Telemetry Metrics
                                                │
                                                ▼
                                    State(n+1) + Checkpoint
```

---

## 3. Detailed Component Layout

### 3.1 `crates/brain-domain/src/projection/` (Domain Models & Value Objects)

- **`id.rs`**: Strongly typed projection identifier `ProjectionId(pub String)` and `ProjectionVersion(pub u32)`.
- **`watermark.rs`**: Event stream offset tracking `Watermark(pub u64)`.
- **`lifecycle.rs`**: Explicit projection status enum:
  - `ProjectionLifecycle`: `Registered`, `Initializing`, `Replaying`, `Live`, `Stopping`, `Stopped`.
- **`checkpoint.rs`**: Immutable `Checkpoint` value object containing `projection_id`, `version`, `watermark`, `timestamp`, and `state_hash`.
- **`reducer.rs`**: Core domain reducer contract (state owned by reducer, replay transparent):
  ```rust
  pub trait ProjectionReducer: Send + Sync {
      fn id(&self) -> ProjectionId;
      fn version(&self) -> ProjectionVersion;
      fn apply_event(&mut self, event: &EventEnvelope) -> Result<(), ProjectionError>;
      fn reset(&mut self) -> Result<(), ProjectionError>;
  }
  ```
- **`errors.rs`**: Typed `ProjectionError` hierarchy (`ReducerFailed`, `VersionMismatch`, `CheckpointCorrupted`, `ReplayFailed`).

### 3.2 `crates/brain-services/src/projection/` (Runtime, Schedulers & Store)

- **`instance.rs`**: `ProjectionInstance` container holding `reducer: Box<dyn ProjectionReducer>`, `lifecycle: ProjectionLifecycle`, `checkpoint: Checkpoint`, and metrics.
- **`registry.rs`**: `ProjectionRegistry` managing registered projections, version validations, and conflict detection.
- **`store.rs`**: `CheckpointStore` trait for atomic state + checkpoint persistence (`save_checkpoint_atomic`, `load_checkpoint`, `reset_projection`).
- **`replay.rs`**: `ReplayEngine` driving deterministic catch-up rebuilding from a specified watermark.
- **`scheduler.rs`**: `ProjectionScheduler` trait and `SequentialProjectionScheduler` implementation.
- **`runtime.rs`**: `ProjectionRuntime` facade orchestrating lifecycle transitions, event dispatch, catch-up replay, and graceful shutdown.

---

## 4. Verification & Testing Strategy

1. **Unit Tests (`brain-domain` & `brain-services`)**:
   - `ProjectionRegistry` registration, version checks, and conflict detection.
   - `CheckpointStore` atomic save, load, and reset operations.
   - Lifecycle state machine transitions (`Registered` -> `Initializing` -> `Replaying` -> `Live` -> `Stopping` -> `Stopped`).
2. **Replay & Invariant Tests (`crates/brain-services/tests/projection_replay_tests.rs`)**:
   - **Replay Equivalence**: Replaying 1,000 events produces identical `Checkpoint` state hash as incremental execution.
   - **Watermark Monotonicity**: Watermark increments strictly monotonically.
   - **Repeated Interruption Recovery**: Repeatedly interrupting replay at arbitrary watermarks $M_1, M_2$ and restarting yields identical final state.
   - **Empty Replay Cutoff**: When `checkpoint == head`, catch-up replay performs zero event processing work.
   - **Idempotence & Duplicate Delivery**: Resending an event with sequence number $\le$ current watermark is safely ignored or idempotently processed.
   - **Projection Version Upgrade**: Updating a projection from Version 1 to Version 2 triggers reset and full catch-up replay correctly.