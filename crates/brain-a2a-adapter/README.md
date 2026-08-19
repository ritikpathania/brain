# brain-a2a-adapter

## Purpose
Agent-to-Agent (A2A) protocol adapter for autonomous peer communication.

## Responsibilities
* Handle A2A peer memory synchronization and consensus messages.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `brain-application`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* A2A Server handler.

## Invariants Protected
* Protocol adapter decoupling.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-a2a-adapter`

## Maintainer
See `CODEOWNERS`.
