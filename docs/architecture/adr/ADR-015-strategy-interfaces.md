# ADR-015: Strategy Interfaces

## Status
Accepted

## Context
Hardcoding specific model rules, routing setups, or publication check policies directly within retrieval loops makes it difficult to introduce new features. It leads to code churn and risks regressions in unrelated areas when extending the system.

## Decision
We decouple system operations behind strategy traits:
1. `RankingModel`: Abstract trait defining the polymorphic model evaluation interface (`LinearRankingModel`, `DecisionTreeRankingModel`).
2. `FeatureNormalizer`: Deconstructs normalization implementations (`MinMaxNormalizer`).
3. `PublicationPolicy`: Abstracts snapshot publication logic (`NoRegressionPolicy`).
4. `ExperimentRouter`: Abstracts retrieval request routing models (`DefaultExperimentRouter`, `CanaryExperimentRouter`).

## Alternatives Considered
* **Enums/Conditional Matches**: Representing all routing/ranking models as a single enum and matching on them. Rejected due to high code churn whenever new models or experiment variants are added.
* **Macros**: Using Rust macros for compile-time generation. Rejected because it complicates debugging and decreases IDE navigation usability.

## Related ADRs
* [ADR-010 (Domain Boundaries)](ADR-010-domain-boundaries.md)
* [ADR-017 (Model Compilation)](ADR-017-model-compilation.md)

## Expected Stability
Long-term.
* **Review Trigger**: Supporting distributed model serving or dynamic Rust plugin loadings (e.g. `wasm` modules).
