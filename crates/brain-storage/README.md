# brain-storage

## Purpose
SQLite schema management, Connection Pool, and Private Repository implementations.

## Responsibilities
* Establish SQLite connections, pools, and WAL (Write-Ahead Logging) initialization.
* Manage database schema migrations dynamically at startup.
* Implement private Node, Edge, Embedding, and Session repositories behind core traits.
* Contain all raw SQL execution (no SQLite calls are permitted outside this crate).

## Dependencies
* **Allowed:** `brain-domain`, `brain-core`, `rusqlite`.
* **Forbidden:** `brain-services`, `brain-tui`, `brain-python`, `brain-plugins`.

## Public Interfaces
* Crate-level connection builders, database migration managers, and repository initialization triggers.

## Owner
Database Engineer
