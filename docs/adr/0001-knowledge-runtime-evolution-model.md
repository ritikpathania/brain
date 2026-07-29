# ADR-0001: Knowledge Runtime Evolution Model

* **Status:** Accepted
* **Date:** 2026-07-28
* **Deciders:** Core Architecture Team

## Context & Problem Statement

As Brain evolves from a basic relational memory engine into a continuous knowledge graph platform, we require strict runtime invariants to govern how entities, evidence, state transitions, retrieval, and reflection operate. Without explicit guarantees, background jobs or complex retrieval pipelines could silently corrupt canonical entity identity, discard historical provenance, or introduce nondeterministic graph mutations.

## Decision Drivers

* **Purity of Domain:** `brain-domain` must remain completely decoupled from storage engines, async runtimes, and external dependencies.
* **Auditability & Provenance:** Every fact and entity transition must remain traceable to original source observations.
* **Compiler Philosophy:** Reconciliation and reflection tasks must behave like a compiler pipeline: deterministic, idempotent, and testable.
* **Separation of Evidence and Projection:** Raw evidence retrieval must be distinct from optional natural-language synthesis or downstream LLM projections.

## Enforced Runtime Invariants

1. **Immutable Canonical Identity:**
   Every entity possesses a single immutable `EntityId` (UUID/ULID). Preferred display names, labels, and aliases are mutable attributes, but the underlying `EntityId` is never mutated or re-used.

2. **Provenance Monotonicity:**
   Compiler and Reflection passes are strictly monotonic with respect to provenance: no pass may discard provenance or observation history; passes may only add, aggregate, archive, or reclassify them.

3. **First-Class Provenance on All Knowledge:**
   Every node, edge, and fact maintains explicit provenance backed by first-class `Observation` records.

4. **Deterministic & Idempotent Reconciliation:**
   All compiler and reconciliation passes (`EntityNormalization`, `AliasResolution`, `DuplicateDetection`, `ContradictionDetection`, `OrphanDetection`) must be strictly deterministic, idempotent ($P(P(G)) = P(G)$), and side-effect isolated.

5. **Read-Only Retrieval:**
   The retrieval engine (`Retriever` $\rightarrow$ `CandidateSet` $\rightarrow$ `Scorer` $\rightarrow$ `Ranker`) never mutates the underlying knowledge graph or state transitions during query execution.

6. **Controlled Reflection Mutability:**
   The `ReflectionEngine` is the sole subsystem permitted to evolve graph topology or transition knowledge states post-ingestion.

7. **Pure Projection Layer:**
   Downstream projections (including optional LLM synthesis) operate strictly on deterministic `EvidenceSet` snapshots and never alter canonical memory directly.

## Consequences

* **Positive:** Guaranteed auditability, reproducible graph state, zero unexpected mutation side-effects during retrieval, and safe background reflection.
* **Negative:** Requires disciplined data modeling with structured evidence and explicit pass reports rather than ad-hoc inline mutations.
