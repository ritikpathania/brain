# Architecture Fitness Tests Specification

> **Status**: Active CI Integration  
> **Authority**: Invariant Enforcement Strategy

---

## 1. Overview

Architecture Fitness Tests automatically enforce the 11 Constitutional Invariants ([CONSTITUTION.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/CONSTITUTION.md)) inside CI. Instead of relying on manual code review, fitness checks prevent boundary erosion, layer violations, and accidental mutation pathways.

---

## 2. Invariant Coverage Status

| Invariant | Status | Automated Fitness Check | Enforcement Strategy |
| :--- | :--- | :--- | :--- |
| **1. Single Mutation Entry** | **Enforced** | `test_single_mutation_entry` | Scans workspace dependencies & source files asserting no crate outside `KnowledgeCompiler` imports write repositories. |
| **2. Compile-Time Read Projections** | **Enforced** | `test_projection_purity` | Source analysis asserting `ReadProjection` trait implementations accept only immutable `&self` and `&CanonicalGraph` parameters. |
| **3. Reflection as Analysis** | **Enforced** | `test_reflection_analysis_boundary` | Verifies reflection passes return `MutationRequest` findings without importing mutable storage engines. |
| **4. Adapter Storage Isolation** | **Enforced** | `test_adapter_storage_isolation` | `cargo_metadata` dependency DAG check asserting adapter crates (`daemon_bridge`, `apps/brain`) have zero direct dependencies on `brain-storage`. |
| **5. Orchestration Only Runtime** | **Partially Enforced** | `test_runtime_orchestration_only` | Dependency check verifying `BrainRuntime` references trait capabilities rather than concrete infrastructure structs. |
| **6. Deterministic Compiler Passes** | **Planned** | `test_deterministic_pass_replay` | Replay test verifying matching input logs yield identical `CompilerResult` byte hashes. |
| **7. Controlled Graph Mutability** | **Enforced** | `test_graph_mutability_control` | Asserts `CanonicalGraph` write handles are unexposed outside the `KnowledgeCompiler` module. |
| **8. Strongly-Typed Provenance** | **Partially Enforced** | `test_provenance_types` | Validates domain entities derive strongly-typed `Provenance` fields. |
| **9. Idempotent Mutation Processing** | **Planned** | `test_idempotent_mutation_processing` | Asserts duplicate `MutationRequest` execution returns identical delta or rejection. |
| **10. Primitive Composition** | **Enforced** | `test_primitive_composition` | Public API contract check asserting public exports match the 12 primitive types in [GLOSSARY.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/GLOSSARY.md). |
| **11. Primitive Evolution** | **Enforced** | `test_primitive_evolution` | Fails CI if unvetted public primitives are introduced without an ADR. |

---

## 3. Allowed Exceptions Mechanism

In rare cases, intentional architectural exceptions are permitted via `crates/brain-fitness-tests/allowlist.toml`. Exceptions MUST be explicitly recorded with a rule name, crate target, justification, and expiration date:

```toml
# crates/brain-fitness-tests/allowlist.toml
[[allow]]
rule = "adapter_storage_isolation"
crate = "legacy_migration_tool"
reason = "One-time migration utility requiring legacy direct DB access"
expires = "2027-01-01"
```
