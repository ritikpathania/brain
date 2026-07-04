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
   * `RankingWeights` encapsulates active scoring multipliers.

2. **`FeedbackEvent`**:
   * Represents a single record of user interaction / relevance feedback. Used as batch inputs for training and calibrating weights. Consists of a version tag to support schema updates.

3. **`LearnedTemporalScorer`**:
   * A pure, deterministic `RankingStrategy` component.
   * Computes fused ranking scores purely based on a `RankingSignals` input and active `RankingWeights` weights.

4. **`WeightCalibrationService`**:
   * Orchestrates the ingestion of feedback events, trains/calibrates parameter weights using a defined `CalibrationPolicy`, validates target invariants, and publishes new `WeightSnapshot` versions alongside a `CalibrationReport`.

---

## 2. Invariants & Rules

1. **Snapshot Determinism**:
   * A fixed `WeightSnapshot` and identical input query must always yield exactly identical scoring outputs and rankings.
2. **Atomic Publication**:
   * A query context must observe exactly one `WeightSnapshot` version for its entire lifecycle. Updates to weights are swapped atomically.
3. **Version Reproducibility**:
   * Replaying a query against a fixed graph and snapshot version produces identical rankings.
4. **Calibration Isolation**:
   * Calibration sweeps must not mutate graphs, temporal visibility projection sets, query evidence, or the volatile caches. Only ranking output weights change.
5. **Weight Snapshot Completeness**:
   * Every published snapshot must define all ranking weights. Partial weight updates are rejected.
6. **A/B Ready & Rollback Safe**:
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
    pub algorithm_used: String,
    pub validation_loss: Option<f64>,
}

/// Metadata tracking weight lineage.
pub struct SnapshotMetadata {
    pub version: SnapshotVersion,
    pub created_at: crate::temporal::TimePoint,
    pub calibration_metadata: CalibrationMetadata,
}

/// Scoring multipliers.
pub struct RankingWeights {
    pub semantic: RankingWeight,
    pub graph: RankingWeight,
    pub recency: RankingWeight,
    pub temporal: RankingWeight,
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
    pub semantic: f64,
    pub graph: f64,
    pub recency: f64,
    pub temporal: f64,
}

/// Immutable calibration result parameters.
pub struct CalibrationReport {
    pub snapshot_version: SnapshotVersion,
    pub feedback_processed: usize,
    pub validation_loss: f64,
    pub convergence_information: String,
    pub publication_decision: bool,
}

/// Parameter knobs configuring calibration loops.
pub struct CalibrationPolicy {
    pub learning_rate: f64,
    pub regularization: f64,
    pub min_feedback_events: usize,
}
```

### LearnedTemporalScorer

* Evaluates candidate signals:
  ```rust
  pub struct LearnedTemporalScorer;
  
  impl LearnedTemporalScorer {
      pub fn score(signals: &RankingSignals, weights: &RankingWeights) -> f64 {
          weights.semantic.value() * signals.semantic
              + weights.graph.value() * signals.graph
              + weights.recency.value() * signals.recency
              + weights.temporal.value() * signals.temporal
      }
  }
  ```

### WeightCalibrationService

* Exposes:
  * `ingest_feedback(&self, event: FeedbackEvent) -> Result<(), BrainError>`: Persists events.
  * `calibrate_weights(&self, policy: &CalibrationPolicy) -> Result<CalibrationReport, BrainError>`: Evaluates events, calculates optimized weights, performs safety checks, and swaps the active snapshot version.
