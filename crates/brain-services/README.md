# brain-services

## Purpose
Business logic services (embedding, retrieval, session, graph consolidation).

## Responsibilities
* Implement the Reciprocal Rank Fusion (RRF) hybrid search query pipeline (lexical + vector + graph expansion).
* Coordinate decay sweeps, intent logging, and memory graph consolidation.
* Orchestrate command executions and coordinate active sessions.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `brain-events`, `brain-config`, `brain-storage`.
* **Forbidden Dependencies:** `brain-tui`, `brain-python` (services layer is decoupled from raw PyO3 bindings and UI rendering loop).

## Public API & Facades
* Services: `RetrievalService`, `SessionService`, `ConsolidationService`.

## Invariants Protected
* RRF ranking monotonicity, transactional intent logging, reflection analysis purity.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-services`

## Maintainer
See `CODEOWNERS`.
