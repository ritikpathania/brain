---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
subsystem: compiler
owns:
  - crates/brain-domain
  - crates/brain-services
depends_on:
  - domain
used_by:
  - retrieval
  - daemon
canonical_specs:
  - docs/subsystems/compiler.md
  - ARCHITECTURE.md
adrs:
  - ADR-001
rfcs:
  - RFC-012
---

# Knowledge Compiler Subsystem Mini-Handbook & Specification

> **Governance Role**: This document is both a **Subsystem Handbook** and the **Canonical Specification** (`canonical: true`) for Knowledge Compiler reconciliation passes and rewrite operations.

---

## 1. Purpose
The Knowledge Compiler is the central state-mutation authority for Brain. Canonical graph updates follow the compiler and reflection command boundaries in [Architecture](../../ARCHITECTURE.md#runtime-invariants).

## 2. Responsibilities
- Receives raw facts, observations, and entity updates.
- Executes multi-pass deterministic reconciliation (`EntityNormalization`, `AliasResolution`, `DuplicateDetection`, `ContradictionDetection`, `StaleKnowledgePass`, `ConfidenceRecalculation`).
- Emits atomic `CompilerPlan` rewrite operations (`MergeFacts`, `SupersedeFact`, `ArchiveFact`, `StrengthenEdge`).

## 3. Out of Scope
- Direct SQLite disk I/O or connection handling (delegated to **Storage**).
- User query execution or candidate score fusion (owned by **Retrieval**).
- Frontend rendering and presentation state (owned by the first-party frontend).

## 4. Architecture Overview
```text
  Raw Observations / Facts
             │
             ▼
 ┌───────────────────────┐
 │  EntityNormalization  │ ──► Canonicalizes display names & alias maps
 └───────────┬───────────┘
             ▼
 ┌───────────────────────┐
 │   AliasResolution     │ ──► Resolves entity identifiers to UUIDs
 └───────────┬───────────┘
             ▼
 ┌───────────────────────┐
 │  DuplicateDetection   │ ──► Merges duplicate fact versions & observations
 └───────────┬───────────┘
             ▼
 ┌───────────────────────┐
 │ContradictionDetection │ ──► Supersedes outdated temporal fact versions
 └───────────┬───────────┘
             ▼
 ┌───────────────────────┐
 │  StaleKnowledgePass   │ ──► Archives expired facts based on decay rules
 └───────────┬───────────┘
             ▼
 ┌───────────────────────┐
 │ConfidenceRecalculation│ ──► Recomputes edge weight & belief metrics
 └───────────┬───────────┘
             ▼
    Compiled Knowledge Graph Plan & Mutation Execution
```

## 5. Runtime Flow
1. **Ingest Phase**: Ingestion payloads produce raw `FactVersion` and `Observation` candidates.
2. **Reconciliation Phase**: The compiler evaluates passes sequentially against an in-memory `KnowledgeSnapshotView`.
3. **Execution Phase**: The resulting `CompilerPlan` is committed atomically by the service layer.

## 6. Key Invariants
- **Idempotency**: Compilation is strictly idempotent ($P(P(G)) = P(G)$).
- **Provenance Monotonicity**: No pass may delete observation history; passes may only add, merge, or reclassify links.
- **Side-Effect Isolation**: Pass evaluation does not trigger network, disk, or external side-effects.

## 7. Owning Crates
- [`crates/brain-domain`](../../crates/brain-domain/README.md): Compiler traits, pass types, rewrite operations.
- [`crates/brain-services`](../../crates/brain-services/README.md): Pass implementations (`duplicate_consolidation`, `contradiction`, etc.).

## 8. Implementation Notes
- Passes register with `PassRegistry` and resolve topological execution ordering at runtime.

## 9. Canonical References
- [Architecture invariants](../../ARCHITECTURE.md#runtime-invariants).
- [`ADR-001`](../architecture/adr/ADR-001-knowledge-runtime-evolution-model.md): Pure domain compiler evaluation invariants.

## 10. Related RFCs
- [`RFC-012: Reflection Engine`](../architecture/rfc/RFC-012-reflection-engine.md)

## 11. Operations
- Background compilation runs automatically during idle reflection passes or post-ingestion triggers.

## 12. Testing
- Compiler and reflection tests in `crates/brain-services/tests/` cover graph mutation and pass idempotency. The [test matrix](../../TEST_INFRA.md) identifies the separate dependency checks.

## 13. Extension Points
- Implement `CompilerPass` trait to register new domain reconciliation rules.

## 14. Subsystem Dependencies
```text
Knowledge Compiler Subsystem
├── Depends on: Domain (brain-domain)
├── Mutates: Storage (brain-storage)
├── Feeds: Retrieval Engine (brain-services)
├── Triggered by: IPC Ingest (daemon)
└── Monitored by: Observability (brain-observability)
```
