# ADR-018: Reproducible ML Lifecycle

## Status
Proposed

## Context
As calibration, evaluation, and experiment routing scale, we need to ensure that model promotions are fully reproducible, auditable, and rollback-capable. Ad-hoc calibrations or promotions without standardized evaluation guarantees lead to regression risks.

## Decision
Every model variant must navigate a standardized, reproducible lifecycle path:
```
Feedback ──► Calibration ──► Offline Evaluation ──► Publication Policy ──► Experiment Routing ──► Promotion / Retirement
```
1. **Feedback**: Structured interaction logs are recorded as feedback records.
2. **Calibration**: Generates a versioned `WeightSnapshot` containing calibrated parameters.
3. **Offline Evaluation**: The candidate snapshot is evaluated over an `EvaluationDataset` producing an `EvaluationReport`.
4. **Publication Policy**: An automated `PublicationPolicy` decides if the report meets the improvement thresholds.
5. **Experiment Routing**: A routed experiment launches variant splits deterministically.
6. **Promotion**: Successful candidates are promoted to the active baseline; unsuccessful ones are retired.

## Alternatives Considered
* **Ad-Hoc Promotion**: Direct deployment of calibrated weights without metric checks. Rejected due to severe regression risks in production.

## Related ADRs
* [ADR-011 (Immutable Snapshots)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-011-immutable-snapshots.md)
* [ADR-014 (Deterministic Execution)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-014-deterministic-execution.md)

## Consequences
* **Regression Safety**: Models cannot be promoted without satisfying metric thresholds.
* **Auditability**: Complete audit trails are preserved from raw feedback to final variant promotions.

## Expected Stability
Long-term.
