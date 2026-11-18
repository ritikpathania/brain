# brain-fitness-tests

## Purpose
Automated architectural fitness checks and compile-time boundary enforcement.

## Responsibilities
* Validate crate dependency DAG acyclicity and layer hierarchy constraints.
* Assert adapter storage isolation (brain-daemon and brain have zero direct storage dependencies).
* Assert PyO3 encapsulation inside brain-python.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `cargo_metadata`, `syn`, `walkdir`.
* **Forbidden Dependencies:** `brain-tui`, `pyo3`, `rusqlite`.

## Public API & Facades
* CI fitness test suite.

## Invariants Protected
* Checks pure domain dependencies, storage isolation at host boundaries, PyO3 ownership and toolchain pins.

## Canonical References
* Specification: [Test matrix](../../TEST_INFRA.md)

## Testing & Verification
* `cargo test -p brain-fitness-tests`

## Maintainer
See `CODEOWNERS`.
