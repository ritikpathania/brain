# ADR-019: Observability First

## Status
Proposed

## Context
Operating a dynamic ML platform requires deep operational visibility. If feature normalizations, routing decisions, and calibration runs occur inside a "black box", identifying feature drift or debugging routing bugs is extremely difficult.

## Decision
We treat observability as an architectural concern by explicitly surfacing telemetry records:
1. `RoutingDecision`: Telemetry object capturing version details and sticky routing justifications.
2. `EvaluationReport` & `PerQueryEvaluation`: Expose granular query metrics, baseline comparison deltas, and explanations.
3. Feature importance data and calibration history are exposed via standard API and diagnostic interfaces.

## Alternatives Considered
* **Implicit Logging**: Standard standard-out text logs. Rejected because structured diagnostic structures are required for automated dashboards.

## Related ADRs
* [ADR-011 (Immutable Snapshots)](ADR-011-immutable-snapshots.md)
* [ADR-018 (Reproducible ML Lifecycle)](ADR-018-reproducible-ml-lifecycle.md)

## Expected Stability
Long-term.
