# Design Specification: Projection Runtime (Phase 3)

## 1. Executive Summary & Goals

The **Projection Runtime** provides a pure, deterministic, event-driven state reduction and catch-up replay framework over domain events (`DomainEvent`, `FactEvent`, `EventEnvelope`). It manages projection registration, event routing, catch-up rebuilding, versioned checkpoint persistence, and failure recovery.

### Architectural Invariants & Core Rules
- **Zero External Dependencies in Domain**: `brain-domain::projection` contains `ProjectionId`, `ProjectionVersion`, `Watermark`, `Checkpoint`, `ProjectionState`, `ProjectionReducer` trait, and typed `ProjectionError` hierarchy with zero async runtimes, storage drivers, or database engines.
- **Single Logical Writer Guarantee**: Every projection acts as a pure, serialized state machine over an ordered sequence of events (`State(n) + Event(n+1) -> State(n+1)`).
- **Deterministic Replay Equivalence**: Replaying events $E_1 \dots E_N$ against an empty projection yields bitwise-identical `ProjectionState` and `Watermark` as processing $E_1 \dots E_N$ incrementally.
- **Pluggable Scheduler Abstraction**: Execution scheduling is insulated behind `ProjectionScheduler`. Phase 3 implements `SequentialProjectionScheduler`, leaving room for future `BatchProjectionScheduler` or `ParallelProjectionScheduler` without altering projection contracts or APIs.
- **Storage-Agnostic Checkpointing**: Checkpoint persistence is defined via `CheckpointStore` trait (`save_checkpoint`, `load_checkpoint`, `reset_projection`).

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
                                       └── ProjectionScheduler
                                                │
                                                ▼
                                   SequentialScheduler
                        (brain-services::projection::scheduler)
                                                │
                                                ▼
                                      ProjectionInstance
                                                │
                                                ▼
                                        ProjectionReducer
                           (brain-domain::projection::reducer)
                                                │
                                                ▼
                                    State(n+1) + Checkpoint
```

---

## 3. Detailed Component Layout

### 3.1 `crates/brain-domain/src/projection/` (Domain Models & Value Objects)

- **`id.rs`**: Strongly typed projection identifier `ProjectionId(pub String)` and `ProjectionVersion(pub u32)`.
- **`watermark.rs`**: Event stream offset tracking `Watermark(pub u64)`.
- **`checkpoint.rs`**: Immutable `Checkpoint` value object containing `projection_id`, `version`, `watermark`, `timestamp`, and `state_hash`.
- **`reducer.rs`**: Core domain reducer contract:
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

- **`registry.rs`**: `ProjectionRegistry` managing registered projections, version validations, and duplicate prevention.
- **`store.rs`**: `CheckpointStore` trait for atomic checkpoint persistence.
- **`replay.rs`**: `ReplayEngine` driving deterministic catch-up rebuilding from a specified watermark.
- **`scheduler.rs`**: `ProjectionScheduler` trait and `SequentialProjectionScheduler` implementation.
- **`runtime.rs`**: `ProjectionRuntime` facade orchestrating event dispatch, catch-up replay, and shutdown signals.

---

## 4. Verification & Testing Strategy

1. **Unit Tests (`brain-domain` & `brain-services`)**:
   - `ProjectionRegistry` registration, version checks, and conflict detection.
   - `CheckpointStore` atomic save, load, and reset operations.
   - Reducer event application and state transitions.
2. **Replay & Invariant Tests (`crates/brain-services/tests/projection_replay_tests.rs`)**:
   - **Replay Equivalence**: Replaying 1,000 events produces identical `Checkpoint` state hash as incremental execution.
   - **Watermark Monotonicity**: Watermark increments strictly monotonically.
   - **Crash Recovery**: Interrupting execution at watermark $M$ and restarting from $M$ yields identical final state.
