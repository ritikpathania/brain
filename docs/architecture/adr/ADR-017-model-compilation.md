# ADR-017: Model Compilation

## Status
Accepted

## Context
Non-linear ranking models (such as decision trees) contain complex, hierarchical evaluation paths. Resolving these paths recursively over serializable representations in online retrieval loops is slow and limits performance optimizations.

## Decision
We decouple model representation into serializable definitions and compiled layout representations:
1. `DecisionTreeDefinition` is the immutable, serializable representation stored in databases or sent across FFI layers.
2. `DecisionTreeCompiler` compiles definitions at load time.
3. `CompiledDecisionTree` executes inference runs on compiled, optimized memory structures.
4. `DecisionTreeRankingModel` wraps both to keep the original definition immutable while serving score calculations from the compiled structure.

## Alternatives Considered
* **Direct Interpretation**: Scanning tree leaves recursively on raw definition JSON objects. Rejected because of high latency and runtime parsing costs.
* **JIT Compilation**: Dynamic runtime code generation. Rejected due to security concerns, platform portability issues, and complex toolchain dependency.

## Related ADRs
* [ADR-015 (Strategy Interfaces)](ADR-015-strategy-interfaces.md)

## Consequences & Tradeoffs
* **Startup / Load Latency**: Introduces a compilation step when instantiating the scoring model.
* **Pipeline Complexity**: Developers must maintain both the serialized definition models and the compiler paths.

## Expected Stability
Long-term.
* **Review Trigger**: The dynamic model size scales past memory limitations, requiring cache compilation designs.
