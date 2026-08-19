# brain-domain

## Purpose
Pure Domain-Driven Design (DDD) domain models, value objects, and pure domain events.

## Responsibilities
* Encapsulate domain entities (Node, Edge, Memory, Session, Observation) with internal validation.
* Protect state invariants and emit pure, side-effect-free DomainEvents on mutation.
* Implement in-memory domain evaluation specifications.

## Boundaries & Constraints
* **Allowed Dependencies:** `serde`, `uuid`, `ulid`, `chrono`.
* **Forbidden Dependencies:** `tokio`, `rusqlite`, `brain-storage`, `brain-services`, `brain-tui`, `pyo3`.

## Public API & Facades
* Entities (`Node`, `Edge`, `Observation`), Events (`DomainEvent`), Specs (`Specification`).

## Invariants Protected
* Zero external infrastructure dependencies, pure in-memory business logic.

## Canonical References
* Specification: `../../docs/architecture/CONSTITUTION.md`

## Testing & Verification
* `cargo test -p brain-domain`

## Maintainer
See `CODEOWNERS`.
