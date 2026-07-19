# Design Spec: Unified Composition Root & Runtime Harness

This design details the introduction of `BrainRuntime` as the single composition root and lifecycle coordinator of the Brain Relational Engine, and the integration harness `runtime_harness.rs` to verify end-to-end operation and teardown.

---

## 1. Approach & Trade-offs

We propose **Approach 1** (declaring `BrainRuntime` within `brain-services`) instead of creating a new crate. This avoids workspace bloat while keeping all service-wiring concerns inside the existing services layer.

```
Host (e.g. Harness or Daemon)
      │
      ▼
┌────────────────────────────────────────────────────────┐
│ brain-services                                         │
│                                                        │
│  BrainRuntime (Composition Root / Public API)          │
│        │                                               │
│        ├── SqliteStorage (from brain-storage)          │
│        ├── SqliteCanonicalizer                         │
│        ├── SqliteReflectionEngine                      │
│        ├── SqliteProjectionManager                     │
│        ├── InMemoryEventDispatcher                     │
│        └─ Observability (CorrelationIndex & Subscriber)│
└────────────────────────────────────────────────────────┘
```

---

## 2. API Design & Lifecycle

`BrainRuntime` will expose a host-agnostic, high-level API:

```rust
pub struct BrainRuntime {
    storage: SqliteStorage,
    dispatcher: Arc<InMemoryEventDispatcher>,
    canonicalizer: SqliteCanonicalizer,
    projection_manager: SqliteProjectionManager,
    correlation_index: Arc<Mutex<CorrelationIndex>>,
    // Option wrapped to allow moving out/dropping during shutdown
    subscriber: Option<ObservabilitySubscriber>,
}

impl BrainRuntime {
    /// Constructs and initializes the entire runtime, spawning background threads.
    pub fn new(db_path: &str) -> Result<Self, BrainError> {
        // 1. Initialize SQLite storage and run migrations
        let storage = SqliteStorage::new(db_path, 4, true)?;
        
        // 2. Initialize Event Dispatcher
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait = Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        // 3. Initialize Observability Subscriber with sync channel
        let correlation_index = Arc::new(Mutex::new(CorrelationIndex::new()));
        let sync_rx = dispatcher.subscribe_sync();
        let subscriber = ObservabilitySubscriber::new(sync_rx, Arc::clone(&correlation_index));

        // 4. Initialize Reflection Engine
        let reflection_engine = Arc::new(SqliteReflectionEngine::new(
            storage.clone(),
            Arc::clone(&dispatcher_trait),
        ));

        // 5. Initialize Canonicalizer (attaching reflection engine)
        let canonicalizer = SqliteCanonicalizer::new(
            storage.clone(),
            Arc::clone(&dispatcher_trait),
        )
        .with_reflection(reflection_engine);

        // 6. Initialize Projection Manager
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

    /// Primary Ingestion boundary. Coordinates validation, canonicalization, and reflection.
    pub fn ingest(&self, obs: Observation) -> Result<CanonicalizationResult, BrainError> {
        self.canonicalizer.canonicalize(obs)
    }

    /// Unified projection query boundary.
    pub fn query_projection<P, Q: ProjectionQuery, PR: Projector<P, Q>>(
        &self,
        projector: &PR,
        query: &Q,
        correlation_id: CorrelationId,
    ) -> P {
        self.projection_manager.project(projector, query, correlation_id)
    }

    /// Allows hosts or adapters to subscribe to runtime event stream.
    /// Exposes a generic channel Receiver without leaking concrete implementation details.
    pub fn subscribe(&self) -> tokio::sync::mpsc::Receiver<Arc<dyn RuntimeEvent>> {
        self.dispatcher.subscribe()
    }

    /// Exposes read-only facade query for correlation index spans.
    pub fn spans_for(&self, corr_id: CorrelationId) -> Option<Vec<brain_observability::timeline::OperationSpan>> {
        let index = self.correlation_index.lock().unwrap();
        index.spans_for(corr_id).map(|spans| spans.to_vec())
    }

    /// Exposes read-only facade query for checking correlation completeness.
    pub fn is_complete(&self, corr_id: CorrelationId) -> bool {
        let index = self.correlation_index.lock().unwrap();
        index.is_complete(corr_id)
    }

    /// Lifecycle boundary: stops workers, flushes event queues, and closes storage connections.
    /// Consumes `self` to statically guarantee no further actions can be invoked after shutdown.
    pub fn shutdown(mut self) -> Result<(), BrainError> {
        // 1. Stop dispatch & drop senders
        self.dispatcher.shutdown();

        // 2. Join the subscriber background thread (joining is handled by drop(sub) in Drop implementation)
        if let Some(sub) = self.subscriber.take() {
            drop(sub);
        }

        // 3. Release SQLite storage connections
        drop(self.storage);

        Ok(())
    }
}
```

