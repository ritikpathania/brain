# BenchmarkResult Schema Changelog

All notable changes to the `BenchmarkResult` machine-readable JSON Schema contract are documented in this file.

The schema uses Semantic Versioning (`MAJOR.MINOR.PATCH`). Schema files under `schemas/benchmark-result/<version>/` are append-only and strictly immutable.

---

## [1.0.0] — 2026-08-06

### Initial Release
- **Machine-Readable JSON Schema**: Draft 2020-12 compliant schema file at `schemas/benchmark-result/1.0.0/benchmark-result.schema.json`.
- **Strict Validation**: Enforces `additionalProperties: false` across all definitions to prevent field drift.
- **Stable Capability Objects**: Decouples capability `id` (e.g. `alias_normalization`) and `display_name` (`Alias Normalization`).
- **Separation of Statuses**: Distinguishes `execution_status` (`COMPLETED`, `TIMED_OUT`, `CANCELLED`, `CRASHED`) from `quality_status` (`PASS`, `QUALITY_WARNING`, `BLOCKED`).
- **Reusable `$defs`**: Defines `Metadata`, `CapabilityIdentifier`, `BenchmarkInfo`, `Measurements`, `Confidence`, `Latency`, `Provenance`, `Diagnostics`.
- **Contract & Compatibility Fixtures**: Valid/invalid regression test fixtures under `tests/` and multi-version historical compatibility fixtures under `compatibility/v1.0.0/`.
