# Design Spec: Learned/Adaptive Temporal Ranking (Phase 12)

This specification details the architecture and database schema for Phase 12 (Learned/Adaptive Temporal Ranking) to improve cognitive retrieval quality while preserving execution reproducibility and determinism.

---

## 1. Architectural Overview

We introduce a hybrid serving/learning model with a strict boundary between online retrieval (serving) and offline/batch calibration (learning).

```
Online Retrieval Pipeline                        Calibration / Learning Loop (Offline)
┌─────────────────────────────────┐              ┌─────────────────────────────┐
│      TemporalProjector          │              │        FeedbackEvents       │
└──────────────┬──────────────────┘              └──────────────┬──────────────┘
               │                                                │
               ▼                                                ▼
┌─────────────────────────────────┐              ┌─────────────────────────────┐
│      Visible Candidates         │              │  WeightCalibrationService   │
└──────────────┬──────────────────┘              └──────────────┬──────────────┘
               │                                                │ (Compute new weights)
               ▼                                                ▼
┌─────────────────────────────────┐              ┌─────────────────────────────┐
│        Ranking Pipeline         │ ◄─────────── │     Atomic Swap / Publish   │
│  (Semantic, Graph, Recency,     │              └─────────────────────────────┘
│   LearnedTemporalScorer)        │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│          Final Ranking          │
└─────────────────────────────────┘
```

### Components

1. **`WeightSnapshot`**:
   * An immutable domain value object divided cleanly into metadata and weights.
   * `SnapshotMetadata` tracks model lineage, version, and training contexts.
   * `RankingWeights` encapsulates active scoring multipliers, immutable by construction with explicit getters.

2. **`FeedbackEvent`**:
   * Represents a single record of user interaction / relevance feedback. Used as batch inputs for training and calibrating weights. Consists of a version tag to support schema updates.

3. **`RankingModel` & `LinearRankingModel`**:
   * `RankingModel` is a polymorphic interface for evaluating candidate signals, tracking its `RankingModelVersion`.
   * `LinearRankingModel` implements `RankingModel` by performing a weighted sum of normalized ranking features.

4. **`ActiveWeightProvider`**:
   * An abstraction trait defined in the **services layer** (`brain-services`) that provides the active immutable `WeightSnapshot` snapshot. The scorer remains unaware of the backing storage mechanism.

5. **`WeightCalibrationService`**:
   * Orchestrates the ingestion of feedback events, trains/calibrates parameter weights using a defined `CalibrationPolicy` (returning a candidate snapshot and `CalibrationReport`), and publishes snapshots atomically.

---

## 2. Invariants & Rules

1. **Snapshot Determinism**:
   * A fixed `WeightSnapshot` and identical input query must always yield exactly identical scoring outputs and rankings.
2. **Atomic Publication**:
   * A query context must observe exactly one `WeightSnapshot` version for its entire lifecycle. Updates to weights are swapped atomically.
3. **Version Reproducibility**:
   * Replaying a query against a fixed graph and snapshot version produces identical rankings.
4. **Calibration Reproducibility**:
   * Given identical input feedback events, calibration policy, and starting snapshot, the calibration process must generate a byte-for-byte identical candidate snapshot.
5. **Calibration Isolation**:
   * Calibration sweeps must not mutate graphs, temporal visibility projection sets, query evidence, or the volatile caches. Only ranking output weights change.
6. **Weight Snapshot Completeness & Atomic Replacement**:
   * Every published snapshot must define all ranking weights. Partial weight updates or live weight mutations are rejected; publication always atomically replaces the entire active snapshot (e.g. `v7 -> v8`).
7. **Model Transparency**:
   * Changing the ranking model implementation while keeping identical model version, weights, and signals must not change ranking outputs.
8. **A/B Ready & Rollback Safe**:
   * Snapshot history is preserved in the database to allow rolling back to previous weight versions.

---

## 3. Database Schema Extensions (Migration Version 6)

