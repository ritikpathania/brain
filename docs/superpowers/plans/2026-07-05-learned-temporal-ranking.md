# Learned/Adaptive Temporal Ranking (Phase 12) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement learned and adaptive temporal ranking to dynamically calibrate feature weights based on user relevance feedback without mutating projection visibility states.

**Architecture:** Introduces immutable, versioned `WeightSnapshot` configurations read by `LearnedTemporalScorer` (via a polymorphic `RankingModel`). Ingested feedback events are processed offline by `CalibrationEngine` to output candidate snapshots, which are atomically swapped by `WeightCalibrationService`.

**Tech Stack:** Rust, SQLite, Serde (JSON serialization).

## Global Constraints

- **Snapshot Determinism**: Fixed `WeightSnapshot` and identical input query must always yield exactly identical scoring outputs.
- **Atomic Publication**: Query context observes exactly one snapshot version; publication atomically replaces the entire active snapshot.
- **Version Reproducibility**: Replaying a query against a fixed graph and snapshot version produces identical rankings.
- **Calibration Reproducibility**: Given identical input feedback, policy, and starting snapshot, calibration generates a byte-for-byte identical candidate snapshot.
- **Calibration Isolation**: Calibration runs never mutate graphs, visibility projection sets, query evidence, or volatile caches.
- **Weight Snapshot Completeness**: Every published snapshot defines all ranking weights.
- **Model Transparency**: Under identical version, weights, and signals, model scoring must be identical.
- **Snapshot Monotonicity**: Published snapshot versions must satisfy `v1 < v2 < v3 < ...`.

---

### Task 1: Core Value Objects & Models

**Files:**
- Create: `crates/brain-domain/tests/ranking_value_objects_tests.rs`
- Modify: `crates/brain-domain/src/retrieval/models.rs`

**Interfaces:**
- Consumes: None
- Produces: `RankingWeight`, `NormalizedSignal`, `SnapshotVersion`, `CalibrationMetadata`, `SnapshotMetadata`, `RankingWeights`, `WeightSnapshot`, `FeedbackEvent`, `RankingSignals`, `CalibrationPolicyVersion`, `CalibrationPolicy`, `CalibrationReport`, `RankingModelVersion`, `RankingModel` trait, `LinearRankingModel`.

- [ ] **Step 1: Write the failing tests**
  Create `crates/brain-domain/tests/ranking_value_objects_tests.rs`:
  ```rust
  use brain_domain::retrieval::models::{RankingWeight, NormalizedSignal, RankingWeights};
  use brain_domain::consolidation::MetricConstructionError;

  #[test]
  fn test_ranking_weight_validation() {
      assert!(RankingWeight::new(1.5).is_ok());
      assert!(matches!(RankingWeight::new(-0.1), Err(MetricConstructionError::OutOfRange { .. })));
      assert!(matches!(RankingWeight::new(f64::NAN), Err(MetricConstructionError::NotFinite { .. })));
  }

  #[test]
  fn test_normalized_signal_validation() {
      assert!(NormalizedSignal::new(0.5).is_ok());
      assert!(matches!(NormalizedSignal::new(-0.01), Err(MetricConstructionError::OutOfRange { .. })));
      assert!(matches!(NormalizedSignal::new(1.01), Err(MetricConstructionError::OutOfRange { .. })));
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test ranking_value_objects_tests`
  Expected: FAIL with compilation error (types not found)

