---
status: active
owner: storage
canonical: false
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
subsystem: storage
owns:
  - crates/brain-storage
depends_on:
  - domain
used_by:
  - compiler
  - retrieval
canonical_specs:
  - docs/reference/storage.md
adrs:
  - ADR-024
rfcs:
  - RFC-001
---

# Storage Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical schema DDL lives in [`docs/reference/storage.md`](../reference/storage.md).

---

## 1. Purpose
The Storage subsystem manages persistent disk storage, SQLite database migrations, raw BLOB vector embedding caches, relational graph indices, and read projection synchronization for the Brain runtime.

## 2. Responsibilities
- Manages connection pools and WAL mode transaction boundaries for `~/.brain/brain.db`.
- Executes versioned DDL schema migrations on startup.
- Persists entity nodes, relationship edges, observation records, and fact version timelines.
- Maintains the `search_projection` FTS5 virtual table and vector embedding BLOB cache.

## 3. Out of Scope
- Knowledge reconciliation passes, entity deduplication, or contradiction resolution (owned by **Compiler**).
- Hybrid BM25/Vector RRF candidate scoring and ranking algorithms (owned by **Retrieval**).
- Terminal visual rendering or UI state management (owned by **TUI**).

## 4. Architecture Overview
```text
┌─────────────────────────────────────────────────────────────────────────┐
│                       Brain Application Context                         │
├────────────────────────────────────┬────────────────────────────────────┤
│       SQLite Relational Engine     │        Search Projection Sync       │
│  - nodes & edges tables            │  - FTS5 Virtual Table              │
│  - fact_versions & observations    │  - BLOB Vector Embedding Cache     │
├────────────────────────────────────┴────────────────────────────────────┤
│                       ~/.brain/brain.db (WAL Mode)                      │
└─────────────────────────────────────────────────────────────────────────┘
```

## 5. Runtime Flow
1. **Initialization**: `brain-storage` opens `~/.brain/brain.db` and runs pending migrations.
2. **Transaction Scoping**: Application services issue queries wrapped in explicit read or write transactions.
3. **Projection Sync**: Entity and edge mutations trigger synchronous `search_projection` FTS5 updates.

## 6. Key Invariants
- **Immutable Canonical Identity**: `EntityId` UUIDs are immutable and never re-used.
- **Monotonic Provenance**: Observations and fact versions are append-only.
- **WAL Thread Safety**: Multiple readers operate concurrently without blocking the single writer.

## 7. Owning Crates
- [`crates/brain-storage`](../../crates/brain-storage/README.md): Connection pool, DDL migrations, transaction handles.

## 8. Implementation Notes
- Uses SQLite WAL mode with `synchronous = NORMAL` for optimal throughput.
- Vector BLOBs store raw single-precision float arrays (`f32`) for SIMD dot-product operations.

## 9. Canonical References
- [`docs/reference/storage.md`](../reference/storage.md): Canonical SQLite DDL schema reference.
- [`docs/architecture/GRAPH_SPEC.md`](../architecture/GRAPH_SPEC.md): Graph invariants and observation monotonicity.
- [`docs/architecture/relations.md`](../architecture/relations.md): Relationship taxonomy and edge decay.

## 10. Related ADRs
- [`ADR-024: IVF Vector Indexing`](../architecture/adr/ADR-024-ivf-vector-indexing.md)

## 11. Related RFCs
- [`RFC-001: Storage Layer Transactional Memory`](../architecture/rfc/RFC-001.md)

## 12. Operations
- **Data Location**: `~/.brain/brain.db` (Database) and `~/.brain/brain.db-wal` (Write-Ahead Log).
- **Maintenance Guide**: See [`docs/guides/maintenance.md`](../guides/maintenance.md).

## 13. Testing
- Integration tests in `crates/brain-storage/tests/` verify schema migrations, FTS5 sync, and transaction rollbacks.

## 14. Extension Points
- Implement custom query projections via `brain-storage::projections` traits.

## 15. Subsystem Dependencies
```text
Storage Subsystem
├── Depends on: Domain (brain-domain)
├── Used by: Knowledge Compiler (brain-services)
├── Queried by: Retrieval Engine (brain-services)
├── Exposed through: IPC Protocol (daemon)
└── Visualized by: TUI (brain-tui)
```