### Table: `weight_snapshots`
```sql
CREATE TABLE weight_snapshots (
    version INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL,
    semantic_weight REAL NOT NULL,
    graph_weight REAL NOT NULL,
    recency_weight REAL NOT NULL,
    temporal_weight REAL NOT NULL,
    calibration_metadata TEXT NOT NULL  -- JSON representation of CalibrationMetadata
);

-- Insert initial default weights as version 1
INSERT INTO weight_snapshots (version, created_at, semantic_weight, graph_weight, recency_weight, temporal_weight, calibration_metadata)
VALUES (1, strftime('%s','now'), 1.0, 1.0, 1.0, 1.0, '{}');
```

### Table: `feedback_events`
```sql
CREATE TABLE feedback_events (
    id TEXT PRIMARY KEY,
    schema_version INTEGER NOT NULL,
    query TEXT NOT NULL,
    node_id TEXT NOT NULL,
    selected INTEGER NOT NULL, -- 0 or 1
    timestamp INTEGER NOT NULL,
    ranking_position INTEGER NOT NULL,
    context TEXT NOT NULL  -- JSON payload for extensible metadata
);
```

---

## 4. Domain & Service Details

### Value Objects & Core Domain Types

```rust
/// Non-negative, finite ranking multiplier value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
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

/// Normalized ranking signal in range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
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

/// Monotonically incrementing snapshot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SnapshotVersion(u64);

impl SnapshotVersion {
    pub fn new(val: u64) -> Self {
        Self(val)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Extensible structured calibration details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
pub struct SnapshotMetadata {
    pub version: SnapshotVersion,
    pub created_at: crate::temporal::TimePoint,
    pub calibration_metadata: CalibrationMetadata,
}

/// Scoring multipliers, immutable by construction.
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

pub struct WeightSnapshot {
    pub metadata: SnapshotMetadata,
    pub weights: RankingWeights,
}

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
pub struct RankingSignals {
    pub semantic: NormalizedSignal,
    pub graph: NormalizedSignal,
    pub recency: NormalizedSignal,
    pub temporal: NormalizedSignal,
}

/// Version tag for calibration policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CalibrationPolicyVersion(u32);

/// Parameter knobs configuring calibration loops.
pub struct CalibrationPolicy {
    pub version: CalibrationPolicyVersion,
    pub learning_rate: f64,
    pub regularization: f64,
    pub min_feedback_events: usize,
}

/// Immutable calibration result parameters.
pub struct CalibrationReport {
    pub candidate_version: SnapshotVersion,
    pub previous_version: SnapshotVersion,
    pub policy_version: CalibrationPolicyVersion,
    pub feedback_processed: usize,
    pub validation_loss: f64,
    pub convergence_information: String,
    pub publication_decision: bool,
}
```

### Abstractions & Models

```rust
/// Version identifier tracking the model implementation structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RankingModelVersion {
    V1Linear,
}

/// Polymorphic interface for scoring candidate nodes.
pub trait RankingModel: Send + Sync {
    fn version(&self) -> RankingModelVersion;
    fn score(&self, signals: &RankingSignals) -> f64;
}

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

### Services Abstractions (`brain-services`)

```rust
/// Abstraction providing the active immutable weight snapshot.
pub trait ActiveWeightProvider: Send + Sync {
    fn active_snapshot(&self) -> Result<std::sync::Arc<WeightSnapshot>, crate::errors::BrainError>;
}

/// Scorer wrapper for candidate retrieval.
pub struct LearnedTemporalScorer {
    pub model: std::sync::Arc<dyn RankingModel>,
}
```

### WeightCalibrationService

* Exposes:
  * `ingest_feedback(&self, event: FeedbackEvent) -> Result<(), BrainError>`: Persists events.
  * `calibrate_weights(&self, policy: &CalibrationPolicy) -> Result<(WeightSnapshot, CalibrationReport), BrainError>`: Computes a candidate snapshot and generates its corresponding report, without publishing.
  * `publish_snapshot(&self, snapshot: WeightSnapshot) -> Result<(), BrainError>`: Atomically swaps the active snapshot version in storage/memory.
