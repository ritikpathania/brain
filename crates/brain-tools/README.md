# brain-tools

## Purpose
Built-in autonomous agent tools and command execution providers.

## Responsibilities
* Provide built-in tool implementations (search, file inspection, memory management).
* Validate tool arguments and enforce execution safety policies.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* `BuiltinToolRegistry`, `StandardTools`.

## Invariants Protected
* Tool parameter safety validation.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-tools`

## Maintainer
See `CODEOWNERS`.
