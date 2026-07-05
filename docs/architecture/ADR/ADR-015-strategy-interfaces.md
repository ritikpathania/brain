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

## Consequences
* **Extensibility**: Adding a new ranking model (e.g. ensemble models) or a geo-based experiment router is done by implementing the trait. Serving orchestrators remain unchanged.
* **Test Isolation**: Individual strategy implementations can be validated in isolation using unit tests.
* **Indirection**: Minor performance overhead due to virtual dispatch, though typically optimized out by compiler monomorphization.
