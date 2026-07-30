# Projection Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 3 — Projection Runtime as a pure, deterministic, event-driven state reduction and catch-up replay engine over domain events (`DomainEvent`, `FactEvent`, `EventEnvelope`).

**Architecture:** A layered runtime architecture. Pure domain contracts (`ProjectionId`, `ProjectionVersion`, `Watermark`, `Checkpoint`, `ProjectionLifecycle`, `ProjectionReducer`, `ProjectionError`) live in `brain-domain::projection` with zero external dependencies. Runtime orchestration, projection instances (`ProjectionInstance`), registry (`ProjectionRegistry`), atomic checkpoint store (`CheckpointStore`), catch-up replay (`ReplayEngine`), sequential scheduling (`SequentialProjectionScheduler`), and recovery live in `brain-services::projection`.

**Tech Stack:** Rust (edition 2021), `serde`, `uuid`, `tokio_util::sync::CancellationToken`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- Reducers own local state; the runtime owns lifecycle, watermarks, scheduling, and checkpoint persistence.
- Replay is completely transparent to reducers (`apply_event` receives events regardless of live vs replay mode).
- Projection state updates and checkpoint watermark persistence commit atomically within the same transaction.
- Given identical event streams and snapshots, event application and catch-up replay must be 100% bitwise deterministic.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Projection Identifier, Watermark & Lifecycle | ⬜ Pending | |
| **M1** | Task 2: Checkpoint, Error Hierarchy & Reducer Trait | ⬜ Pending | |
| **M1 Checkpoint** | **Public API Review & Interface Freeze** | ⬜ Pending | |
| **M2** | Task 3: Projection Instance Container | ⬜ Pending | |
| **M2** | Task 4: Projection Registry | ⬜ Pending | |
| **M3** | Task 5: Atomic Checkpoint Store & Persistence | ⬜ Pending | |
| **M4** | Task 6: Catch-Up Replay Engine | ⬜ Pending | |
| **M4** | Task 7: Sequential Projection Scheduler | ⬜ Pending | |
| **M5** | Task 8: Projection Runtime Facade | ⬜ Pending | |
| **M6** | Task 9: Replay Invariants, Interruption Recovery & Verification | ⬜ Pending | |

---

### Task 1: Projection Identifier, Watermark & Lifecycle

**Files:**
- Create: `crates/brain-domain/src/projection/id.rs`
- Create: `crates/brain-domain/src/projection/watermark.rs`
- Create: `crates/brain-domain/src/projection/lifecycle.rs`
- Create: `crates/brain-domain/tests/projection_id_tests.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`
- Modify: `crates/brain-domain/src/lib.rs`

**Interfaces:**
- Consumes: `serde`
- Produces: `ProjectionId`, `ProjectionVersion`, `Watermark`, `ProjectionLifecycle`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/projection_id_tests.rs
use brain_domain::projection::id::*;
use brain_domain::projection::lifecycle::*;
use brain_domain::projection::watermark::*;

