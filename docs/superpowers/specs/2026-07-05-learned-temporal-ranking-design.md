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
   * An immutable domain value object containing adaptive weight multipliers for ranking signals:
     * `semantic_weight`
     * `graph_weight`
     * `recency_weight`
     * `temporal_weight` (based on temporal span/interaction signals)
   * Includes metadata (version, creation time, calibration details) to guarantee complete version reproducibility.

2. **`FeedbackEvent`**:
   * Represents a single record of user interaction / relevance feedback. Used as batch inputs for training and calibrating weights.

3. **`LearnedTemporalScorer`**:
   * A pure, deterministic `RankingStrategy` component.
   * Reads from an immutable `WeightSnapshot` snapshot. It never mutates state or updates weights itself.

4. **`WeightCalibrationService`**:
   * Orchestrates the ingestion of feedback events, trains/calibrates parameter weights, validates target invariants (e.g. weight boundaries), and performs an atomic publish swap of the active snapshot.

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
5. **A/B Ready & Rollback Safe**:
   * Snapshot history is preserved in the database to allow rolling back to previous weight sets.

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
    calibration_metadata TEXT NOT NULL  -- JSON payload with training details
);

-- Insert initial default weights as version 1
INSERT INTO weight_snapshots (version, created_at, semantic_weight, graph_weight, recency_weight, temporal_weight, calibration_metadata)
VALUES (1, strftime('%s','now'), 1.0, 1.0, 1.0, 1.0, '{}');
```

### Table: `feedback_events`
```sql
CREATE TABLE feedback_events (
    id TEXT PRIMARY KEY,
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

### Domain Entities

```rust
pub struct WeightSnapshot {
    pub version: u64,
    pub created_at: u64,
    pub semantic_weight: f64,
    pub graph_weight: f64,
    pub recency_weight: f64,
    pub temporal_weight: f64,
    pub calibration_metadata: String,
}

pub struct FeedbackEvent {
    pub id: String,
    pub query: String,
    pub node_id: NodeId,
    pub selected: bool,
    pub timestamp: u64,
    pub ranking_position: usize,
    pub context: String,
}
```

### LearnedTemporalScorer

* Combines node signals:
  $$\text{LearnedScore}_i = w_{sem} \cdot S_{sem} + w_{graph} \cdot S_{graph} + w_{rec} \cdot S_{rec} + w_{temp} \cdot S_{temp}$$
  where:
  - $S_{sem}$: Normalized semantic vector or keyword similarity.
  - $S_{graph}$: Structural graph pagerank / degree centrality centrality.
  - $S_{rec}$: Decay recency preference score.
  - $S_{temp}$: Interaction density score (number of temporal edge observations involving node $i$).

### WeightCalibrationService

* Exposes:
  * `ingest_feedback(event: FeedbackEvent)`: Persists user actions.
  * `calibrate_weights(alpha: f64)`: Evaluates accumulated feedback and performs an optimization step (e.g. gradient adjustment or moving average update towards selected feature densities), generating and validating a new `WeightSnapshot`.
  * `publish_snapshot(snapshot: WeightSnapshot)`: Atomically swaps the active snapshot version.
