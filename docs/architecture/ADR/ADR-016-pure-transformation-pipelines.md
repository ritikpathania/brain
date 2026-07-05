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

## Alternatives Considered
* **Stateful Orchestrators**: In-place mutation of caches and repositories during calculation runs. Rejected because of synchronization overhead and race conditions.

## Related ADRs
* [ADR-010 (Domain Boundaries)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-010-domain-boundaries.md)
* [ADR-012 (Value Objects)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-012-value-objects.md)

## Consequences & Tradeoffs
* **Allocations**: Increases short-lived heap allocations and intermediate copy creation overhead during calculations, though mitigated by Rust's efficient memory management.

## Expected Stability
Long-term.
