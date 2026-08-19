# brain-config

## Purpose
Hierarchical configuration loading, environment overrides, and schema validation.

## Responsibilities
* Load and validate configuration from TOML files and environment variables (`BRAIN_*`).
* Provide immutable configuration snapshots for runtime execution.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `serde`, `toml`.
* **Forbidden Dependencies:** `brain-storage`, `brain-services`, `brain-tui`, `pyo3`.

## Public API & Facades
* `BrainConfig`, `StorageConfig`, `UdsConfig`, `TuiConfig`.

## Invariants Protected
* Deterministic immutable configuration snapshotting (ADR-011).

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-config`

## Maintainer
See `CODEOWNERS`.
