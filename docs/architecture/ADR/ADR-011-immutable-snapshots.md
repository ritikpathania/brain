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

## Consequences
* **Safety**: Active ranking calculations cannot experience race conditions since weight definitions are read-only and immutable.
* **Audit Trail**: Every request can be traced back to a specific, immutable snapshot version, enabling complete historical auditing and replication.
* **Storage**: Keeping old versions requires disk storage, though negligible given the small footprint of model weight parameters.