#[test]
fn test_projection_id_watermark_lifecycle() {
    let id = ProjectionId::new("graph_adjacency");
    let version = ProjectionVersion(1);
    let watermark = Watermark(100);
    let state = ProjectionLifecycle::Live;

    assert_eq!(id.as_str(), "graph_adjacency");
    assert_eq!(version.0, 1);
    assert_eq!(watermark.0, 100);
    assert_eq!(state, ProjectionLifecycle::Live);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test projection_id_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::id`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/id.rs
//! Strongly typed projection identifier and version value objects.

use serde::{Deserialize, Serialize};

/// Unique projection identifier string wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectionId(pub String);

impl ProjectionId {
    /// Creates a new ProjectionId.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns string reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Logical schema/code version of projection logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectionVersion(pub u32);
```

```rust
// crates/brain-domain/src/projection/watermark.rs
//! Event stream offset sequence watermark.

use serde::{Deserialize, Serialize};

/// Monotonic event stream sequence watermark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Watermark(pub u64);
```

```rust
// crates/brain-domain/src/projection/lifecycle.rs
//! Projection runtime lifecycle states.

use serde::{Deserialize, Serialize};

/// Explicit lifecycle states of a projection instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionLifecycle {
    /// Registered in runtime but not initialized.
    Registered,
    /// Loading checkpoint or preparing storage.
    Initializing,
    /// Performing catch-up event replay.
    Replaying,
    /// Processing live event stream.
    Live,
    /// Gracefully stopping.
    Stopping,
    /// Terminated/stopped.
    Stopped,
}
```

Create `crates/brain-domain/src/projection/mod.rs` re-exporting `id`, `watermark`, `lifecycle`, and export `pub mod projection; pub use projection::*;` in `crates/brain-domain/src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test projection_id_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add ProjectionId, ProjectionVersion, Watermark, and ProjectionLifecycle"
```

---

### Task 2: Checkpoint, Error Hierarchy & Reducer Trait

**Files:**
- Create: `crates/brain-domain/src/projection/checkpoint.rs`
- Create: `crates/brain-domain/src/projection/errors.rs`
- Create: `crates/brain-domain/src/projection/reducer.rs`
- Create: `crates/brain-domain/tests/projection_reducer_tests.rs`
- Modify: `crates/brain-domain/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionId`, `ProjectionVersion`, `Watermark`, `EventEnvelope`
- Produces: `Checkpoint`, `ProjectionError`, `ProjectionReducer`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/projection_reducer_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;

struct DummyReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    count: usize,
}

impl ProjectionReducer for DummyReducer {
    fn id(&self) -> ProjectionId { self.id.clone() }
    fn version(&self) -> ProjectionVersion { self.version }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> {
        self.count += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.count = 0;
        Ok(())
    }
}

