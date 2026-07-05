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

## Consequences
* **Incorrect States Excluded**: It is mathematically impossible to instantiate a ranking model or evaluation metrics containing NaNs or out-of-range floats.
* **Self-Documenting Code**: Types describe their mathematical limits (e.g. passing `NdcgScore` instead of a naked `f64`).
* **Ergonomics**: Code requires calling `.value()` to extract the inner primitive, and handling `Result` packages during construction.
