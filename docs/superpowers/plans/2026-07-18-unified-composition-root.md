# Unified Composition Root Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify the relational memory engine capabilities (`SqliteStorage`, `SqliteCanonicalizer`, `SqliteReflectionEngine`, `SqliteProjectionManager`, `InMemoryEventDispatcher`, and `ObservabilitySubscriber`) under a single `BrainRuntime` composition root and lifecycle owner, and verify end-to-end operation and teardown via a runtime integration harness.

**Architecture:** Implement `BrainRuntime` in `crates/brain-services/src/brain_runtime.rs` within `brain-services`. Update `InMemoryEventDispatcher` and `ObservabilitySubscriber` to support explicit shutdown and thread joining.

**Tech Stack:** Rust, SQLite, std::thread, tokio

## Global Constraints

- Expose a single, unified, high-level API. Do not expose internal component capability accessors by default.
- `shutdown()` must consume `self` to statically prevent invocation of work after teardown.
- Hide concrete synchronization types (e.g. `Arc<Mutex<CorrelationIndex>>`) behind read facade methods.
- Proactively join observability thread on subscriber drop.
- Zero regressions across existing Sprint 1–4 test suite.

---

### Task 1: Update ObservabilitySubscriber for thread joining on drop

**Files:**
- Modify: `crates/brain-observability/src/subscriber.rs`
- Test: `crates/brain-services/tests/sprint3_tests.rs`

**Interfaces:**
- `ObservabilitySubscriber` struct changes `_handle` from `JoinHandle<()>` to `handle: Option<JoinHandle<()>>`
- Implements `Drop` for `ObservabilitySubscriber`

- [ ] **Step 1: Write Drop implementation in `crates/brain-observability/src/subscriber.rs`**
  Modify the struct fields and implement `Drop` to take and join the handle:
  ```rust
  pub struct ObservabilitySubscriber {
      index: Arc<Mutex<CorrelationIndex>>,
      handle: Option<thread::JoinHandle<()>>,
  }
  
  impl Drop for ObservabilitySubscriber {
      fn drop(&mut self) {
          if let Some(h) = self.handle.take() {
              let _ = h.join();
          }
      }
  }
  ```
- [ ] **Step 2: Update construction call in `subscriber.rs`**
  Update `new` constructor to wrap the spawned join handle in `Some(...)`.
- [ ] **Step 3: Run existing sprint3 tests to verify compile & check**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-services --test sprint3_tests`
  Expected: PASS
- [ ] **Step 4: Commit**
  ```bash
  git add crates/brain-observability/src/subscriber.rs
  git commit -m "refactor: drop and join thread in ObservabilitySubscriber"
  ```

---

### Task 2: Update InMemoryEventDispatcher with shutdown method

**Files:**
- Modify: `crates/brain-services/src/event_dispatcher.rs`

**Interfaces:**
- Produces: `InMemoryEventDispatcher::shutdown(&self)`

- [ ] **Step 1: Add shutdown method to `crates/brain-services/src/event_dispatcher.rs`**
  ```rust
  impl InMemoryEventDispatcher {
      /// Drop all sync and async event subscription senders, closing channels
      /// and triggering graceful shutdown of downstream receiver loops.
      pub fn shutdown(&self) {
          let mut subs = self.subscribers.lock().unwrap();
          subs.clear();
          let mut sync_subs = self.sync_subscribers.lock().unwrap();
          sync_subs.clear();
      }
  }
  ```
- [ ] **Step 2: Run cargo check**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo check`
  Expected: Success
- [ ] **Step 3: Commit**
  ```bash
  git add crates/brain-services/src/event_dispatcher.rs
  git commit -m "feat: add shutdown to InMemoryEventDispatcher"
  ```

---

### Task 3: Implement BrainRuntime Composition Root

**Files:**
- Create: `crates/brain-services/src/brain_runtime.rs`
- Modify: `crates/brain-services/src/lib.rs`

