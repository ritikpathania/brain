# ADR-013: Behavioral Invariants

## Status
Accepted

## Context
Standard assertions (checking if `rank() == [A, B]`) are fragile because they break whenever retrieval weights, calibration logic, or candidate profiles change. Testing against specific data instances leads to test churn and fails to guarantee core mathematical truths.

## Decision
We write test assertions against core behavioral and mathematical invariants instead of specific data values:
1. **Model Transparency**: Zero weights on a signal must yield identical scoring contributions, asserting transparency.
2. **Normalization Stability**: Bounds checking ensures raw vectors mapped to `[0.0, 1.0]` do not shift order.
3. **Feature Isolation**: Changing one feature dimension must not alter the calculation of unrelated feature dimensions.
4. **Metric Monotonicity**: Increasing document relevance scores must never decrease calculated DCG or NDCG.
5. **Routing Stability**: Hashing identical keys over identical configurations must yield identical decisions.
6. **Allocation Conservation**: Allocations must sum to exactly 1.0.

## Alternatives Considered
* **Golden Snapshot Files**: Writing tests that compare outputs against static text logs. Rejected due to high test churn during calibration updates.
* **Integration-Only Verification**: Relying entirely on manual/e2e checks. Rejected due to inability to pinpoint core mathematical bugs.

## Related ADRs
* [ADR-012 (Value Objects)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-012-value-objects.md)
* [ADR-014 (Deterministic Execution)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-014-deterministic-execution.md)

## Expected Stability
Long-term.
