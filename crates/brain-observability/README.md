# brain-observability

## Purpose
Tracing, metrics, profiling spans, and telemetry exporters.

## Responsibilities
* Setup global tracing subscribers, span metrics, and log filtering.
* Profile request/response latency and memory footprints.
* Coordinate system diagnostics and telemetry reporting.

## Dependencies
* **Allowed:** `tracing` and shared utility primitives.
* **Forbidden:** `brain-storage`, `brain-tui`, `brain-session`, `brain-services` (observability must remain a cross-cutting utility).

## Public Interfaces
* Global tracing and logging initialization controllers.

## Owner
SRE & Infrastructure Team
