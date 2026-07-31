# Phase 5.3.5 — Query Conformance & Replay Equivalence Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`tests/query_conformance_tests.rs`)

---

## 1. Executive Summary & Contract Surface

Phase 5.3.5 establishes the formal **Query Conformance & Replay Equivalence Suite** (`brain-services/tests/query_conformance_tests.rs`). The suite acts as an architectural regression gate, validating that all query evaluators (`NeighborhoodEvaluator`, `SearchEvaluator`, `HybridEvaluator`) satisfy our core architectural contracts regardless of future internal implementation changes.

### Architectural Contracts Validated:

| Contract | Mathematical / Behavioral Invariant | Target Evaluators |
| :--- | :--- | :--- |
| **Replay Equivalence** | `Query(Snapshot_Batch) == Query(Snapshot_Incremental)` (normalized for timing) | All |
| **Duplicate-Free Output** | $\forall i, j \in \text{matches}, i \neq j \implies \text{entity\_id}_i \neq \text{entity\_id}_j$ | All |
| **Deterministic Total Ordering** | Matches strictly sorted by primary key + `KnowledgeEntityId` ASC tie-breaking | All |
| **Pagination Algebra** | $\text{Slice}(0, N + M) = \text{Slice}(0, N) \cup \text{Slice}(N, M)$ with preserved `total_matched` | All |
| **Snapshot Read Immutability** | Repeated calls produce identical outputs with zero side effects or snapshot mutation | All |
| **Cross-Evaluator Isolation** | Executing evaluator $X$ leaves zero shared state or cache mutations for evaluator $Y$ | All |

---

## 2. Specification of Invariants

### 1. Replay Equivalence Contract
When an identical stream of `FactEvent`s is reduced via batch replay into `Snapshot A` and reduced incrementally event-by-event into `Snapshot B`:
$$\text{Normalize}(\text{QueryFacade::query}(S_A)) \equiv \text{Normalize}(\text{QueryFacade::query}(S_B))$$
where `Normalize` zeroes out `metadata.execution_duration_us` before comparison.

### 2. Duplicate-Free Output Contract
Every returned `QueryFacadeResult.matches` vector MUST contain zero duplicate `KnowledgeEntityId` entries across all evaluators and multi-modal query configurations.

### 3. Deterministic Total Ordering Contract
Whenever primary sort fields compare equal between two `EntityMatch` candidates, the tie-breaker `a.entity_id.0.cmp(&b.entity_id.0)` guarantees 100% deterministic total ordering independent of candidate discovery order or storage hash ordering.

### 4. Pagination Algebra Contract
For any candidate set of size $T$:
- $\text{paginate}(\text{offset}=0, \text{limit}=N + M) = ([\text{matches}_{0..N}], T)$
- $\text{paginate}(\text{offset}=0, \text{limit}=N) \cup \text{paginate}(\text{offset}=N, \text{limit}=M) = ([\text{matches}_{0..N+M}], T)$
- $\forall \text{offset} \ge T$, $\text{paginate}(\text{offset}, \text{limit}) = ([], T)$

### 5. Snapshot Read Immutability Contract
Executing queries against a `ProjectionSnapshot` MUST NOT mutate internal state or alter future query results across $N$ consecutive executions.

### 6. Cross-Evaluator Isolation Contract
Executing a `NeighborhoodQuery`, `LexicalSearchQuery`, and `HybridSearchQuery` sequentially against the same `KnowledgeQueryFacade` instance yields identical results compared to executing each query in isolation against a fresh facade instance.

---

## 3. Verification & Testing Strategy

1. **Conformance Test Suite (`crates/brain-services/tests/query_conformance_tests.rs`)**:
   - `test_conformance_replay_equivalence_across_evaluators`: Validates batch replay vs incremental snapshot equivalence for `NeighborhoodEvaluator`, `SearchEvaluator`, and `HybridEvaluator`.
   - `test_conformance_duplicate_free_invariant`: Validates zero duplicate entities returned across multi-modal queries.
   - `test_conformance_deterministic_total_ordering`: Validates primary field sorting and `EntityId` ASC tie-breaking.
   - `test_conformance_pagination_algebra`: Validates mathematical pagination equivalence and `total_matched` preservation.
   - `test_conformance_snapshot_immutability`: Validates repeated query execution immutability.
   - `test_conformance_cross_evaluator_isolation`: Validates zero cross-evaluator side effects.