**Interfaces:**
- Produces: `BrainRuntime` struct and methods:
  - `BrainRuntime::new(db_path: &str) -> Result<Self, BrainError>`
  - `BrainRuntime::ingest(&self, obs: Observation) -> Result<CanonicalizationResult, BrainError>`
  - `BrainRuntime::query_projection<P, Q, PR>(&self, projector: &PR, query: &Q, corr_id: CorrelationId) -> P`
  - `BrainRuntime::subscribe(&self) -> tokio::sync::mpsc::Receiver<Arc<dyn RuntimeEvent>>`
  - `BrainRuntime::spans_for(&self, corr_id: CorrelationId) -> Option<Vec<OperationSpan>>`
  - `BrainRuntime::is_complete(&self, corr_id: CorrelationId) -> bool`
  - `BrainRuntime::shutdown(self) -> Result<(), BrainError>`

- [ ] **Step 1: Write `crates/brain-services/src/brain_runtime.rs`**
  Ensure constructor is exception-safe (if step 4/5/6 fails, RAII automatically drops partially initialized storage/dispatcher and terminates/joins spawned threads).
  Include explicit ordering documentation inside `shutdown()`.
  ```rust
  use std::sync::{Arc, Mutex};
  use std::time::SystemTime;
  use brain_core::{
      errors::BrainError,
      events::{CorrelationId, RuntimeEvent, RuntimeEventDispatcher},
      evolution::{CanonicalizationResult, Observation},
      projection::{ProjectionQuery, Projector},
  };
  use brain_storage::SqliteStorage;
  use brain_observability::{CorrelationIndex, ObservabilitySubscriber, timeline::OperationSpan};
  use crate::{
      InMemoryEventDispatcher, SqliteCanonicalizer, SqliteProjectionManager, SqliteReflectionEngine,
  };

  pub struct BrainRuntime {
      storage: SqliteStorage,
      dispatcher: Arc<InMemoryEventDispatcher>,
      canonicalizer: SqliteCanonicalizer,
      projection_manager: SqliteProjectionManager,
      correlation_index: Arc<Mutex<CorrelationIndex>>,
      subscriber: Option<ObservabilitySubscriber>,
  }

  impl BrainRuntime {
      /// Creates a new fully initialized BrainRuntime.
      ///
      /// **Exception safety**: If construction fails midway, partial resources (e.g. connection pools
      /// or spawned threads) are dropped in reverse initialization order, ensuring clean teardown.
      pub fn new(db_path: &str) -> Result<Self, BrainError> {
          let storage = SqliteStorage::new(db_path, 4, true)?;
          let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
          let dispatcher_trait = Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

          let correlation_index = Arc::new(Mutex::new(CorrelationIndex::new()));
          let sync_rx = dispatcher.subscribe_sync();
          let subscriber = ObservabilitySubscriber::new(sync_rx, Arc::clone(&correlation_index));

          let reflection_engine = Arc::new(SqliteReflectionEngine::new(
              storage.clone(),
              Arc::clone(&dispatcher_trait),
          ));

          let canonicalizer = SqliteCanonicalizer::new(
              storage.clone(),
              Arc::clone(&dispatcher_trait),
          )
          .with_reflection(reflection_engine);

          let epoch = Arc::new(Mutex::new(brain_domain::EpochId::initial()));
          let projection_manager = SqliteProjectionManager::new(
              storage.clone(),
              epoch,
              Arc::clone(&dispatcher_trait),
          );

          Ok(Self {
              storage,
              dispatcher,
              canonicalizer,
              projection_manager,
              correlation_index,
              subscriber: Some(subscriber),
          })
      }

      pub fn ingest(&self, obs: Observation) -> Result<CanonicalizationResult, BrainError> {
          self.canonicalizer.canonicalize(obs)
      }

      pub fn query_projection<P, Q: ProjectionQuery, PR: Projector<P, Q>>(
          &self,
          projector: &PR,
          query: &Q,
          correlation_id: CorrelationId,
      ) -> P {
          self.projection_manager.project(projector, query, correlation_id)
      }

      pub fn subscribe(&self) -> tokio::sync::mpsc::Receiver<Arc<dyn RuntimeEvent>> {
          self.dispatcher.subscribe()
      }

      pub fn spans_for(&self, corr_id: CorrelationId) -> Option<Vec<OperationSpan>> {
          let index = self.correlation_index.lock().unwrap();
          index.spans_for(corr_id).map(|spans| spans.to_vec())
      }

      pub fn is_complete(&self, corr_id: CorrelationId) -> bool {
          let index = self.correlation_index.lock().unwrap();
          index.is_complete(corr_id)
      }

      /// Exposes read-only storage reference.
      ///
      /// **Testing support only**: Not intended as a host extension API. Use only for test query assertions.
      pub fn storage_ref(&self) -> &SqliteStorage {
          &self.storage
      }

      /// Gracefully shuts down the runtime components.
      ///
      /// **Teardown Invariants**:
      /// 1. Close event dispatcher channels first (rejects new work, drops all SyncSenders).
      /// 2. Drop the subscriber. The dropped subscriber triggers join on the observability background thread.
      /// 3. Release/drop the SQLite storage connection pool.
      pub fn shutdown(mut self) -> Result<(), BrainError> {
          self.dispatcher.shutdown();
          if let Some(sub) = self.subscriber.take() {
              drop(sub);
          }
          drop(self.storage);
          Ok(())
      }
  }
  ```
