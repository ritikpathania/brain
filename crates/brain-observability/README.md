# brain-observability

## Purpose
Structured logging, tracing subscribers, and diagnostic telemetry capture.

## Responsibilities
* Initialize `tracing_subscriber` with JSON or human-readable formatters.
* Capture runtime diagnostic snapshots and health counters.

## Boundaries & Constraints
* **Allowed Dependencies:** `tracing`, `tracing-subscriber`, `serde`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* `TelemetrySubscriber`, `DiagnosticSnapshot`.

## Invariants Protected
* Observability first (ADR-019).

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-observability`

## Maintainer
See `CODEOWNERS`.
