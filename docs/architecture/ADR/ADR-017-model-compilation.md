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

## Consequences
* **Performance**: The compiler can flatten recursion, pre-allocate nodes sequentially, or precompute lookup arrays.
* **Flexibility**: We can optimize compiled layouts for latency or memory without altering the serialized definition schema or breaking database backward compatibility.
* **Initialization Cost**: Minor overhead at startup during compilation, which is offset by faster online evaluation speeds.
