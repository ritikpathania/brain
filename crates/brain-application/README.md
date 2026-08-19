# brain-application

## Purpose
Runtime composition facade and ApplicationRuntime lifecycle builder.

## Responsibilities
* Provide `RuntimeBuilder` and `ApplicationRuntime` for subsystem composition.
* Coordinate graceful startup, background worker spawning, and shutdown sequencing.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `brain-services`, `brain-storage`, `brain-events`, `brain-config`.
* **Forbidden Dependencies:** `brain-tui`, `pyo3`.

## Public API & Facades
* `ApplicationRuntime`, `RuntimeBuilder`, `RuntimeHandle`.

## Invariants Protected
* Single runtime composition facade, lifecycle orchestration purity.

## Canonical References
* Specification: `../../docs/architecture/ARCHITECTURE_INVARIANTS.md`

## Testing & Verification
* `cargo test -p brain-application`

## Maintainer
See `CODEOWNERS`.
