# Learned/Adaptive Temporal Ranking (Phase 12) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement learned and adaptive temporal ranking to dynamically calibrate feature weights based on user relevance feedback without mutating projection visibility states.

**Architecture:** Introduces immutable, versioned `WeightSnapshot` configurations read by `LearnedTemporalScorer` (via a polymorphic `RankingModel`). Ingested feedback events are processed by `CalibrationEngine` using a pluggable `CalibrationAlgorithm` (such as the initial v1 `LinearAdjustmentAlgorithm`). Active snapshots are atomically swapped by `WeightCalibrationService`.

**Tech Stack:** Rust, SQLite, Serde (JSON serialization).

## Global Constraints

- **Snapshot Determinism**: Fixed `WeightSnapshot` and identical input query must always yield exactly identical scoring outputs.
- **Atomic Publication**: Query context observes exactly one snapshot version; publication atomically replaces the entire active snapshot.
- **Version Reproducibility**: Replaying a query against a fixed graph and snapshot version produces identical rankings.
- **Calibration Reproducibility**: Given identical input feedback, policy, and starting snapshot, calibration generates a byte-for-byte identical candidate snapshot.
- **Calibration Idempotence**: Calibrating with no new feedback events yields the current active snapshot without generating a new candidate snapshot version.
- **Calibration Isolation**: Calibration runs never mutate graphs, visibility projection sets, query evidence, or volatile caches.
- **Weight Snapshot Completeness**: Every published snapshot defines all ranking weights.
- **Model Transparency**: Under identical version, weights, and signals, model scoring must be identical.
- **Snapshot Monotonicity**: Published snapshot versions must satisfy `v1 < v2 < v3 < ...`.
- **A/B Ready & Rollback Safe**: Active weight snapshots can be reverted/rolled back cleanly to any valid historical version.

---

### Task 1: Domain Value Objects & Models

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
  Add definitions to `crates/brain-domain/src/retrieval/models.rs`:
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
          let bits = self.0.to_bits();
          state.write_u64(bits);
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
          let bits = self.0.to_bits();
          state.write_u64(bits);
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

  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

  /// Enumeration of calibration algorithms.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub enum CalibrationAlgorithmType {
      LinearAdjustment,
  }

  /// Parameter knobs configuring calibration loops.
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct CalibrationPolicy {
      pub version: CalibrationPolicyVersion,
      pub algorithm: CalibrationAlgorithmType,
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
  ```sql
  CREATE TABLE weight_snapshots (
      version INTEGER PRIMARY KEY,
      created_at INTEGER NOT NULL,
      semantic_weight REAL NOT NULL,
      graph_weight REAL NOT NULL,
      recency_weight REAL NOT NULL,
      temporal_weight REAL NOT NULL,
      calibration_metadata TEXT NOT NULL
  );
  CREATE TABLE feedback_events (
      id TEXT PRIMARY KEY,
      schema_version INTEGER NOT NULL,
      query TEXT NOT NULL,
      node_id TEXT NOT NULL,
      selected INTEGER NOT NULL,
      timestamp INTEGER NOT NULL,
      ranking_position INTEGER NOT NULL,
      context TEXT NOT NULL
  );
  ```

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
  use std::sync::Arc;
  use brain_core::errors::BrainError;
  use brain_domain::retrieval::models::WeightSnapshot;

  pub trait ActiveWeightProvider: Send + Sync {
      fn active_snapshot(&self) -> Result<Arc<WeightSnapshot>, BrainError>;
      fn swap_active(&self, new_snapshot: WeightSnapshot) -> Result<(), BrainError>;
  }
  ```

- [ ] **Step 2: Integrate `LearnedTemporalScorer`**
  Modify `crates/brain-services/src/retrieval/temporal.rs` to add `LearnedTemporalScorer` implementing `RankingStrategy`. It should evaluate features (semantic score, pagerank/degree graph score, recency decay, temporal density) using `RankingSignals`, retrieve weights from `ActiveWeightProvider`, and score using `LinearRankingModel`.

- [ ] **Step 3: Write tests verifying integration**
  Write tests in `crates/brain-services/tests/temporal_calibration_tests.rs` to verify that `LearnedTemporalScorer` correctly weights signals.

- [ ] **Step 4: Commit**
  Run: `git commit -m "feat: integrate ActiveWeightProvider and LearnedTemporalScorer"`

---

### Task 5: Calibration Algorithm & CalibrationEngine

**Files:**
- Modify: `crates/brain-domain/src/retrieval/models.rs`
- Create: `crates/brain-services/tests/calibration_engine_tests.rs`

**Interfaces:**
- Consumes: `FeedbackEvent`, `CalibrationPolicy`
- Produces: `CalibrationAlgorithm` trait, `LinearAdjustmentAlgorithm` implementation, `CalibrationEngine`.

- [ ] **Step 1: Write `CalibrationAlgorithm` and `LinearAdjustmentAlgorithm`**
  Implement linear moving-average heuristics that deterministically adjust weights:
  - If a feedback event has `selected = true`, increase the weights corresponding to its active signals slightly.
  - If `selected = false`, decrease weights slightly.
  - Perform validation bounds checking to keep weights finite, non-negative, and fully populated.

- [ ] **Step 2: Implement `CalibrationEngine`**
  Pure orchestrator to select `CalibrationAlgorithm` and run optimization, producing a candidate `WeightSnapshot` and `CalibrationReport` without publication.

- [ ] **Step 3: Write validation and idempotence tests**
  Verify that when 0 events are processed, `LinearAdjustmentAlgorithm` returns the current active snapshot with zero validation loss.

- [ ] **Step 4: Commit**
  Run: `git commit -m "feat: implement pluggable CalibrationAlgorithm and CalibrationEngine"`

---

### Task 6: WeightCalibrationService & Rollback Orchestration

**Files:**
- Create/Modify: `crates/brain-services/src/retrieval/calibration.rs`

**Interfaces:**
- Consumes: `CalibrationEngine`, `ActiveWeightProvider`, SQLite storage backend
- Produces: `WeightCalibrationService` (`ingest_feedback`, `calibrate_weights`, `publish_snapshot`, `rollback_to`).

- [ ] **Step 1: Implement `WeightCalibrationService`**
  Coordinate database queries, snapshot monotonicity checking (`candidate_version > current_version`), atomic swap updating `ActiveWeightProvider`, and explicit `rollback_to(version)` to revert the active version.

- [ ] **Step 2: Run verification and tests**
  Write `publication_tests.rs` and `reproducibility_tests.rs` to verify safe publishing, idempotence, rollbacks, and monotonicity limits.

- [ ] **Step 3: Commit**
  Run: `git commit -m "feat: implement WeightCalibrationService with publication and rollback safety"`