#[test]
fn test_projection_reducer_contract() {
    let mut reducer = DummyReducer {
        id: ProjectionId::new("dummy"),
        version: ProjectionVersion(1),
        count: 0,
    };
    assert_eq!(reducer.id().as_str(), "dummy");
    assert_eq!(reducer.count, 0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test projection_reducer_tests
```
Expected: FAIL with `unresolved import brain_domain::projection::checkpoint`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/projection/checkpoint.rs
//! Immutable projection checkpoint value object.

use crate::bkf::Timestamp;
use crate::projection::id::*;
use crate::projection::watermark::*;
use serde::{Deserialize, Serialize};

/// Immutable checkpoint record tracking watermark and state hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Target projection ID.
    pub projection_id: ProjectionId,
    /// Projection code/schema version.
    pub version: ProjectionVersion,
    /// Current sequence watermark.
    pub watermark: Watermark,
    /// Checkpoint timestamp.
    pub timestamp: Timestamp,
    /// Optional state hash for verification.
    pub state_hash: Option<String>,
}
```

```rust
// crates/brain-domain/src/projection/errors.rs
//! Typed error hierarchy for projection runtime.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error during projection reduction or replay.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ProjectionError {
    /// Reducer execution failed.
    #[error("Reducer error: {message}")]
    ReducerFailed {
        /// Error details.
        message: String,
    },
    /// Version mismatch between state and code.
    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch {
        /// Expected version.
        expected: u32,
        /// Found version.
        found: u32,
    },
    /// Checkpoint corrupted.
    #[error("Checkpoint corrupted: {detail}")]
    CheckpointCorrupted {
        /// Error detail.
        detail: String,
    },
    /// Catch-up replay failed.
    #[error("Replay failed at watermark {watermark}: {reason}")]
    ReplayFailed {
        /// Watermark offset.
        watermark: u64,
        /// Failure reason.
        reason: String,
    },
}
```

```rust
// crates/brain-domain/src/projection/reducer.rs
//! Pure domain projection reducer contract.

use crate::projection::errors::*;
use crate::projection::id::*;
use brain_events::EventEnvelope;

/// Core domain reducer trait processing events (replay transparent).
pub trait ProjectionReducer: Send + Sync {
    /// Unique identifier for projection.
    fn id(&self) -> ProjectionId;
    /// Schema/code version of projection logic.
    fn version(&self) -> ProjectionVersion;
    /// Applies an event envelope to update internal state.
    fn apply_event(&mut self, event: &EventEnvelope) -> Result<(), ProjectionError>;
    /// Resets projection state back to initial/empty conditions.
    fn reset(&mut self) -> Result<(), ProjectionError>;
}
```

Re-export `checkpoint`, `errors`, `reducer` in `crates/brain-domain/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test projection_reducer_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add Checkpoint, ProjectionError hierarchy, and ProjectionReducer trait"
```

---

### Milestone 1 Checkpoint: Public API Review & Interface Freeze

- Verify `brain-domain` compiles clean with no warnings.
- Run `cargo test -p brain-domain`.
- Freeze `brain-domain::projection` exports.

---

### Task 3: Projection Instance Container (`crates/brain-services/src/projection/instance.rs`)

**Files:**
- Create: `crates/brain-services/src/projection/instance.rs`
- Create: `crates/brain-services/tests/projection_instance_tests.rs`
- Modify: `crates/brain-services/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionReducer`, `ProjectionLifecycle`, `Checkpoint`
- Produces: `ProjectionInstance` container (`lifecycle`, `checkpoint`, `metrics`, `apply_event`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/projection_instance_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;
use brain_services::projection::instance::*;

struct MockReducer;
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("mock") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> { Ok(()) }
    fn reset(&mut self) -> Result<(), ProjectionError> { Ok(()) }
}

#[test]
fn test_projection_instance_lifecycle_transitions() {
    let reducer = Box::new(MockReducer);
    let mut instance = ProjectionInstance::new(reducer);

    assert_eq!(instance.lifecycle(), ProjectionLifecycle::Registered);
    instance.set_lifecycle(ProjectionLifecycle::Live);
    assert_eq!(instance.lifecycle(), ProjectionLifecycle::Live);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_instance_tests
```
Expected: FAIL with `unresolved import brain_services::projection::instance`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/projection/instance.rs
//! Projection instance container holding reducer, lifecycle, checkpoint, and telemetry metrics.

use brain_domain::bkf::Timestamp;
use brain_domain::projection::*;
use brain_events::EventEnvelope;

/// Runtime metrics for projection.
#[derive(Debug, Clone, Default)]
pub struct ProjectionMetrics {
    /// Total events processed.
    pub events_processed: u64,
}

/// Container wrapping a projection reducer alongside runtime metadata.
pub struct ProjectionInstance {
    reducer: Box<dyn ProjectionReducer>,
    lifecycle: ProjectionLifecycle,
    checkpoint: Checkpoint,
    metrics: ProjectionMetrics,
}

impl ProjectionInstance {
    /// Creates a new ProjectionInstance around a reducer.
    pub fn new(reducer: Box<dyn ProjectionReducer>) -> Self {
        let id = reducer.id();
        let version = reducer.version();
        Self {
            reducer,
            lifecycle: ProjectionLifecycle::Registered,
            checkpoint: Checkpoint {
                projection_id: id,
                version,
                watermark: Watermark(0),
                timestamp: Timestamp::now(),
                state_hash: None,
            },
            metrics: ProjectionMetrics::default(),
        }
    }

    /// Returns projection ID.
    pub fn id(&self) -> ProjectionId {
        self.reducer.id()
    }

    /// Returns projection version.
    pub fn version(&self) -> ProjectionVersion {
        self.reducer.version()
    }

    /// Returns current lifecycle state.
    pub fn lifecycle(&self) -> ProjectionLifecycle {
        self.lifecycle
    }

    /// Sets lifecycle state.
    pub fn set_lifecycle(&mut self, state: ProjectionLifecycle) {
        self.lifecycle = state;
    }

    /// Returns current checkpoint.
    pub fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }

    /// Applies event and updates watermark.
    pub fn apply_event(&mut self, event: &EventEnvelope, seq: u64) -> Result<(), ProjectionError> {
        self.reducer.apply_event(event)?;
        self.checkpoint.watermark = Watermark(seq);
        self.metrics.events_processed += 1;
        Ok(())
    }
}
```

Re-export `instance` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_instance_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement ProjectionInstance container with lifecycle and checkpoint metadata"
```

---

### Task 4: Projection Registry (`crates/brain-services/src/projection/registry.rs`)

**Files:**
- Create: `crates/brain-services/src/projection/registry.rs`
- Create: `crates/brain-services/tests/projection_registry_v2_tests.rs`
- Modify: `crates/brain-services/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionId`, `ProjectionInstance`
- Produces: `ProjectionRegistryV2` (`register`, `get`, `get_mut`, `list_instances`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/projection_registry_v2_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;
use brain_services::projection::instance::*;
use brain_services::projection::registry::*;

struct MockReducer(String);
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new(&self.0) }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> { Ok(()) }
    fn reset(&mut self) -> Result<(), ProjectionError> { Ok(()) }
}

#[test]
fn test_projection_registry_register_and_retrieve() {
    let mut registry = ProjectionRegistryV2::new();
    let instance = ProjectionInstance::new(Box::new(MockReducer("p1".to_string())));

    registry.register(instance).unwrap();
    assert!(registry.get(&ProjectionId::new("p1")).is_some());
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_registry_v2_tests
```
Expected: FAIL with `unresolved import brain_services::projection::registry`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/projection/registry.rs
//! Registry managing registered projection instances.

use crate::projection::instance::*;
use brain_domain::projection::*;
use std::collections::HashMap;

/// Registry managing active ProjectionInstance entries.
#[derive(Default)]
pub struct ProjectionRegistryV2 {
    instances: HashMap<ProjectionId, ProjectionInstance>,
}

impl ProjectionRegistryV2 {
    /// Creates a new ProjectionRegistryV2.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a projection instance.
    pub fn register(&mut self, instance: ProjectionInstance) -> Result<(), ProjectionError> {
        let id = instance.id();
        if self.instances.contains_key(&id) {
            return Err(ProjectionError::ReducerFailed {
                message: format!("Duplicate projection ID registered: {}", id.as_str()),
            });
        }
        self.instances.insert(id, instance);
        Ok(())
    }

    /// Gets reference to an instance.
    pub fn get(&self, id: &ProjectionId) -> Option<&ProjectionInstance> {
        self.instances.get(id)
    }

    /// Gets mutable reference to an instance.
    pub fn get_mut(&mut self, id: &ProjectionId) -> Option<&mut ProjectionInstance> {
        self.instances.get_mut(id)
    }

    /// Returns iterator over instances.
    pub fn instances_mut(&mut self) -> impl Iterator<Item = &mut ProjectionInstance> {
        self.instances.values_mut()
    }
}
```

Re-export `registry` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_registry_v2_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement ProjectionRegistryV2 managing instances"
```

---

### Task 5: Atomic Checkpoint Store & Persistence (`crates/brain-services/src/projection/store.rs`)

**Files:**
- Create: `crates/brain-services/src/projection/store.rs`
- Create: `crates/brain-services/tests/checkpoint_store_tests.rs`
- Modify: `crates/brain-services/src/projection/mod.rs`

**Interfaces:**
- Consumes: `Checkpoint`, `ProjectionId`
- Produces: `CheckpointStore` trait (`save_checkpoint_atomic`, `load_checkpoint`, `reset_projection`), `InMemoryCheckpointStore`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/checkpoint_store_tests.rs
use brain_domain::bkf::Timestamp;
use brain_domain::projection::*;
use brain_services::projection::store::*;

#[test]
fn test_checkpoint_store_save_and_load() {
    let mut store = InMemoryCheckpointStore::new();
    let id = ProjectionId::new("p1");
    let checkpoint = Checkpoint {
        projection_id: id.clone(),
        version: ProjectionVersion(1),
        watermark: Watermark(50),
        timestamp: Timestamp::now(),
        state_hash: None,
    };

    store.save_checkpoint_atomic(&checkpoint).unwrap();
    let loaded = store.load_checkpoint(&id).unwrap().unwrap();
    assert_eq!(loaded.watermark, Watermark(50));
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test checkpoint_store_tests
```
Expected: FAIL with `unresolved import brain_services::projection::store`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/projection/store.rs
//! Storage-agnostic CheckpointStore trait and InMemoryCheckpointStore.

use brain_domain::projection::*;
use std::collections::HashMap;

/// Trait for atomic checkpoint persistence.
pub trait CheckpointStore: Send + Sync {
    /// Saves a checkpoint atomically.
    fn save_checkpoint_atomic(&mut self, checkpoint: &Checkpoint) -> Result<(), ProjectionError>;
    /// Loads the latest checkpoint for a projection.
    fn load_checkpoint(&self, id: &ProjectionId) -> Result<Option<Checkpoint>, ProjectionError>;
    /// Resets checkpoint state for a projection.
    fn reset_projection(&mut self, id: &ProjectionId) -> Result<(), ProjectionError>;
}

/// In-memory CheckpointStore implementation for tests and volatile projections.
#[derive(Default)]
pub struct InMemoryCheckpointStore {
    checkpoints: HashMap<ProjectionId, Checkpoint>,
}

impl InMemoryCheckpointStore {
    /// Creates a new InMemoryCheckpointStore.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save_checkpoint_atomic(&mut self, checkpoint: &Checkpoint) -> Result<(), ProjectionError> {
        self.checkpoints.insert(checkpoint.projection_id.clone(), checkpoint.clone());
        Ok(())
    }

    fn load_checkpoint(&self, id: &ProjectionId) -> Result<Option<Checkpoint>, ProjectionError> {
        Ok(self.checkpoints.get(id).cloned())
    }

    fn reset_projection(&mut self, id: &ProjectionId) -> Result<(), ProjectionError> {
        self.checkpoints.remove(id);
        Ok(())
    }
}
```

Re-export `store` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test checkpoint_store_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement CheckpointStore trait and InMemoryCheckpointStore"
```

---

### Task 6: Catch-Up Replay Engine (`crates/brain-services/src/projection/replay.rs`)

**Files:**
- Create: `crates/brain-services/src/projection/replay.rs`
- Create: `crates/brain-services/tests/replay_engine_tests.rs`
- Modify: `crates/brain-services/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionInstance`, `EventEnvelope`, `Watermark`
- Produces: `ReplayEngine::replay_catchup(instance, events, target_watermark)`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/replay_engine_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;
use brain_services::projection::instance::*;
use brain_services::projection::replay::*;

struct MockReducer(usize);
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("mock") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> {
        self.0 += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.0 = 0;
        Ok(())
    }
}

#[test]
fn test_replay_engine_catchup() {
    let reducer = Box::new(MockReducer(0));
    let mut instance = ProjectionInstance::new(reducer);

    ReplayEngine::replay_catchup(&mut instance, &[], Watermark(0)).unwrap();
    assert_eq!(instance.lifecycle(), ProjectionLifecycle::Live);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test replay_engine_tests
```
Expected: FAIL with `unresolved import brain_services::projection::replay`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/projection/replay.rs
//! Deterministic catch-up replay engine.

use crate::projection::instance::*;
use brain_domain::projection::*;
use brain_events::EventEnvelope;

/// Catch-up replay engine.
pub struct ReplayEngine;

impl ReplayEngine {
    /// Replays events to catch up a projection to target watermark.
    pub fn replay_catchup(
        instance: &mut ProjectionInstance,
        events: &[EventEnvelope],
        target_watermark: Watermark,
    ) -> Result<(), ProjectionError> {
        let current_wm = instance.checkpoint().watermark;
        if current_wm >= target_watermark {
            instance.set_lifecycle(ProjectionLifecycle::Live);
            return Ok(());
        }

        instance.set_lifecycle(ProjectionLifecycle::Replaying);
        for (idx, event) in events.iter().enumerate() {
            let seq = current_wm.0 + idx as u64 + 1;
            if seq > target_watermark.0 {
                break;
            }
            instance.apply_event(event, seq)?;
        }

        instance.set_lifecycle(ProjectionLifecycle::Live);
        Ok(())
    }
}
```

Re-export `replay` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test replay_engine_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement ReplayEngine for catch-up rebuilding"
```

---

### Task 7: Sequential Projection Scheduler (`crates/brain-services/src/projection/scheduler.rs`)

**Files:**
- Create: `crates/brain-services/src/projection/scheduler.rs`
- Create: `crates/brain-services/tests/projection_scheduler_tests.rs`
- Modify: `crates/brain-services/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionRegistryV2`, `EventEnvelope`
- Produces: `ProjectionScheduler` trait, `SequentialProjectionScheduler`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/projection_scheduler_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;
use brain_services::projection::instance::*;
use brain_services::projection::registry::*;
use brain_services::projection::scheduler::*;

struct MockReducer(usize);
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("mock") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> {
        self.0 += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.0 = 0;
        Ok(())
    }
}

#[test]
fn test_sequential_scheduler_dispatches_events() {
    let mut registry = ProjectionRegistryV2::new();
    let instance = ProjectionInstance::new(Box::new(MockReducer(0)));
    registry.register(instance).unwrap();

    let mut scheduler = SequentialProjectionScheduler::new();
    scheduler.dispatch_event(&mut registry, &EventEnvelope::default(), 1).unwrap();
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_scheduler_tests
```
Expected: FAIL with `unresolved import brain_services::projection::scheduler`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/projection/scheduler.rs
//! Sequential projection scheduler (single-writer per-projection invariant).

use crate::projection::registry::*;
use brain_domain::projection::*;
use brain_events::EventEnvelope;

/// Trait for projection event scheduling.
pub trait ProjectionScheduler: Send + Sync {
    /// Dispatches a single event sequentially across registered projections.
    fn dispatch_event(
        &mut self,
        registry: &mut ProjectionRegistryV2,
        event: &EventEnvelope,
        seq: u64,
    ) -> Result<(), ProjectionError>;
}

/// Sequential single-writer projection scheduler.
#[derive(Default)]
pub struct SequentialProjectionScheduler;

impl SequentialProjectionScheduler {
    /// Creates a new SequentialProjectionScheduler.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProjectionScheduler for SequentialProjectionScheduler {
    fn dispatch_event(
        &mut self,
        registry: &mut ProjectionRegistryV2,
        event: &EventEnvelope,
        seq: u64,
    ) -> Result<(), ProjectionError> {
        for instance in registry.instances_mut() {
            instance.apply_event(event, seq)?;
        }
        Ok(())
    }
}
```

Re-export `scheduler` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_scheduler_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement ProjectionScheduler trait and SequentialProjectionScheduler"
```

---

### Task 8: Projection Runtime Facade (`crates/brain-services/src/projection/runtime.rs`)

**Files:**
- Create: `crates/brain-services/src/projection/runtime.rs`
- Create: `crates/brain-services/tests/projection_runtime_tests.rs`
- Modify: `crates/brain-services/src/projection/mod.rs`

**Interfaces:**
- Consumes: `ProjectionRegistryV2`, `CheckpointStore`, `ProjectionScheduler`, `ReplayEngine`
- Produces: `ProjectionRuntimeV2` (`register_projection`, `dispatch_event`, `catchup_all`, `shutdown`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/projection_runtime_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;

struct MockReducer(usize);
impl ProjectionReducer for MockReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("mock") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> {
        self.0 += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.0 = 0;
        Ok(())
    }
}

#[test]
fn test_projection_runtime_lifecycle() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntimeV2::new(store);

    let instance = ProjectionInstance::new(Box::new(MockReducer(0)));
    runtime.register_projection(instance).unwrap();

    runtime.dispatch_event(&EventEnvelope::default(), 1).unwrap();
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_runtime_tests
```
Expected: FAIL with `unresolved import brain_services::projection::runtime`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/projection/runtime.rs
//! ProjectionRuntimeV2 facade orchestrating registration, replay, scheduling, and atomic checkpoint persistence.

use crate::projection::instance::*;
use crate::projection::registry::*;
use crate::projection::replay::*;
use crate::projection::scheduler::*;
use crate::projection::store::*;
use brain_domain::projection::*;
use brain_events::EventEnvelope;

/// Facade managing Phase 3 Projection Runtime operations.
pub struct ProjectionRuntimeV2 {
    registry: ProjectionRegistryV2,
    store: Box<dyn CheckpointStore>,
    scheduler: SequentialProjectionScheduler,
}

impl ProjectionRuntimeV2 {
    /// Creates a new ProjectionRuntimeV2 with a CheckpointStore.
    pub fn new(store: Box<dyn CheckpointStore>) -> Self {
        Self {
            registry: ProjectionRegistryV2::new(),
            store,
            scheduler: SequentialProjectionScheduler::new(),
        }
    }

    /// Registers a projection instance.
    pub fn register_projection(&mut self, instance: ProjectionInstance) -> Result<(), ProjectionError> {
        self.registry.register(instance)
    }

    /// Dispatches an event live to registered projections and persists checkpoints.
    pub fn dispatch_event(&mut self, event: &EventEnvelope, seq: u64) -> Result<(), ProjectionError> {
        self.scheduler.dispatch_event(&mut self.registry, event, seq)?;
        for instance in self.registry.instances_mut() {
            self.store.save_checkpoint_atomic(instance.checkpoint())?;
        }
        Ok(())
    }

    /// Catches up all projections to target watermark.
    pub fn catchup_all(&mut self, events: &[EventEnvelope], target_watermark: Watermark) -> Result<(), ProjectionError> {
        for instance in self.registry.instances_mut() {
            ReplayEngine::replay_catchup(instance, events, target_watermark)?;
            self.store.save_checkpoint_atomic(instance.checkpoint())?;
        }
        Ok(())
    }
}
```

Re-export `runtime` in `crates/brain-services/src/projection/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_runtime_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement ProjectionRuntimeV2 facade orchestrating dispatch, replay, and checkpoint persistence"
```

---

### Task 9: Replay Invariants, Interruption Recovery & Verification

**Files:**
- Create: `crates/brain-services/tests/projection_replay_invariants_tests.rs`

**Interfaces:**
- Consumes: All Phase 3 Projection Runtime components
- Produces: Tests verifying replay equivalence, repeated interruption recovery, empty catchup cutoffs, duplicate event handling, and version migration.

- [ ] **Step 1: Write invariant tests**

```rust
// crates/brain-services/tests/projection_replay_invariants_tests.rs
use brain_domain::projection::*;
use brain_events::EventEnvelope;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;

struct CountingReducer(u64);
impl ProjectionReducer for CountingReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("counting") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &EventEnvelope) -> Result<(), ProjectionError> {
        self.0 += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.0 = 0;
        Ok(())
    }
}

#[test]
fn test_replay_equivalence_and_interruption_recovery() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntimeV2::new(store);

    let instance = ProjectionInstance::new(Box::new(CountingReducer(0)));
    runtime.register_projection(instance).unwrap();

    let events = vec![EventEnvelope::default(), EventEnvelope::default()];
    runtime.catchup_all(&events, Watermark(2)).unwrap();
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test projection_replay_invariants_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add projection replay equivalence and interruption recovery invariant tests"
```
