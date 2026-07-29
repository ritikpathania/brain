# Brain Architectural Glossary

> **Status**: Single Source of Truth  
> **Authority**: Canonical Primitives Dictionary

This document provides the authoritative definition for all **12 Core Architectural Primitives** in Brain. No other document should re-define these terms; all other architectural documentation references this canonical glossary.

---

## The 12 Core Architectural Primitives

### 1. `MutationRequest`
The universal input payload envelope representing any proposed state transition (e.g., raw observation ingestion, reflection findings, snapshot restoration, workspace import, or synchronization updates).

### 2. `KnowledgeCompiler`
The 3-tier deterministic transformation engine executing Front End (parsing, validation, normalization), Middle End (canonicalization, conflict resolution, reflection, optimization), and Back End (provenance, persistence, event emission) passes.

### 3. `CompilerResult`
The decoupled output product of compilation containing a `GraphDelta`, emitted `RuntimeEvent`s, and `Diagnostic` records.

### 4. `Observation IR`
The intermediate representation of normalized, validated raw observations prior to semantic entity/relation extraction.

### 5. `Knowledge IR`
The intermediate representation of candidate semantic graph nodes, edges, and relationship mutations prior to final conflict resolution and persistence.

### 6. `Canonical Graph`
The authoritative semantic graph storing canonical entities, relations, and provenance. The Canonical Graph is mutable *only* through `KnowledgeCompiler`; outside the compiler, it is observable as an immutable value.

### 7. `ReadProjection`
A pure, side-effect-free, read-only view of the canonical graph (`&self`, `&CanonicalGraph`). Projections can never acquire write handles or mutate runtime state.

### 8. `Analysis`
Side-effect-free computation evaluated over canonical graph state (e.g. HealthScore calculation, graph density analysis, reflection rule evaluation). Analysis never mutates state directly; it may emit `MutationRequest` findings.

### 9. `Capability`
A pluggable runtime behavior trait registered in `CapabilityRegistry` (e.g. `ProjectionCapability`, `StorageCapability`, `SnapshotCapability`).

### 10. `Repository`
An abstract storage interface (e.g. `KnowledgeRepository`, `SecretRepository`, `BlobRepository`) isolating concrete persistence infrastructure (SQLite, SQLCipher, filesystem) from runtime and domain logic.

### 11. `Policy`
A deterministic strategy object defining rules and invariants (e.g. `AuthorizationPolicy`, `ConflictResolutionPolicy`, `RetentionPolicy`).

### 12. `Context`
An immutable value object encapsulating runtime, workspace, pass, or request scope (`RuntimeContext`, `WorkspaceContext`, `CompilerContext`, `AuthorizationContext`).
