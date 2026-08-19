# brain-core

## Purpose
Trait specifications, Repository interfaces, Agent contracts, and system-wide Error types.

## Responsibilities
* Define Repository interfaces for database abstraction (NodeRepository, EdgeRepository, SessionRepository).
* Specify Agent contracts (chat, extraction, embedding, planning) and plugin lifecycle interfaces.
* Define system-wide error hierarchy enum (`BrainError`).

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`.
* **Forbidden Dependencies:** `brain-storage`, `brain-events`, `brain-services`, `brain-tui`, `brain-python`.

## Public API & Facades
* Repositories (`NodeRepository`, `SessionRepository`), Traits (`Agent`, `Tool`), `BrainError`.

## Invariants Protected
* Interface-only abstraction layer, zero concrete infrastructure dependencies.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-core`

## Maintainer
See `CODEOWNERS`.
