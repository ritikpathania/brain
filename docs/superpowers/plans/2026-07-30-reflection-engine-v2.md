# Reflection Engine v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Reflection Engine v2 as a self-maintaining reasoning engine with pure observational passes, deterministic conflict resolution, event-sourced immutability, and transactional rewrite execution.

**Architecture:** Domain models (`Entity`, `SemanticAssertion`, `FactVersion`, `TemporalWindow`, `Confidence`) in `brain-domain` expose a storage-agnostic `KnowledgeSnapshotView`. Pure reflection passes in `brain-services` analyze snapshots independently, returning `RewritePlan` intent. A deterministic `ConflictResolver` and `RewriteValidator` merge and validate plans, which are then transactionally applied to the `EventStore` by `RewriteExecutor` as immutable `FactEvent`s.

**Tech Stack:** Rust (edition 2021), `serde`, `uuid`, `rusqlite`, `parking_lot`, `tokio`.

## Global Constraints

- Domain models in `crates/brain-domain/` must have ZERO external subsystem dependencies (no database engines, async runtimes, or network protocols).
- All floating-point confidence values must be encapsulated in `Confidence` and enforced in `[0.0, 1.0]`.
- All `TemporalWindow` instances must satisfy `asserted_at <= observed_at` and `valid_from <= valid_to`.
- Fact lineage is single-sided (`supersedes: Option<FactVersionId>`).
- Reflection passes are pure, observational functions of `(Snapshot, Context)`.
- All iteration over entities, assertions, facts, and rewrite operations MUST be deterministically sorted (by ID or stable key) to guarantee event log replay determinism.

---

## Milestone 1: Domain Foundation (`crates/brain-domain`)

### Task 1: Domain Value Objects (`crates/brain-domain/src/bkf/value_objects.rs`)

**Files:**
- Create: `crates/brain-domain/src/bkf/value_objects.rs`
- Modify: `crates/brain-domain/src/bkf/mod.rs`
- Test: `crates/brain-domain/tests/value_object_tests.rs`

**Interfaces:**
- Consumes: None
- Produces: `Timestamp`, `Confidence`, `EntityName`, `PredicateName`, `PredicateCardinality`, `LiteralValue`, `ReflectionPassId`

- [ ] **Step 1: Write failing unit tests for value objects**

```rust
// crates/brain-domain/tests/value_object_tests.rs
use brain_domain::bkf::value_objects::*;

#[test]
fn test_confidence_bounds() {
    assert!(Confidence::new(0.5).is_ok());
    assert!(Confidence::new(0.0).is_ok());
    assert!(Confidence::new(1.0).is_ok());
    assert!(Confidence::new(-0.1).is_err());
    assert!(Confidence::new(1.1).is_err());
}

#[test]
fn test_entity_name_normalization() {
    let name = EntityName::new("  John  Doe  ").unwrap();
    assert_eq!(name.as_str(), "John Doe");
    assert!(EntityName::new("   ").is_err());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-domain --test value_object_tests`
Expected: FAIL with "unresolved import" or "file not found"

- [ ] **Step 3: Implement value objects**

Create `crates/brain-domain/src/bkf/value_objects.rs` with `Timestamp`, `Confidence`, `EntityName`, `PredicateName`, `PredicateCardinality`, `LiteralValue`, `ReflectionPassId`.

- [ ] **Step 4: Verify test passes**

Run: `cargo test -p brain-domain --test value_object_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/
git commit -m "feat(domain): add core reflection value objects"
```

---

### Task 2: Core Domain Entities (`crates/brain-domain/src/bkf/fact_version.rs`)

**Files:**
- Create: `crates/brain-domain/src/bkf/fact_version.rs`
- Modify: `crates/brain-domain/src/bkf/mod.rs`
- Test: `crates/brain-domain/tests/fact_version_tests.rs`

**Interfaces:**
- Consumes: Value objects from Task 1
- Produces: `Entity`, `Predicate`, `SemanticAssertion`, `TemporalWindow`, `Provenance`, `FactVersion`

