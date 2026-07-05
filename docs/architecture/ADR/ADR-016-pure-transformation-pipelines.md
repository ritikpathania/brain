# ADR-016: Pure Transformation Pipelines

## Status
Accepted

## Context
As subsystems scale, mixing side-effects (such as database writes, I/O updates, or network calls) with processing logic makes systems difficult to reason about. It leads to unpredictable bugs, makes test mock setups complex, and compromises execution determinism.

## Decision
We enforce a design pattern where all primary subsystems are structured as pure transformation pipelines:
```
Immutable Input ──► Pure Transformation ──► Immutable Output
```
1. **Temporal Projection**: `TemporalProjector` processes immutable `TemporalEdge` slices to return `TemporalSnapshot`.
2. **Feature Extraction**: `FeatureExtractor` takes raw candidates and returns `RawFeatureVector`.
3. **Normalization**: `FeatureNormalizer` scales a `RawFeatureVector` to produce `RankingSignals`.
4. **Calibration**: `CalibrationEngine` reads immutable `FeedbackEvent` records to optimize and output `WeightSnapshot`.
5. **Evaluation**: `OfflineEvaluator` scores snapshots against an `EvaluationDataset` to yield `EvaluationReport`.

## Consequences
* **Side-Effect-Free**: Pipelines calculate outputs in-memory without mutating external databases or modifying runtime states.
* **Composability**: Subsystems can be chained together easily because outputs of one pipeline serve as inputs to the next.
* **Zero Bias**: Isolating mutations guarantees that processing can be re-run indefinitely without causing drift or introducing execution bias.