- [ ] **Step 3: Write minimal implementation**
  Add definition to `crates/brain-domain/src/retrieval/models.rs`:
  ```rust
  use crate::consolidation::MetricConstructionError;

  /// Non-negative, finite ranking multiplier value.
  #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
  pub struct RankingWeight(f64);

  impl RankingWeight {
      pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
          if !val.is_finite() {
              return Err(MetricConstructionError::NotFinite { val });
          }
          if val < 0.0 {
              return Err(MetricConstructionError::OutOfRange { val, min: 0.0, max: f64::MAX });
          }
          Ok(Self(val))
      }

      pub fn value(&self) -> f64 {
          self.0
      }
  }

  impl std::hash::Hash for RankingWeight {
      fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
          hash_f64(self.0, state);
      }
  }

  impl Eq for RankingWeight {}

  /// Normalized ranking signal in range [0.0, 1.0].
  #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
  pub struct NormalizedSignal(f64);

  impl NormalizedSignal {
      pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
          if !val.is_finite() {
              return Err(MetricConstructionError::NotFinite { val });
          }
          if val < 0.0 || val > 1.0 {
              return Err(MetricConstructionError::OutOfRange { val, min: 0.0, max: 1.0 });
          }
          Ok(Self(val))
      }

      pub fn value(&self) -> f64 {
          self.0
      }
  }

  impl std::hash::Hash for NormalizedSignal {
      fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
          hash_f64(self.0, state);
      }
  }

  impl Eq for NormalizedSignal {}

  /// Monotonically incrementing snapshot identifier.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
  pub struct SnapshotVersion(pub u64);

  impl SnapshotVersion {
      pub fn new(val: u64) -> Self {
          Self(val)
      }

      pub fn value(&self) -> u64 {
          self.0
      }
  }

  /// Extensible structured calibration details.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct CalibrationMetadata {
      algorithm_used: String,
      validation_loss: Option<f64>,
  }

  impl CalibrationMetadata {
      pub fn new(algorithm_used: String, validation_loss: Option<f64>) -> Self {
          Self {
              algorithm_used,
              validation_loss,
          }
      }

      pub fn algorithm_used(&self) -> &str {
          &self.algorithm_used
      }

      pub fn validation_loss(&self) -> Option<f64> {
          self.validation_loss
      }
  }

  /// Metadata tracking weight lineage.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct SnapshotMetadata {
      pub version: SnapshotVersion,
      pub created_at: crate::temporal::TimePoint,
      pub calibration_metadata: CalibrationMetadata,
  }

  /// Scoring multipliers, immutable by construction.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct RankingWeights {
      semantic: RankingWeight,
      graph: RankingWeight,
      recency: RankingWeight,
      temporal: RankingWeight,
  }

  impl RankingWeights {
      pub fn new(
          semantic: RankingWeight,
          graph: RankingWeight,
          recency: RankingWeight,
          temporal: RankingWeight,
      ) -> Self {
          Self {
              semantic,
              graph,
              recency,
              temporal,
          }
      }

      pub fn semantic(&self) -> RankingWeight {
          self.semantic
      }

      pub fn graph(&self) -> RankingWeight {
          self.graph
      }

      pub fn recency(&self) -> RankingWeight {
          self.recency
      }

      pub fn temporal(&self) -> RankingWeight {
          self.temporal
      }
  }

  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct WeightSnapshot {
      pub metadata: SnapshotMetadata,
      pub weights: RankingWeights,
  }

  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct FeedbackEvent {
      pub id: String,
      pub schema_version: u32,
      pub query: String,
      pub node_id: NodeId,
      pub selected: bool,
      pub timestamp: u64,
      pub ranking_position: usize,
      pub context: String,
  }

  /// Feature scores representing candidate properties.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub struct RankingSignals {
      pub semantic: NormalizedSignal,
      pub graph: NormalizedSignal,
      pub recency: NormalizedSignal,
      pub temporal: NormalizedSignal,
  }

  /// Version tag for calibration policies.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
  pub struct CalibrationPolicyVersion(pub u32);

  /// Parameter knobs configuring calibration loops.
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct CalibrationPolicy {
      pub version: CalibrationPolicyVersion,
      pub learning_rate: f64,
      pub regularization: f64,
      pub min_feedback_events: usize,
  }

  /// Immutable calibration result parameters.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct CalibrationReport {
      pub candidate_version: SnapshotVersion,
      pub previous_version: SnapshotVersion,
      pub policy_version: CalibrationPolicyVersion,
      pub feedback_processed: usize,
      pub validation_loss: f64,
      pub convergence_information: String,
      pub publication_decision: bool,
  }

  /// Version identifier tracking the model implementation structure.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub enum RankingModelVersion {
      V1Linear,
  }

  /// Polymorphic interface for scoring candidate nodes.
  pub trait RankingModel: Send + Sync {
      fn version(&self) -> RankingModelVersion;
      fn score(&self, signals: &RankingSignals) -> f64;
  }

  #[derive(Debug, Clone)]
  pub struct LinearRankingModel {
      weights: RankingWeights,
  }

  impl LinearRankingModel {
      pub fn new(weights: RankingWeights) -> Self {
          Self { weights }
      }
  }

  impl RankingModel for LinearRankingModel {
      fn version(&self) -> RankingModelVersion {
          RankingModelVersion::V1Linear
      }

      fn score(&self, signals: &RankingSignals) -> f64 {
          self.weights.semantic().value() * signals.semantic.value()
              + self.weights.graph().value() * signals.graph.value()
              + self.weights.recency().value() * signals.recency.value()
              + self.weights.temporal().value() * signals.temporal.value()
      }
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test ranking_value_objects_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  Run: `git add crates/brain-domain/src/retrieval/models.rs crates/brain-domain/tests/ranking_value_objects_tests.rs && git commit -m "feat: add ranking value objects and models for Phase 12"`

---

### Task 2: Migration Version 6

**Files:**
- Modify: `crates/brain-storage/src/migrations.rs`

**Interfaces:**
- Consumes: SQLite DB structure.
- Produces: DB tables `weight_snapshots` and `feedback_events`.

- [ ] **Step 1: Write migration Version 6**
  In `crates/brain-storage/src/migrations.rs`, update `MIGRATIONS` or `apply_migrations` to add Version 6:
  ```rust
  // Inside crates/brain-storage/src/migrations.rs
  // Find where migrations are defined and append version 6:
  ```
  Let's verify the exact migration structure. In Phase 10 we altered the migrations, let's view it first.
  Let's do this step once the implementation begins.

- [ ] **Step 2: Run tests to verify migration completes successfully**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-storage`
  Expected: PASS

