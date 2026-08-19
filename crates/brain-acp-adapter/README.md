# brain-acp-adapter

## Purpose
Agent Context Protocol (ACP) adapter for multi-agent contextual coordination.

## Responsibilities
* Implement ACP wire protocol handlers and contextual memory sharing.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `brain-application`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* ACP Server handler.

## Invariants Protected
* Protocol adapter decoupling.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-acp-adapter`

## Maintainer
See `CODEOWNERS`.
