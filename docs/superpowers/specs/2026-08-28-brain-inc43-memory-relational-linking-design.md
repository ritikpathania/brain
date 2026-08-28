# Increment 43 — Knowledge Graph Relational Authoring (`/memory link` & `/memory unlink`) Design Specification

## Overview

Increment 43 closes the Knowledge Graph explicit relational authoring seam:
1. **Backend Engine / Daemon RPCs**:
   - `v1/memory/link`: Validates source and target nodes exist, constructs a canonical `Edge` with `RelationKind` (default: `related_to`), and persists it via `EdgeRepository`.
   - `v1/memory/unlink`: Deletes the composite edge `EdgeId(source, target, relation)` via `EdgeRepository`.
2. **TypeScript SDK (`BrainBackendClient.ts`)**:
   - Exposes `linkMemory(...)` and `unlinkMemory(...)`.
3. **State Layer (`SessionController.ts`)**:
   - Implements `linkMemories(sourceQuery, targetQuery, relation?)` and `unlinkMemories(sourceQuery, targetQuery, relation?)`.
   - Resolves node query prefixes and labels deterministically with unambiguous resolution.
   - Emits structured confirmation notices without optimistic/local state mutation.
4. **Command Surface (`builtin.ts` & `AppShell.tsx`)**:
   - `/memory link <source> <target> [relation]`
   - `/memory unlink <source> <target> [relation]`
   - Top-level aliases `/link` and `/unlink`.

```text
/memory link <source> <target> [relation]
                │
                ▼
SessionController.linkMemories()
   ├── Resolves source & target nodes via authoritative search
   └── Dispatches RPC
                │
                ▼
BrainBackendClient.linkMemory()
                │
                ▼
v1/memory/link [daemon/src/transport/uds/handlers.rs]
   ├── Validates source and target exist via NodeRepository
   ├── Constructs canonical Edge(source, target, relation, weight)
   └── Saves edge via EdgeRepository::save(&edge)
                │
                ▼
authoritative response -> SessionController confirmation notice
                │
                ▼
live inspection via MemoryOverlayView (/memory)
```

---

## Architectural Invariants & Guarantees

1. **Authoritative Storage Persistence**:
   - All relationship mutations pass through `EdgeRepository` in SQLite.
   - No local/shell-side graph state fabrication.
2. **Deterministic Resolution**:
   - Node queries match exact ID prefixes or unique labels. Ambiguous or missing targets abort cleanly with error notices without dispatching mutations.
3. **Canonical Relation Kinds**:
   - Uses domain `RelationKind` (default: `related_to`, with normalization).
4. **Failure Atomicity**:
   - RPC rejections or validation failures leave controller and storage state unchanged.

---

## Verification Matrix

1. **TypeScript Unit Tests (`memoryLinkLogic.test.ts`)**:
   - Link with node IDs and node labels.
   - Unresolved and ambiguous query handling.
   - Default relation `related_to` vs custom relations (`depends_on`, `implements`).
   - Unlink operations and error handling.
   - Command parsing for `/memory link`, `/memory unlink`, `/link`, `/unlink`.
2. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc43.py`)**:
   - Ingest Concept A (`AuthService`) and Concept B (`Database`).
   - Run `/memory link AuthService Database depends_on`.
   - Open `/memory` -> verify `AuthService` expands with relation `depends_on -> Database`.
   - Run `/memory unlink AuthService Database depends_on`.
   - Open `/memory` -> verify relation is removed.
3. **Full 20-Suite Regression Matrix**:
   - All 20 canonical PTY suites green.
   - Full 414+ Bun test suite green.
   - 74 Rust domain/storage tests green.
   - Clean detached worktree `cargo check --workspace`.
   - 3,938 WIP paths preserved byte-for-byte.