- [ ] **Step 3: Commit**
  Run: `git add crates/brain-storage/src/migrations.rs && git commit -m "migration: add version 6 schema for weights and feedback"`

---

### Task 3: SQLite Storage Serializers

**Files:**
- Modify: `crates/brain-storage/src/store.rs`

**Interfaces:**
- Consumes: SQLite DB tables.
- Produces: `save_weight_snapshot`, `get_weight_snapshot`, `list_all_weight_snapshots`, `save_feedback_event`, `list_all_feedback_events`.

- [ ] **Step 1: Write tests for storage serializers**
  Add unit tests in `crates/brain-storage/tests/storage_tests.rs` verifying that `WeightSnapshot` and `FeedbackEvent` are successfully saved and loaded.

- [ ] **Step 2: Implement save/load functions in `store.rs`**
  Write functions to parse/convert domain structures into SQL columns, encoding `CalibrationMetadata` and context fields to JSON strings.

- [ ] **Step 3: Run storage tests**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-storage`
  Expected: PASS

- [ ] **Step 4: Commit**
  Run: `git add crates/brain-storage/src/store.rs crates/brain-storage/tests/storage_tests.rs && git commit -m "feat: implement SQLite serialization for snapshots and feedback"`

---

### Task 4: ActiveWeightProvider & LearnedScorer Integration

**Files:**
- Create: `crates/brain-services/src/retrieval/active_weights.rs`
- Modify: `crates/brain-services/src/retrieval/temporal.rs`
- Modify: `crates/brain-services/src/lib.rs`

**Interfaces:**
- Consumes: `WeightSnapshot`
- Produces: `ActiveWeightProvider` trait, `LearnedTemporalScorer` (as a `RankingStrategy`).

- [ ] **Step 1: Define `ActiveWeightProvider` and implement `DefaultActiveWeightProvider`**
  ```rust
  // crates/brain-services/src/retrieval/active_weights.rs
  use std::sync::{Arc, RwLock};
  use brain_core::errors::BrainError;
  use brain_domain::retrieval::models::WeightSnapshot;

  pub trait ActiveWeightProvider: Send + Sync {
      fn active_snapshot(&self) -> Result<Arc<WeightSnapshot>, BrainError>;
      fn swap_active(&self, new_snapshot: WeightSnapshot) -> Result<(), BrainError>;
  }

  pub struct DefaultActiveWeightProvider {
      active: RwLock<Arc<WeightSnapshot>>,
  }
  ```

- [ ] **Step 2: Integrate `LearnedTemporalScorer`**
  Modify `crates/brain-services/src/retrieval/temporal.rs` to add `LearnedTemporalScorer` implementing `RankingStrategy`. It should evaluate features (semantic score, pagerank/degree graph score, recency decay, temporal density) using `RankingSignals`, retrieve weights from `ActiveWeightProvider`, and score using `LinearRankingModel`.

- [ ] **Step 3: Write tests verifying integration**
  Write tests in `crates/brain-services/tests/temporal_calibration_tests.rs` to verify that `LearnedTemporalScorer` correctly weights signals.

- [ ] **Step 4: Commit**
  Run: `git commit -m "feat: integrate ActiveWeightProvider and LearnedTemporalScorer"`

---

### Task 5: CalibrationEngine & WeightCalibrationService

**Files:**
- Modify: `crates/brain-services/src/retrieval/calibration.rs`
- Modify: `crates/brain-services/src/lib.rs`

**Interfaces:**
- Consumes: `FeedbackEvent`, `CalibrationPolicy`
- Produces: `CalibrationEngine` (pure computation), `WeightCalibrationService` (orchestration, validation, atomic publication).

- [ ] **Step 1: Write `CalibrationEngine`**
  Implement the optimization loop:
  Given feedback events and the current snapshot, adjust weights using gradient-descent rules (shifting weight towards features of selected nodes and away from unselected nodes), outputting a candidate `WeightSnapshot` and `CalibrationReport`.

- [ ] **Step 2: Write `WeightCalibrationService`**
  Coordinate storage transactions: load events, call `CalibrationEngine`, validate invariants (completeness, non-negativity), write candidate snapshot to DB, and perform atomic swap using `ActiveWeightProvider`.

- [ ] **Step 3: Write tests for calibration loop**
  Create integration tests validating convergence, determinism, and reproducibility.

- [ ] **Step 4: Commit**
  Run: `git commit -m "feat: implement CalibrationEngine and WeightCalibrationService"`

---

### Task 6: Determinism & Invariant Tests

**Files:**
- Create: `crates/brain-services/tests/learned_ranking_invariants_tests.rs`

**Interfaces:**
- Consumes: Full ranking and calibration pipeline.
- Produces: Invariant verification suite.

- [ ] **Step 1: Write tests for Model Transparency, Calibration Reproducibility, Snapshot Monotonicity**
- [ ] **Step 2: Run all tests to guarantee correctness**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
  Expected: PASS
- [ ] **Step 3: Commit**
  Run: `git add crates/brain-services/tests/learned_ranking_invariants_tests.rs && git commit -m "test: verify Phase 12 invariants"`
