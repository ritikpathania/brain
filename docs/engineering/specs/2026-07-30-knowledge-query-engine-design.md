# Design Specification: Knowledge Query Engine (Phase 2)

## 1. Executive Summary & Goals

The **Knowledge Query Engine** provides a pure, deterministic, storage-agnostic semantic query layer over `KnowledgeSnapshotView`. It compiles typed query requests into a **Bound Query**, generates an immutable **Logical Plan**, optimizes it via a rule-based pass pipeline, lowers it into an explicit **Physical Plan**, and executes it using a **Pull-Scheduled Batch Execution Engine**.

### Architectural Invariants & Core Rules
- **Zero External Dependencies in Domain**: `brain-domain::query` contains AST, Bound Query, Logical Plan, Query Result value objects, slot-indexed binding models, and typed `QueryError` hierarchy with zero async runtimes, storage drivers, or execution dependencies.
- **Storage Independence**: All physical scan operators query exclusively via `KnowledgeSnapshotView` over typed `ScanTarget` enums. No direct access to SQLite, DuckDB, or raw event logs.
- **Deterministic Execution**: Given identical `KnowledgeSnapshotView` and query parameters, physical execution produces bitwise-identical `QueryResult`s, `QueryStatistics`, and `ExecutionStatistics`. Join ordering tie-breaks are lexicographically stable.
- **Indexed Variable Slots (`SlotId`)**: Variables resolve to zero-allocation numerical `SlotId(usize)` offsets during semantic binding. `BindingRow` holds a `Vec<Option<QueryValue>>` indexed by `SlotId`.
- **Root-Driven Pull-Scheduled Batch Processing**: Data flows through the physical operator tree in opaque vectorized batches (`BindingBatch`), scheduled top-down by root operators to support early termination (`LIMIT`) and cancellation.
- **Compiler Stage Separation**: Explicit boundaries between Semantic Binding, Logical Planning, Logical Optimization, Physical Planning, and Physical Execution.

---

## 2. Compiler Pipeline & Data Flow Architecture

```text
                                  Caller / Service
                                         │
                                         ▼
                                Query AST / Algebra
                            (brain-domain::query::ast)
                                         │
                                         ▼
                                  Semantic Binder
                     (brain-services::query::semantic_binder)
                                         │
                                         ▼
                             Bound Query & Binding Schema
                           (brain-domain::query::bound)
                                         │
                                         ▼
                                  Logical Planner
                     (brain-services::query::logical_planner)
                                         │
                                         ▼
                                   Logical Plan
                      (brain-domain::query::logical_plan)
                                         │
                                         ▼
                                 Logical Optimizer
                    (brain-services::query::logical_optimizer)
                         ├── Normalization Pass
                         ├── Predicate Pushdown Pass
                         ├── Constraint Folding Pass
                         ├── Traversal Simplification Pass
                         ├── Projection Pruning Pass
                         └── Join Ordering Pass (Stable Tie-Break)
                                         │
                                         ▼
                                  Physical Planner
                     (brain-services::query::physical_planner)
                                         │
                                         ▼
                                   Physical Plan
                       (brain-services::query::physical_plan)
                                         │
                                         ▼
                        Pull-Scheduled Batch Execution Engine
                     (brain-services::query::execution_engine)
                                         │
                                         ▼
                                    QueryResult
                       (brain-domain::query::result)
```

---

## 3. Detailed Component & Module Layout

### 3.1 `crates/brain-domain/src/query/` (Domain Model & Value Objects)

- **`ast.rs`**: Builder and AST expression models.
  - `Query`: Entry point containing filters, pattern rules, temporal constraints, traversal specs, and limit/offset bounds.
  - `QueryVar`: Variable identifier (e.g. `?person`, `?city`).
  - `Pattern`: Pattern expression (e.g. `Pattern::triple(subject, predicate, object)`).
- **`bound.rs`**: Semantic binding representation.
  - `SlotId`: Strongly typed numerical slot offset (`SlotId(pub usize)`).
  - `BindingSchema`: Maps `QueryVar` to `SlotId`.
  - `BoundQuery`: Validated query AST with resolved `BindingSchema`, canonicalized predicate names, and variable slot assignments.
- **`scan_target.rs`**: Typed domain scan target enum.
  - `ScanTarget`: `ActiveFacts`, `HistoricalFacts`, `Entities`, `Assertions`, `Predicates`.
- **`logical_plan.rs`**: Immutable logical algebra nodes.
  - `LogicalPlan`: Enum containing `Scan { target: ScanTarget }`, `Filter`, `Join`, `Traverse`, `TemporalWindowFilter`, `Project`, `Limit`, `Sort` (extensible for future `Aggregate`, `Distinct`, `Union`, `Exists`, `OptionalMatch`).