- [ ] **Step 1: Write failing tests for TemporalWindow and FactVersion**

```rust
// crates/brain-domain/tests/fact_version_tests.rs
use brain_domain::bkf::fact_version::*;
use brain_domain::bkf::value_objects::*;

#[test]
fn test_temporal_window_invariants() {
    let t1 = Timestamp::now();
    let t2 = Timestamp::now();
    assert!(TemporalWindow::new(t1, t2, t1, Some(t2)).is_ok());
    assert!(TemporalWindow::new(t2, t1, t1, None).is_err()); // asserted > observed
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-domain --test fact_version_tests`
Expected: FAIL

- [ ] **Step 3: Implement TemporalWindow and FactVersion domain structures**

- [ ] **Step 4: Verify test passes**

Run: `cargo test -p brain-domain --test fact_version_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/
git commit -m "feat(domain): implement Entity, SemanticAssertion, TemporalWindow and FactVersion"
```

---

### Task 3: Snapshot Abstraction & Fact Events (`crates/brain-domain/src/bkf/snapshot.rs`)

**Files:**
- Create: `crates/brain-domain/src/bkf/snapshot.rs`
- Create: `crates/brain-domain/src/bkf/events.rs`
- Modify: `crates/brain-domain/src/bkf/mod.rs`
- Test: `crates/brain-domain/tests/snapshot_event_tests.rs`

**Interfaces:**
- Consumes: Task 1 and Task 2 models
- Produces: `KnowledgeSnapshotView` trait, `FactEvent` enum (`FactRecorded`, `FactSuperseded`, `FactArchived`)

- [ ] **Step 1: Write failing snapshot and event tests**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `KnowledgeSnapshotView` trait and `FactEvent` enum**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/
git commit -m "feat(domain): add KnowledgeSnapshotView trait and FactEvent models"
```

---

### Task 4: Declarative Rewrite Plan Models (`crates/brain-domain/src/bkf/rewrite_plan.rs`)

**Files:**
- Create: `crates/brain-domain/src/bkf/rewrite_plan.rs`
- Modify: `crates/brain-domain/src/bkf/mod.rs`
- Test: `crates/brain-domain/tests/rewrite_plan_tests.rs`

**Interfaces:**
- Consumes: Tasks 1-3 models
- Produces: `RewriteReason`, `RewriteOperation`, `RewritePlan`

- [ ] **Step 1: Write failing rewrite plan tests**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `RewriteReason`, `RewriteOperation`, `RewritePlan`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/
git commit -m "feat(domain): add RewritePlan and RewriteOperation models"
```

---

## Milestone 2: Reflection Infrastructure (`crates/brain-services`)

### Task 5: Pass Interface & Reflection Context (`crates/brain-services/src/reflection/pass_context.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/pass_context.rs`
- Modify: `crates/brain-services/src/reflection/mod.rs`
- Test: `crates/brain-services/tests/reflection_pass_tests.rs`

**Interfaces:**
- Consumes: `brain-domain` types
- Produces: `ReflectionContext`, `PassDiagnostic`, `ReflectionOutcome`, `ReflectionPass` trait (`analyze`)

- [ ] **Step 1: Write failing test for reflection pass interface**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement ReflectionContext and ReflectionPass trait**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): define ReflectionContext and ReflectionPass trait"
```

---

### Task 6: Pass Registry & Topological DAG Resolver (`crates/brain-services/src/reflection/registry_dag.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/registry_dag.rs`
- Modify: `crates/brain-services/src/reflection/mod.rs`
- Test: `crates/brain-services/tests/pass_dag_tests.rs`

**Interfaces:**
- Consumes: Task 5 pass interface
- Produces: `PassRegistry`, `PassDAGResolver` (topological sort by `dependencies()`)

- [ ] **Step 1: Write failing tests for topological pass ordering**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement topological pass DAG solver**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): add PassRegistry and topological DAG resolver"
```

---

### Task 7: Deterministic Conflict Resolver (`crates/brain-services/src/reflection/conflict_resolver.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/conflict_resolver.rs`
- Modify: `crates/brain-services/src/reflection/mod.rs`
- Test: `crates/brain-services/tests/conflict_resolver_tests.rs`

