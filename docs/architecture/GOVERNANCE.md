---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Architectural Governance Policy

> **Status**: Active Policy  
> **Authority**: System Evolution Governance

---

## 1. Abstraction Layers & Change Cadence

The architecture separates concerns into four distinct document layers with explicit change policies:

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                       1. Architecture Constitution                      │
│      (Normative Rules: MUST / MUST NOT | 3 Axioms | 11 Invariants)     │
│                 [Change Cadence: Very Rare (Identity Change)]           │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       2. Reference Specification                        │
│   (Stable Interfaces: KnowledgeCompiler, IRs, Repositories | SHOULD)    │
│            [Change Cadence: Occasional (Versioned Spec Revisions)]      │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                                 3. ADRs                                 │
│        (Decision Records: Rationale, Benchmarks, Trade-offs, History)   │
│                 [Change Cadence: Continuous (Append-Only Log)]          │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       4. Current Implementation                         │
│    (Code Mechanics: SQLite, SQLCipher, Pass Execution, Caches, Indexes) │
│                [Change Cadence: Continuous (CI-Verified Code)]          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Change Procedures

### A. Amending the Constitution
* **Trigger**: A fundamental change in system identity (e.g. transitioning away from local-first authority).
* **Required Artifacts**: RFC proposal, ADR detailing identity shift, migration impact assessment.
* **Approval Gate**: Requires formal architectural review and explicit maintainer consensus.

### B. Updating the Specification
* **Trigger**: Evolving interface contracts, introducing new primitives, or adjusting IR boundaries.
* **Required Artifacts**: ADR presenting benchmark evidence or tradeoff analysis, versioned specification update.
* **Approval Gate**: Requires passing architectural fitness tests and compatibility checks.

### C. Creating an ADR
* **Trigger**: Resolving implementation options (e.g. reflection scheduling, snapshot persistence models, conflict resolution algorithms).
* **Required Artifacts**: Markdown record in `docs/architecture/adr/ADR-xxx.md` covering Context, Decision, Consequences, and Empirical Evidence.

---

## 3. Operational Review Framework

Every pull request and design proposal must be evaluated against three distinct review disciplines:

* **Code Review**: *Is the implementation correct, maintainable, secure, and performant?*
* **Architecture Review**: *Does this preserve the Constitution, Specification, and the 11 Runtime Invariants?*
* **ADR Review**: *Is there sufficient empirical evidence and benchmark rationale for this design decision?*

---

## 4. Phase Exit Criteria

Transition between engineering roadmap phases requires satisfying objective exit criteria:

* **Phase 0 Exit**: `CONSTITUTION.md`, `GOVERNANCE.md`, `GLOSSARY.md`, and `SPECIFICATION.md` exist and render cleanly with zero duplicate concept definitions.
* **Phase 1 Exit**: `crates/brain-fitness-tests` is integrated into CI; `cargo test -p brain-fitness-tests` deterministically fails builds on layer/boundary violations.
* **Phase 2 Exit**: Runtime contracts compile independently; `KnowledgeCompiler`, `CompilerResult`, `GraphDelta`, and Repositories form a stable public API.
* **Phase 3 Exit**: A single observation traverses the complete compiler pipeline from `MutationRequest` to repository commit deterministically.
* **Phase 4+ Exit**: New capabilities are introduced incrementally, each backed by an ADR and validated via automated fitness checks and benchmarks.
