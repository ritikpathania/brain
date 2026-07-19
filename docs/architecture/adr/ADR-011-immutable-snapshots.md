# ADR-011: Immutable Snapshots

## Status
Accepted

## Context
Online serving layers require stable weights and experiment configurations. If models are calibrated or modified directly in place on active serving structures, the ranking behavior becomes non-deterministic, making reproducibility, rollback, and A/B test auditing impossible.

## Decision
We model all weights and experiments as versioned, immutable snapshot snapshots:
1. `WeightSnapshot` holds metadata (creation timestamp, loss) and immutable `RankingWeights`.
2. `ExperimentConfiguration` holds registered variants and allocations under an immutable configuration version.
3. Swaps to active structures (via `ActiveWeightProvider` or `ExperimentRouter`) are executed atomically by replacing references, never by mutating fields of an existing snapshot in-place.
4. Old snapshots are preserved in the relational repository as read-only records.

## Alternatives Considered
* **In-Place DB Updates**: Overwriting active weight records in SQLite directly. Rejected because active runs could query corrupted/in-transition weights, making rollback impossible.
* **Mutable Cache**: Maintaining an in-memory mutable cache of weights. Rejected because of multi-threading race conditions and lock-contention overhead.

## Related ADRs
* [ADR-014 (Deterministic Execution)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-014-deterministic-execution.md)

## Expected Stability
Long-term.
* **Review Trigger**: Requirements for streaming real-time weight deltas that exceed the latency threshold of snapshot replacement.
