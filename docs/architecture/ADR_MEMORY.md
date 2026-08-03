# ADR-007: Memory Architecture & `/memory` Command Semantics

**Status**: Proposed  
**Date**: 2026-08-02  
**Authors**: Antigravity & Engineering Team  
**Replaces / Extends**: Extends RFC-006 (Knowledge Inspection)  

---

## 1. Purpose

The Memory Architecture introduces persistent, consolidated relational memory capabilities into the Brain system. While `/search` executes ad-hoc exploratory queries across the entire knowledge graph, `/memory` enables inspecting, pinning, consolidating, and managing curated remembered facts, session context, and long-term knowledge observations.

---

## 2. Memory Types

1. **Short-Term Memory (STM)**:
   - Ephemeral in-session observations, prompt inputs, and active typewriter state.
   - Volatile lifetime bound to the current conversation session.
2. **Long-Term Memory (LTM)**:
   - Consolidated entity facts, relation links, and historical observations.
   - Persisted in the relational memory engine.
3. **BKF-Backed Memories (Bounded Knowledge Forest)**:
   - Structured subgraphs representing consolidated domain concepts and retrieval indexes.
4. **Runtime Context**:
   - Explicitly pinned nodes (`workspace_context`) attached to prompt executions.

---

## 3. Lifecycle & Domain Model (`MemoryState`)

Memory lifecycle transitions are governed by an explicit domain enum rather than implicit booleans or scattered timestamps:

```rust
pub enum MemoryState {
    /// Active memory available for retrieval and context.
    Active,
    /// Explicitly pinned memory locked in runtime context.
    Pinned,
    /// Retained memory moved to cold storage.
    Archived,
    /// Stale or decayed memory past TTL threshold.
    Expired,
}
```

### Lifecycle Flow

```text
Creation (Ingestion) ──► Active ──► Pinned / Archived / Expired
```

---

## 4. Retrieval & Command Semantics (Discovery vs. Stewardship)

- `/search` $\rightarrow$ **Discovery**: Ad-hoc query matching and exploratory graph search.
- `/memory` $\rightarrow$ **Stewardship**: Curating, inspecting, pinning, consolidating, and annotating remembered knowledge.

| Concern | `/search` (Discovery) | `/memory` (Stewardship) |
| :--- | :--- | :--- |
| **Primary Goal** | Ad-hoc discovery & search hit retrieval | Stewardship & curation of remembered knowledge |
| **Scope** | Global Knowledge Graph | Active, pinned, and consolidated memory records |
| **Operations** | Read-only search ranking | List, inspect, pin, consolidate, archive |
| **Output** | Rank-ordered search result items | Memory items & inspection overlays |

---

## 5. Inspection Entry Points

All system subsystems that surface entities converge on the single, unified **Inspection Capability**:

```text
Search
Memory
Context
Graph Traversal ───► Inspection Capability (InspectorViewModel)
Session History
```

Regardless of entry point (search hit, memory item, context node, or graph link), inspection displays through `InspectorViewModel` without duplicating presentation logic.

---

## 6. Architectural Invariant Traceability

This ADR explicitly enforces the canonical invariants defined in [ARCHITECTURAL_INVARIANTS.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/ARCHITECTURAL_INVARIANTS.md):

- **Invariant 1 (DDD Layering)**: `brain-domain` owns memory models (`MemoryId`, `Observation`) with zero UI dependencies.
- **Invariant 4 (Presentation ViewModel Separation)**: Memory inspection views use immutable projections (`MemoryViewModel`).
- **Invariant 6 (One Command = One Deterministic Capability)**: `/memory` maps to a dedicated memory service.
- **Invariant 7 (Capability-Oriented Interfaces)**: Exposes `ExecutionClient::inspect_memory(...)` instead of storage-specific queries.
- **Invariant 8 (Domain Before Presentation)**: Zero UI formatting inside domain memory entities.

---

## 7. Non-Goals

To prevent scope creep, `/memory` explicitly excludes the following:
- `/memory` is **NOT** a replacement for `/search`.
- `/memory` is **NOT** responsible for executing graph traversals.
- `/memory` does **NOT** expose underlying transport or database IPC schemas.
- `/memory` does **NOT** implement custom overlay widgets; it delegates rendering entirely to `InspectorViewModel`.