**Interfaces:**
- Consumes: `RewritePlan`, `RewriteOperation`
- Produces: `ConflictResolver` (normative merge rules: merge operations by target, pass priority, stable `ReflectionPassId` tiebreaker)

- [ ] **Step 1: Write failing unit tests for conflict resolution rules**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `ConflictResolver`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): implement deterministic ConflictResolver"
```

---

### Task 8: Rewrite Validator & Transactional Executor (`crates/brain-services/src/reflection/executor.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/executor.rs`
- Modify: `crates/brain-services/src/reflection/mod.rs`
- Test: `crates/brain-services/tests/executor_tests.rs`

**Interfaces:**
- Consumes: Storage transaction, `RewritePlan`, `ConflictResolver`
- Produces: `RewriteValidator`, `RewriteExecutor`

- [ ] **Step 1: Write failing test for validation and atomic transaction execution**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `RewriteValidator` and `RewriteExecutor`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): add RewriteValidator and transactional RewriteExecutor"
```

---

## Milestone 3: Core Reflection Passes (`crates/brain-services::reflection::passes`)

### Task 9: `CanonicalizationPass` (`crates/brain-services/src/reflection/passes/canonicalization.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/passes/canonicalization.rs`
- Test: `crates/brain-services/tests/canonicalization_pass_tests.rs`

- [ ] **Step 1: Write failing test for casing, whitespace, and alias normalization**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `CanonicalizationPass`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): implement CanonicalizationPass"
```

---

### Task 10: `ContradictionPass` (`crates/brain-services/src/reflection/passes/contradiction.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/passes/contradiction.rs`
- Test: `crates/brain-services/tests/contradiction_pass_tests.rs`

- [ ] **Step 1: Write failing test for exclusive predicate conflict resolution (e.g. LivesIn Delhi vs Mumbai)**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `ContradictionPass`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): implement ContradictionPass"
```

---

### Task 11: `DuplicateConsolidationPass` (`crates/brain-services/src/reflection/passes/duplicate.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/passes/duplicate.rs`
- Test: `crates/brain-services/tests/duplicate_pass_tests.rs`

- [ ] **Step 1: Write failing test for entity & assertion deduplication emitting `MergeFacts`**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `DuplicateConsolidationPass`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): implement DuplicateConsolidationPass"
```

---

### Task 12: `StaleKnowledgePass` (`crates/brain-services/src/reflection/passes/stale.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/passes/stale.rs`
- Test: `crates/brain-services/tests/stale_pass_tests.rs`

- [ ] **Step 1: Write failing test for expired temporal windows**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `StaleKnowledgePass`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): implement StaleKnowledgePass"
```

---

### Task 13: `ConfidenceRecalculationPass` (`crates/brain-services/src/reflection/passes/confidence.rs`)

**Files:**
- Create: `crates/brain-services/src/reflection/passes/confidence.rs`
- Test: `crates/brain-services/tests/confidence_pass_tests.rs`

- [ ] **Step 1: Write failing test for multi-source provenance corroboration**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Implement `ConfidenceRecalculationPass`**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "feat(services): implement ConfidenceRecalculationPass"
```

---

## Milestone 4: Verification & Hardening (`crates/brain-services/tests/`)

### Task 14: End-to-End Replay & Determinism Verification Suite (`crates/brain-services/tests/reflection_v2_e2e_tests.rs`)

**Files:**
- Create: `crates/brain-services/tests/reflection_v2_e2e_tests.rs`

- [ ] **Step 1: Write end-to-end multi-pass execution and replay invariance tests**
- [ ] **Step 2: Run test to verify failure**
- [ ] **Step 3: Execute full suite, verify non-mutation invariants and deterministic event replay**
- [ ] **Step 4: Verify all workspace tests pass cleanly**

Run: `cargo test --workspace`
Expected: All tests PASS cleanly

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/
git commit -m "test(services): add comprehensive determinism and replay verification suite for Reflection Engine v2"
```
