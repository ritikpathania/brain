# ADR-012: Value Objects

## Status
Accepted

## Context
Raw primitives (like `f64` or `String`) do not enforce business limits or mathematical bounds. Allowing weights, metrics, normalizations, and splits to be represented by primitive floats introduces risks of silent arithmetic bugs (such as division by zero, NaNs, or infinite bounds).

## Decision
We enforce a value-object-first design pattern. Raw primitives are wrapped in domain value objects that validate values during construction:
1. `RankingWeight` validates that values are non-negative and finite.
2. `NormalizedSignal` and `MetricScore` (e.g. `NdcgScore`, `MrrScore`) validate values lie strictly within the range `[0.0, 1.0]`.
3. `SplitThreshold` and `LeafScore` validate that inputs are finite.
4. Constructors return explicit validation errors instead of silently masking invalid bounds.

## Alternatives Considered
* **Primitive Floats**: Storing naked `f64` across all equations. Rejected because of risks of silent math overflows or floating-point anomalies.
* **Type Aliases**: Declaring `type NormalizedSignal = f64`. Rejected because aliases do not provide compile-time safety or runtime validation checking.

## Related ADRs
* [ADR-010 (Domain Boundaries)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-010-domain-boundaries.md)
* [ADR-016 (Pure Transformation Pipelines)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-016-pure-transformation-pipelines.md)

## Expected Stability
Long-term.
* **Review Trigger**: Extreme performance bottlenecks where CPU profiling shows object allocation/validation checking in tight mathematical loops is a blocker.