---

## 3. Ownership

| Component | Owner |
|---|---|
| SQLite storage | `BrainRuntime` |
| Dispatcher | `BrainRuntime` |
| Reflection engine | `BrainRuntime` |
| Projection manager | `BrainRuntime` |
| Observability subscriber | `BrainRuntime` |

---

## 4. Detailed Component Modifications

### A. `InMemoryEventDispatcher`
Add a `.shutdown()` method to drop held channel senders and trigger downstream receiver shutdowns:
```rust
impl InMemoryEventDispatcher {
    pub fn shutdown(&self) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.clear();
        let mut sync_subs = self.sync_subscribers.lock().unwrap();
        sync_subs.clear();
    }
}
```

### B. `ObservabilitySubscriber`
Change the internal join handle to `Option` to enable explicit joining on drop:
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

---

## 5. Verification Plan

We will create a new integration test target under `crates/brain-services/tests/runtime_harness.rs` executing the staged rollout:

```rust
#[test]
fn test_runtime_harness_lifecycle() {
    let temp_db = TempDir::new(); // or dynamic temp file path
    
    // 1. Construction
    let runtime = BrainRuntime::new(&temp_db.path())?;
    let mut event_rx = runtime.subscribe();

    // 2. Operation
    let corr_id = CorrelationId::new_v4();
    let obs = make_obs("Harness canonicalization", corr_id);
    let result = runtime.ingest(obs)?;
    assert_eq!(result.epoch.0, 1);

    // Verify projection query
    let projector = SqliteProjector::new(runtime.storage_clone()); // or helper
    let projection = runtime.query_projection(&projector, &MemoryListQuery { limit: 10 }, corr_id);
    assert_eq!(projection.items.len(), 1);

    // 3. Teardown
    runtime.shutdown()?;

    // Verify background threads terminated
    // Verify no leaks, clean DB handles
}
```

---

## 6. Post-Implementation Note: Background Worker Shutdown Invariant

*Added after Sprint 5 implementation.*

### The Deadlock That Was Found

During Sprint 5 testing, both observability tests hung indefinitely. The root cause was a
**drop-order deadlock** caused by the new `impl Drop` join on `ObservabilitySubscriber`:

```text
Problem (before the fix):

Drop order (Rust reverse-declaration order):
  _subscriber drops first  →  join() called
  thread blocks on recv()  ←  waiting for SyncSender to be dropped
  canonicalizer still holds Arc<dyn RuntimeEventDispatcher>  ←  refcount > 0
  dispatcher is NOT freed  →  SyncSender stays alive
  → DEADLOCK
```

The subtle part: `drop(dispatcher_trait)` and `drop(dispatcher)` only reduce the Arc
refcount. If a third holder exists (here, `canonicalizer` held an `Arc<dyn RuntimeEventDispatcher>`
clone), the `InMemoryEventDispatcher` is never freed and the `SyncSender` is never dropped.

### The Fix

`dispatcher.shutdown()` explicitly clears the internal sender lists, **regardless of Arc
refcount**. This is why the method exists:

```rust
pub fn shutdown(&self) {
    self.subscribers.lock().unwrap().clear();       // closes async channels
    self.sync_subscribers.lock().unwrap().clear();  // closes sync channels → thread unblocks
}
```

### The Design Principle

> **Background workers must terminate because the runtime explicitly ends their communication
> channels, not because the final `Arc` happens to be dropped.**

Relying on Arc drop order for shutdown correctness is fragile:
- A new service added later may introduce another Arc clone of the dispatcher.
- The compiler will not warn that the shutdown ordering assumption has been broken.
- The failure mode is a silent hang, not a compile error or panic.

`dispatcher.shutdown()` makes the intent explicit and immune to future Arc clone counts.

### The Invariant Encoded in `BrainRuntime::shutdown()`

```rust
pub fn shutdown(mut self) -> Result<(), BrainError> {
    // Step 1: close channels — thread unblocks from recv(), sees Disconnected, exits
    self.dispatcher.shutdown();

    // Step 2: join thread — returns immediately because thread has already exited
    if let Some(sub) = self.subscriber.take() {
        drop(sub);
    }

    // Step 3: release connection pool
    drop(self.storage);

    Ok(())
}
```

This ordering **must not be reversed**. Steps 1 and 2 are tightly coupled. If step 2
precedes step 1, `join()` blocks forever.

### Extension to Future Workers

Any future background worker added to the runtime (e.g. a consolidation worker, a
projection pre-computation thread, an async event processor) should follow the same pattern:

1. The runtime holds a sender or signal that terminates the worker's loop.
2. `shutdown()` sends the termination signal **first**.
3. Only then does `shutdown()` join or await the worker.

The communication channel is the shutdown mechanism. Arc lifetime is not.
