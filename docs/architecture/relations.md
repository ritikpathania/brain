---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Relationship Semantics and Declarative Taxonomy Specification

This document defines the canonical specification for graph relationship ontology, registry validation, and edge creation rules within the Brain relational memory engine.

---

## 1. Purpose
Relations classify structural connections (edges) between knowledge nodes. Defining relation semantics in a centralized data model prevents different ingestion heuristics, query traversal, or visualization layers from inventing inconsistent or conflicting relation identifiers.

## 2. Relation Ontology
Standard relations supported by the relational memory engine:
* **`uses`**: A component consumes or leverages a library, resource, or service. (Directed, transitive).
* **`depends_on`**: A strict dependency boundary between systems or modules. (Directed, transitive).
* **`runs_on`**: Execution environment or runtime coupling (e.g. OS, container). (Directed, non-transitive).
* **`develops`**: Developer ownership, authoring, or implementation relations. (Directed, non-transitive).
* **`stored_in`**: Physical/logical storage location (e.g. directory, database). (Directed, non-transitive).
* **`configures`**: Configuration bindings mapping parameters between entities. (Directed, non-transitive).
* **`communicates_via`**: Network protocols or socket-level channels. (Directed, non-transitive).
* **`associated_with`**: A generic, undirected symmetric association between two nodes.

## 3. Registry Authority
The declarative Relation Registry (defined in `protocol/relations.json`) is the absolute source of truth for relation semantics. 
* Consumers (such as the TUI, python extractors, or analytics pipelines) may cache registry records for performance.
* No consumer is allowed to redefine, ignore, or interpret relation characteristics (e.g. symmetry or direction) independently.

## 4. Typed Edge Creation
All code creating graph edges must go through a registry-aware or compile-time typed API (e.g., `GraphBuilder::add_edge(source, RelationKind, target)`) rather than constructing relation identifier strings directly. This prevents unauthorized relation strings from bypassing ontology checks.

## 5. Validation Invariants
The registry constructor (`RelationRegistry::new`) enforces strict ontological invariants at runtime and fails fast on violation:
1. **Uniqueness**: Every relation ID must be unique and non-empty.
2. **Display Clarity**: Every relation must declare a non-empty display name.
3. **Inverse Integrity**: If relation `A` specifies an `inverse: Some(B)`, then relation `B` must exist.
4. **Inverse Symmetry**: If `A.inverse == B`, then `B.inverse == A` must hold.
5. **Symmetric Restriction**: A symmetric relation (where `symmetry` is true) can only declare itself as its inverse (`A.inverse == A`).
6. **Undirected Symmetry**: Undirected relations must be symmetric.

## 6. Serialization Invariants
* **`RelationId` Newtype**: Wrap relation string keys in a strongly-typed `RelationId` wrapper. 
* **Borrow Optimization**: `RelationId` implements `Borrow<str>`, `AsRef<str>`, and `Deref<Target = str>` so registry HashMap lookups using `&str` perform zero dynamic memory allocation.
* **Non-Defaultable**: `RelationId` does not implement `Default` to prevent the creation of invalid empty placeholders.
* **Representation Policy**: Domain objects may only expose strongly typed identifiers. Primitive string representations exist exclusively at serialization boundaries (IPC, JSON, SQLite, CLI parsing, etc.).


## 7. Migration & Versioning Policy
Treat the registry data file and relation enum definitions with the same version constraints as a database schema:
* **v1.x**: Backwards compatibility for legacy flat relation names is preserved.
* **v2.0**: Deprecate flat/legacy string heuristics.
* **v3.0**: Enforce strict `RelationId` checks at parser socket boundary.
* Every ontological change must be accompanied by compatibility analysis and version bump decisions.

## 8. Examples

### Registry Entry (`relations.json`)
```json
{
  "id": "associated_with",
  "display_name": "associated with",
  "inverse": "associated_with",
  "directionality": "undirected",
  "symmetry": true,
  "transitivity": false,
  "fallback_suppression": false,
  "confidence_strategy": "average",
  "description": "Symmetric undirected association."
}
```

## 9. Extension Policy
* To add or modify relations, contributors must add the variant to `RelationKind` and `RelationKind::ALL` in Rust, and add the matching entry to `protocol/relations.json`.
* Bidirectional completeness test suites will automatically fail if the code and declarative registry drift.

## 10. Related Specifications & Reference Documents
* [GRAPH_SPEC.md](GRAPH_SPEC.md): Graph model specifications.
* [STABILITY.md](STABILITY.md): System stability invariants.

## 11. Registry Consultation Invariant
The registry is consulted at architectural boundaries to produce validated semantic objects. Internal algorithms operate on those semantic objects rather than repeatedly consulting the registry.
* **Boundary Validation**: `GraphBuilder` queries the registry exactly once when validation is performed during edge additions.
* **Policy Isolation**: Traversal algorithms resolve a `TraversalPolicy` at the start of traversal steps rather than inspecting raw database/registry schemas inside inner loops.
* **Decoupled Components**: The storage layer is schema-agnostic and never queries the registry; serialization acts as a pure boundary.

## 12. Inference & Construction Invariant Guarantees
To maintain structural and semantic purity, the system enforces the following mathematical invariants:
* **Pure Graph Construction**: Graph construction is a pure function of canonicalized input and registry configuration. Given the same normalized inputs, the same alias mappings, and the same registry, the graph builder must always produce an identical graph, regardless of ingestion order or execution environment.
* **Edge Identity Stability**: Existing edge identifiers must never change during inference or suppression. `GraphBuilder` creates original edges, inference adds new edges, and suppression removes edges. No stage modifies or updates edge identifiers in place, preserving stable references for caching, downstream consumers, and explainability.
* **Rule Determinism**: Given the same canonical graph and the same relation registry, `InferenceEngine` must always derive the same set of inferred edges, independent of iteration order.
* **Explanation / Derivation Determinism**: Given the same canonical graph, registry, and inference rule set, every inferred edge must be produced by the same rule with the same supporting edge set. This covers why an edge exists, not just that it exists.
* **Explanation Completeness**: Every edge with `ProvenanceSource::Inferred` must have a valid derivation record, and every supporting edge referenced in that record must exist in the graph. That ensures the graph is never left in a state where an inferred edge cannot be explained.
* **Explanation Acyclicity**: The derivation graph itself must be acyclic. This ensures inference never produces reasoning cycles that require runtime cycle breaking to interpret.
* **Projection Completeness**: Every node and edge present in the validated graph must appear exactly once in the analytical projection tables. This protects the synchronization boundary from silently dropping graph records.
* **Monotonicity Contract**: 
  * `GraphBuilder` constructs.
  * `InferenceEngine` only adds edges (strictly monotonic: $G_{inferred} \ge G_{canonical}$). It never deletes or mutates existing edges.
  * `SuppressionEngine` only removes edges (filtering generic fallback relationships).
* **Provenance Monotonicity**: The `InferenceEngine` may *only* emit edges with `ProvenanceSource::Inferred`. It must never copy, carry over, or alter the provenance of source edges. `SuppressionEngine` never rewrites provenance; it simply filters the resulting graph.
* **Analytics Determinism**: Given the same validated graph, every analytical computation must produce identical results regardless of traversal order or internal hash iteration.



