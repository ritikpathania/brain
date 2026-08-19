# brain-storage

## Purpose
SQLite schema management, Connection Pool, and Private Repository implementations.

## Responsibilities
* Establish SQLite connections, pools, and WAL (Write-Ahead Logging) initialization.
* Manage database schema migrations dynamically at startup.
* Implement private Node, Edge, Embedding, and Session repositories behind core traits.
* Contain all raw SQL execution (sole owner of SQLite persistence in the workspace).

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `rusqlite`, `r2d2`.
* **Forbidden Dependencies:** `brain-services`, `brain-tui`, `brain-python`, `brain-plugins`.

## Public API & Facades
* Connection builders, database migration managers, typed repository facades (`SqliteStore`).

## Invariants Protected
* Sole owner of concrete SQLite persistence (Invariant 1), transactional isolation.

## Canonical References
* Specification: `../../docs/reference/storage.md`

## Testing & Verification
* `cargo test -p brain-storage`

## Maintainer
See `CODEOWNERS`.