- **`filters.rs`**: Predicate expressions over entity names, kinds, literal values, and confidence bounds.
- **`temporal.rs`**: Point-in-time (`at(timestamp)`), window (`between(start, end)`), active (`active()`), and historical (`historical()`) visibility filters.
- **`traversal.rs`**: Graph traversal specifiers: `neighbors(entity)`, `shortest_path(from, to, max_depth)`, and `lineage(fact)`.
- **`errors.rs`**: Strongly typed `QueryError` and `QueryExecutionError` hierarchy.
- **`result.rs`**: Immutable query response value objects.
  - `QueryResult`: Contains `schema: BindingSchema`, `bindings: Vec<BindingRow>`, `statistics: QueryStatistics`, and `execution_statistics: ExecutionStatistics`.
  - `BindingRow`: Compact `Vec<Option<QueryValue>>` indexed by `SlotId`.
  - `QueryStatistics`: Logical result metadata (result count, logical plan depth, traversal depth, pattern count).
  - `ExecutionStatistics`: Runtime metrics (rows scanned, total batches, execution `Duration`, memory bytes, `OperatorMetrics` list).
- **`explain.rs`**: Formatted `ExplainPlan` output containing `logical_plan_str` and `physical_plan_str`.

### 3.2 `crates/brain-services/src/query/` (Planner, Optimizer & Execution Engine)

- **`semantic_binder.rs`**: Validates AST variables, assigns `SlotId`s into `BindingSchema`, checks scope, canonicalizes predicate names, and emits `BoundQuery`.
- **`logical_planner.rs`**: Translates `BoundQuery` into `LogicalPlan`.
- **`logical_optimizer.rs`**: Executes a deterministic multi-pass rule pipeline:
  1. `NormalizationPass`: Standardizes expression trees.
  2. `PredicatePushdownPass`: Pushes filters closer to scan operators.
  3. `ConstraintFoldingPass`: Simplifies redundant temporal/confidence bounds.
  4. `TraversalSimplificationPass`: Merges redundant path steps.
  5. `ProjectionPruningPass`: Removes unused variable bindings.
  6. `JoinOrderingPass`: Reorders joins based on pattern selectivity (lexicographically stable tie-breaking).
- **`physical_planner.rs`**: Selects physical operators and constructs `PhysicalPlan`.
- **`physical_plan.rs`**: Immutable tree representation of physical operators ready for execution or `EXPLAIN`.
- **`context.rs`**: Split into immutable `ExecutionConfig` (`query_id`, `batch_size`, `execution_budget`, `execution_mode`, `feature_flags`) and mutable `ExecutionState` (`cancellation_token`, telemetry collectors).
- **`batch.rs`**: `BindingBatch` opaque vector batch structure supporting `append`, `clear`, `len`, `capacity`, `is_empty`, and iteration over `BindingRow`s.
- **`execution_engine.rs`**: Drives `PhysicalPlan` execution against `KnowledgeSnapshotView`, wrapping operators with duration timing.
- **`explain_formatter.rs`**: Formatter generating readable text for `EXPLAIN` queries.
- **`operators/`**: Extensible physical operators implementing `PhysicalOperator`:
  - `ScanOperator`: Scans entities, assertions, or active facts from `KnowledgeSnapshotView` over typed `ScanTarget`.
  - `FilterOperator`: Evaluates boolean expressions over `BindingBatch`.
  - `JoinOperator`: Executes joins across batches (strategy selected by Physical Planner).
  - `TraverseOperator`: BFS/DFS path expansion across graph edges.
  - `TemporalOperator`: Filters fact visibility by point-in-time or window constraints.
  - `LimitOperator`: Truncates batch streams and signals completion.

---

## 4. Physical Operator Execution Contract

```rust
/// Status returned by a physical operator after pulling a batch.
pub enum BatchStatus {
    /// Batch filled with bindings, more data available upstream.
    HaveMore,
    /// Batch filled with bindings (possibly empty), upstream depleted.
    Exhausted,
}

/// Pure physical operator interface.
pub trait PhysicalOperator: Send + Sync {
    /// Pulls the next vectorized batch from this operator.
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError>;

    /// Returns row and batch metrics for this operator.
    fn metrics(&self) -> OperatorMetrics;
}
```

---

## 5. Verification & Testing Strategy

1. **Unit Tests (`brain-domain` & `brain-services`)**:
   - Semantic Binder validation & slot assignment (`SlotId`).
   - Logical Planner translation correctness.
   - Individual Optimizer Rule Pass transformations.
   - Physical Operator batch evaluation in isolation.
2. **Planner Invariant & Snapshot Tests (`crates/brain-services/tests/query_snapshot_tests.rs`)**:
   - Planner idempotence and logical plan determinism.
   - `LogicalPlan` snapshot verification.
   - `PhysicalPlan` snapshot verification.
   - `EXPLAIN` format snapshot verification.
3. **Integration Tests (`crates/brain-services/tests/query_engine_tests.rs`)**:
   - End-to-end multi-pattern graph query execution against populated `KnowledgeSnapshotView`.
   - Point-in-time temporal queries verifying active vs. historical fact snapshot visibility.
   - Neighborhood & shortest-path graph traversal verification.
4. **Property & Invariant Tests (`crates/brain-services/tests/query_engine_property_tests.rs`)**:
   - **Execution Determinism**: Executing the same query AST against the same snapshot 100 times produces identical `QueryResult`s and identical `ExecutionStatistics`.
   - **Batch-Size Invariance**: Executing queries with `batch_size = 1`, `batch_size = 10`, and `batch_size = 100` produces identical final bindings.
   - **Early Limit Cutoff**: Querying with `limit(5)` over 10,000 matching facts scans $\le$ batch size facts beyond limit.
