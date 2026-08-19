# brain-arch-tests

## Purpose
AST-based architectural rule validation and static analysis test suite.

## Responsibilities
* Enforce AST-level import constraints across all crates in the workspace.
* Assert no upward infrastructure imports from core and domain layers.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `syn`, `quote`, `walkdir`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`.

## Public API & Facades
* Architecture test assertions.

## Invariants Protected
* Static structural boundary enforcement.

## Canonical References
* Specification: `../../docs/architecture/CONSTITUTION.md`

## Testing & Verification
* `cargo test -p brain-arch-tests`

## Maintainer
See `CODEOWNERS`.