- [ ] **Step 2: Declare & Export from `crates/brain-services/src/lib.rs`**
  Add:
  ```rust
  pub mod brain_runtime;
  pub use brain_runtime::BrainRuntime;
  ```
- [ ] **Step 3: Compile check**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo check`
  Expected: Success
- [ ] **Step 4: Commit**
  ```bash
  git add crates/brain-services/src/brain_runtime.rs crates/brain-services/src/lib.rs
  git commit -m "feat: add BrainRuntime composition root"
  ```

---

### Task 5: Implement Runtime Harness Integration Test

**Files:**
- Create: `crates/brain-services/tests/runtime_harness.rs`

- [ ] **Step 1: Write integration tests in `crates/brain-services/tests/runtime_harness.rs`**
  ```rust
  use std::sync::Arc;
  use std::time::SystemTime;
  use brain_core::{
      events::CorrelationId,
      evolution::{Observation, Provenance},
  };
  use brain_services::{BrainRuntime, SqliteProjector, MemoryListQuery, MemoryListProjection};
  use tempfile::tempdir;

  fn make_obs(payload: &str, corr_id: CorrelationId) -> Observation {
      Observation {
          payload: payload.as_bytes().to_vec(),
          media_type: "text/plain".to_string(),
          provenance: Provenance {
              source_adapter: "test".to_string(),
              timestamp: SystemTime::now(),
              correlation_id: corr_id,
          },
      }
  }

  #[test]
  fn test_runtime_harness_lifecycle() {
      let dir = tempdir().expect("Failed to create tempdir");
      let db_path = dir.path().join("test.db");
      let db_str = db_path.to_str().expect("Valid path string");

      // 1. Construction & Setup
      let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

      // Verify startup observer events by subscribing
      let mut rx = runtime.subscribe();

      // 2. Exercise - Ingestion & Reflection
      let corr_id = CorrelationId::new_v4();
      let result = runtime.ingest(make_obs("Ingested from Harness", corr_id))
          .expect("Failed to ingest observation");

      assert_eq!(result.epoch.0, 1);
      assert_eq!(result.affected_entities.len(), 1);

      // Verify Projection Query using storage_ref
      let projector = SqliteProjector::new(runtime.storage_ref().clone());
      let query = MemoryListQuery { limit: 10 };
      let projection = runtime.query_projection(&projector, &query, corr_id);
      assert_eq!(projection.items.len(), 1);
      assert_eq!(projection.items[0].label, "Ingested from Harness");

      // Verify Observability index collects timelines
      let spans = runtime.spans_for(corr_id).expect("Timeline exists");
      assert!(!spans.is_empty());
      assert!(runtime.is_complete(corr_id));

      // 3. Teardown & Lifecycle Assertion
      runtime.shutdown().expect("Failed to shutdown cleanly");

      // Ensure background threads terminated and channel is disconnected (not just empty)
      assert_eq!(
          rx.try_recv().unwrap_err(),
          tokio::sync::mpsc::error::TryRecvError::Disconnected
      );
  }
  ```
- [ ] **Step 2: Run the newly created integration test**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test runtime_harness`
  Expected: PASS
- [ ] **Step 3: Commit**
  ```bash
  git add crates/brain-services/tests/runtime_harness.rs
  git commit -m "test: add runtime_harness end-to-end integration test"
  ```

---

### Task 6: Run Regression & Verify All Crates

- [ ] **Step 1: Run all workspace tests**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --workspace`
  Expected: PASS (all tests green)
- [ ] **Step 2: Verify the final state**
  Confirm all tests pass without warnings or issues.
