# brain-cli-adapter

## Purpose
CLI argument formatting, tabular output serializers, and terminal presentation helpers.

## Responsibilities
* Format query hits, graph inspections, and status outputs for standard stdout/stderr.
* Parse user input flags and options.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`.
* **Forbidden Dependencies:** `brain-storage`, `pyo3`.

## Public API & Facades
* CLI presentation formatters.

## Invariants Protected
* Presentation decoupled from business logic.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-cli-adapter`

## Maintainer
See `CODEOWNERS`.
