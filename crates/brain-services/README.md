# brain-services

## Purpose
Business logic services (embedding, retrieval, session).

## Responsibilities
* Implement the Reciprocal Rank Fusion (RRF) hybrid search query pipeline (lexical + vector + graph expansion).
* Coordinate decay sweeps and memory graph consolidation.
* Orchestrate command executions and coordinate active sessions.

## Dependencies
* **Allowed:** `brain-domain`, `brain-core`, `brain-events`, `brain-config`.
* **Forbidden:** `brain-tui`, `brain-python` (services layer is decoupled from raw PyO3 bindings and UI rendering loop).

## Public Interfaces
* Services: `RetrievalService`, `SessionService`, `ConsolidationService`

## Owner
Core Development Team
