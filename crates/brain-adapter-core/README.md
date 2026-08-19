# brain-adapter-core

## Purpose
Shared adapter traits, common protocol codecs, and generic capability registries.

## Responsibilities
* Provide shared traits for external agent adapters (MCP, ACP, A2A).
* Implement common request/response envelope transformation helpers.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* Shared adapter traits and registries.

## Invariants Protected
* Shared adapter infrastructure (ADR-023).

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-adapter-core`

## Maintainer
See `CODEOWNERS`.
