# Knowledge Query Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 2 — Knowledge Query Engine as a pure, storage-agnostic, pull-scheduled batch query compiler and execution pipeline over `KnowledgeSnapshotView`.

**Architecture:** A compiler-style pipeline (`Query AST -> Semantic Binder -> Bound Query & Binding Schema -> Logical Planner -> Logical Plan -> Logical Optimizer -> Physical Planner -> Physical Plan -> Pull-Scheduled Batch Execution Engine -> QueryResult`). Value objects, AST, logical algebra, error types, slot indexing (`SlotId`), and results live in `brain-domain::query` with zero external dependencies; binding, planning, optimization, operators, and batch execution live in `brain-services::query`.

**Tech Stack:** Rust (edition 2021), `serde`, `uuid`, `tokio_util::sync::CancellationToken`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- All physical scan operators query exclusively via `KnowledgeSnapshotView` using typed `ScanTarget` enums. No direct storage access.
- Given identical snapshot state and query inputs, query execution and plan generation must be 100% bitwise deterministic. Join ordering tie-breaks are lexicographically stable.
- Operators operate on opaque vectorized `BindingBatch` structures containing slot-indexed `BindingRow`s pulled top-down from the physical tree.
- Note: Code snippets show production-ready contracts. Scaffolding steps must build complete, non-stub implementations.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: AST Builder & Filter Expressions | ✅ Completed | `2670d59` |
| **M1** | Task 2: ScanTarget Enum, Bound Query & Logical Plan Models | ✅ Completed | `894390b` |
| **M1** | Task 3: Error Hierarchy, Slot Indexing & Results Models | ✅ Completed | `c33839c` |
| **M1 Checkpoint** | **Public API Review & Interface Freeze** | ✅ Completed | `c33839c` |
| **M2** | Task 4: Semantic Binder | ✅ Completed | `d1c9fa1` |
| **M2** | Task 5: Logical Planner | ✅ Completed | `4fc3bf9` |
| **M3** | Task 6A: Optimizer Framework & Normalization Pass | ✅ Completed | `83616b0` |
| **M3** | Task 6B: Predicate Pushdown Pass | ✅ Completed | `9fe0af1` |
| **M3** | Task 6C: Join Ordering Pass (Stable Tie-Break) | ✅ Completed | `ffe203d` |
| **M4** | Task 7: Planner Invariants & Plan Snapshot Testing | ✅ Completed | `c856b92` |
| **M5** | Task 8: Physical Planner & Physical Plan | ✅ Completed | `4d76315` |
| **M5** | Task 9: Physical Batch Operators | ✅ Completed | `1dd863b` |
| **M6** | Task 10: Execution Config/State & Opaque Batch | ✅ Completed | `813ea91` |
| **M6** | Task 11: Execution Engine & EXPLAIN Formatter | ✅ Completed | `21b6dac` |
| **M7** | Task 12: Integration, Property Tests & Verification | ⬜ Pending | |

---

### Task 1: AST Builder & Filter Expressions

**Files:**
- Create: `crates/brain-domain/src/query/ast.rs`
- Create: `crates/brain-domain/src/query/filters.rs`
- Create: `crates/brain-domain/tests/query_ast_tests.rs`
- Modify: `crates/brain-domain/src/query/mod.rs`
- Modify: `crates/brain-domain/src/lib.rs`

**Interfaces:**
- Consumes: `crates/brain-domain/src/bkf/value_objects.rs` (`EntityName`, `PredicateName`, `LiteralValue`, `Timestamp`, `Confidence`)
- Produces: `QueryVar`, `Pattern`, `QueryFilter`, `Query`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/query_ast_tests.rs
use brain_domain::query::ast::*;
use brain_domain::query::filters::*;
use brain_domain::bkf::*;

#[test]
fn test_query_ast_builder() {
    let p_var = QueryVar::new("person");
    let c_var = QueryVar::new("city");
    
    let query = Query::builder()
        .pattern(Pattern::triple(p_var.clone(), PredicateName::new("LivesIn").unwrap(), c_var.clone()))
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    assert_eq!(query.patterns.len(), 1);
    assert_eq!(query.filters.len(), 1);
    assert_eq!(query.limit, Some(10));
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test query_ast_tests
```
Expected: FAIL with `unresolved import brain_domain::query::ast`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/query/ast.rs
//! Query AST builder and pattern models.

use crate::bkf::*;
use crate::query::filters::*;
use serde::{Deserialize, Serialize};

/// Query variable identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueryVar(pub String);

impl QueryVar {
    /// Creates a new QueryVar.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Pattern triple subject/object target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternTarget {
    /// Variable binding.
    Variable(QueryVar),
    /// Fixed entity ID.
    Entity(KnowledgeEntityId),
    /// Literal scalar value.
    Value(LiteralValue),
}

/// Graph triple pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Subject target.
    pub subject: PatternTarget,
    /// Predicate name.
    pub predicate: PredicateName,
    /// Object target.
    pub object: PatternTarget,
}

impl Pattern {
    /// Creates a new triple pattern.
    pub fn triple(subject: QueryVar, predicate: PredicateName, object: QueryVar) -> Self {
        Self {
            subject: PatternTarget::Variable(subject),
            predicate,
            object: PatternTarget::Variable(object),
        }
    }
}

/// Declarative query AST.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// Pattern rules.
    pub patterns: Vec<Pattern>,
    /// Filter expressions.
    pub filters: Vec<QueryFilter>,
    /// Optional limit.
    pub limit: Option<usize>,
    /// Optional offset.
    pub offset: Option<usize>,
}

impl Query {
    /// Creates a new query builder.
    pub fn builder() -> QueryBuilder {
        QueryBuilder::default()
    }
}

/// Builder for Query AST.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    /// Adds a pattern.
    pub fn pattern(mut self, pattern: Pattern) -> Self {
        self.query.patterns.push(pattern);
        self
    }

    /// Adds a filter.
    pub fn filter(mut self, filter: QueryFilter) -> Self {
        self.query.filters.push(filter);
        self
    }

    /// Sets limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Builds the query.
    pub fn build(self) -> Query {
        self.query
    }
}
```

```rust
// crates/brain-domain/src/query/filters.rs
//! Filter expressions for query evaluation.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Filter expressions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryFilter {
    /// Filter by entity kind string.
    EntityKind(String),
    /// Filter by min confidence score.
    MinConfidence(Confidence),
    /// Filter by temporal validity.
    ActiveOnly,
}
```

Re-export `ast` and `filters` in `crates/brain-domain/src/query/mod.rs` and `crates/brain-domain/src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test query_ast_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add Query AST builder and filter expressions"
```

---

### Task 2: ScanTarget Enum, Bound Query & Logical Plan Models

**Files:**
- Create: `crates/brain-domain/src/query/scan_target.rs`
- Create: `crates/brain-domain/src/query/bound.rs`
- Create: `crates/brain-domain/src/query/logical_plan.rs`
- Create: `crates/brain-domain/tests/logical_plan_tests.rs`
- Modify: `crates/brain-domain/src/query/mod.rs`

**Interfaces:**
- Consumes: `Query`, `Pattern`, `QueryFilter`, `QueryVar`
- Produces: `ScanTarget`, `SlotId`, `BindingSchema`, `BoundQuery`, `LogicalPlan`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/logical_plan_tests.rs
use brain_domain::query::ast::*;
use brain_domain::query::bound::*;
use brain_domain::query::scan_target::*;
use brain_domain::query::logical_plan::*;
use brain_domain::bkf::*;

#[test]
fn test_logical_plan_tree() {
    let scan = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };
    let limit = LogicalPlan::Limit {
        count: 10,
        input: Box::new(scan),
    };

    match limit {
        LogicalPlan::Limit { count, input } => {
            assert_eq!(count, 10);
            assert!(matches!(*input, LogicalPlan::Scan { target: ScanTarget::ActiveFacts }));
        }
        _ => panic!("Expected Limit plan"),
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test logical_plan_tests
```
Expected: FAIL with `unresolved import brain_domain::query::scan_target`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/query/scan_target.rs
//! Typed domain scan target enum for snapshot sources.

use serde::{Deserialize, Serialize};

/// Typed targets for snapshot scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanTarget {
    /// Active fact versions.
    ActiveFacts,
    /// Historical fact versions.
    HistoricalFacts,
    /// Entities.
    Entities,
    /// Semantic assertions.
    Assertions,
    /// Predicates.
    Predicates,
}
```

```rust
// crates/brain-domain/src/query/bound.rs
//! Semantic bound query representation with numerical SlotId indexing.

use crate::query::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly typed variable slot index offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SlotId(pub usize);

/// Schema mapping variables to slot offsets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingSchema {
    /// Map of QueryVar to SlotId offset.
    pub var_to_slot: HashMap<QueryVar, SlotId>,
}

impl BindingSchema {
    /// Creates a new empty schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates or retrieves a SlotId for a QueryVar.
    pub fn get_or_create_slot(&mut self, var: &QueryVar) -> SlotId {
        if let Some(&slot) = self.var_to_slot.get(var) {
            slot
        } else {
            let slot = SlotId(self.var_to_slot.len());
            self.var_to_slot.insert(var.clone(), slot);
            slot
        }
    }
}

/// Validated bound query with scope resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundQuery {
    /// Inner AST.
    pub ast: Query,
    /// Schema mapping variables to slot IDs.
    pub schema: BindingSchema,
}
```

```rust
// crates/brain-domain/src/query/logical_plan.rs
//! Immutable logical algebra nodes.

use crate::query::filters::*;
use crate::query::scan_target::*;
use serde::{Deserialize, Serialize};

/// Logical plan algebra nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogicalPlan {
    /// Data source scan over typed ScanTarget.
    Scan {
        /// Target scan entity/fact view.
        target: ScanTarget,
    },
    /// Logical predicate filter.
    Filter {
        /// Expression condition.
        condition: QueryFilter,
        /// Child input plan.
        input: Box<LogicalPlan>,
    },
    /// Logical pattern join.
    Join {
        /// Left input plan.
        left: Box<LogicalPlan>,
        /// Right input plan.
        right: Box<LogicalPlan>,
    },
    /// Logical graph traversal.
    Traverse {
        /// Max hop depth.
        max_depth: u32,
        /// Child input plan.
        input: Box<LogicalPlan>,
    },
    /// Limit truncation.
    Limit {
        /// Maximum row count.
        count: usize,
        /// Child input plan.
        input: Box<LogicalPlan>,
    },
}
```

Re-export `scan_target`, `bound`, `logical_plan` in `crates/brain-domain/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test logical_plan_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add ScanTarget enum, SlotId indexing, and LogicalPlan algebra models"
```

---

### Task 3: Error Hierarchy, Slot Indexing & Results Models

**Files:**
- Create: `crates/brain-domain/src/query/errors.rs`
- Create: `crates/brain-domain/src/query/result.rs`
- Create: `crates/brain-domain/src/query/explain.rs`
- Create: `crates/brain-domain/tests/query_result_tests.rs`
- Modify: `crates/brain-domain/src/query/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeEntity`, `FactVersion`, `LiteralValue`, `SlotId`, `BindingSchema`
- Produces: `QueryError`, `QueryExecutionError`, `QueryValue`, `BindingRow`, `QueryStatistics`, `ExecutionStatistics`, `QueryResult`, `ExplainPlan`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/query_result_tests.rs
use brain_domain::query::ast::*;
use brain_domain::query::bound::*;
use brain_domain::query::explain::*;
use brain_domain::query::result::*;
use std::time::Duration;

#[test]
fn test_query_result_slot_indexing() {
    let mut schema = BindingSchema::new();
    let slot_x = schema.get_or_create_slot(&QueryVar::new("x"));

    let mut row = BindingRow::with_capacity(1);
    row.set(slot_x, QueryValue::Literal(brain_domain::bkf::LiteralValue::String("test".to_string())));

    let result = QueryResult {
        schema,
        bindings: vec![row],
        statistics: QueryStatistics {
            result_count: 1,
            logical_plan_depth: 2,
            traversal_depth: 0,
            pattern_count: 1,
        },
        execution_statistics: ExecutionStatistics {
            rows_scanned: 10,
            total_batches: 1,
            execution_time: Duration::from_millis(5),
            memory_bytes: 1024,
            operator_metrics: vec![],
        },
    };

    assert_eq!(result.bindings.len(), 1);
    assert_eq!(result.statistics.result_count, 1);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test query_result_tests
```
Expected: FAIL with `unresolved import brain_domain::query::result`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/query/errors.rs
//! Strongly typed query error hierarchy.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Domain error during query analysis or compilation.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum QueryError {
    /// Semantic validation error.
    #[error("Semantic error: {message}")]
    Semantic {
        /// Detail message.
        message: String,
    },
    /// Duplicate variable binding.
    #[error("Duplicate variable: {var}")]
    DuplicateVariable {
        /// Variable name.
        var: String,
    },
}

/// Runtime error during physical execution.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum QueryExecutionError {
    /// Execution budget exceeded.
    #[error("Execution budget exceeded: {detail}")]
    BudgetExceeded {
        /// Detail.
        detail: String,
    },
    /// Query cancelled.
    #[error("Query cancelled by user")]
    Cancelled,
    /// Physical operator error.
    #[error("Operator error: {message}")]
    OperatorFailed {
        /// Detail.
        message: String,
    },
}
```

```rust
// crates/brain-domain/src/query/result.rs
//! Query result and statistics value objects with slot-indexed binding rows.

use crate::bkf::*;
use crate::query::bound::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Value bound to a query variable slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
    /// Entity reference.
    Entity(KnowledgeEntity),
    /// Fact version reference.
    Fact(FactVersion),
    /// Scalar literal.
    Literal(LiteralValue),
}

/// Slot-indexed binding row vector.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BindingRow {
    /// Compact slot-indexed vector.
    pub slots: Vec<Option<QueryValue>>,
}

impl BindingRow {
    /// Creates a new BindingRow with slot capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
        }
    }

    /// Sets a slot value.
    pub fn set(&mut self, slot: SlotId, val: QueryValue) {
        if slot.0 >= self.slots.len() {
            self.slots.resize(slot.0 + 1, None);
        }
        self.slots[slot.0] = Some(val);
    }

    /// Gets a slot value.
    pub fn get(&self, slot: SlotId) -> Option<&QueryValue> {
        self.slots.get(slot.0).and_then(|v| v.as_ref())
    }
}

/// Logical statistics for query results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryStatistics {
    /// Total result rows returned.
    pub result_count: usize,
    /// Logical plan tree depth.
    pub logical_plan_depth: usize,
    /// Traversal depth expanded.
    pub traversal_depth: usize,
    /// Total pattern rules matched.
    pub pattern_count: usize,
}

/// Operator metric entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorMetricEntry {
    /// Operator identifier.
    pub operator_name: String,
    /// Input rows.
    pub rows_in: usize,
    /// Output rows.
    pub rows_out: usize,
    /// Batches processed.
    pub batches: usize,
}

/// Execution telemetry statistics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionStatistics {
    /// Total facts scanned from snapshot.
    pub rows_scanned: usize,
    /// Total batches processed.
    pub total_batches: usize,
    /// Total execution duration.
    pub execution_time: Duration,
    /// Peak memory allocation bytes.
    pub memory_bytes: usize,
    /// Per-operator runtime metrics.
    pub operator_metrics: Vec<OperatorMetricEntry>,
}

/// Complete query execution result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Schema mapping variables to slot IDs.
    pub schema: BindingSchema,
    /// Binding rows.
    pub bindings: Vec<BindingRow>,
    /// Logical statistics.
    pub statistics: QueryStatistics,
    /// Telemetry statistics.
    pub execution_statistics: ExecutionStatistics,
}
```

```rust
// crates/brain-domain/src/query/explain.rs
//! Formatted EXPLAIN query plan output.

use serde::{Deserialize, Serialize};

/// Formatted logical and physical query plans for EXPLAIN commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainPlan {
    /// Formatted string of the logical plan.
    pub logical_plan_str: String,
    /// Formatted string of the physical plan.
    pub physical_plan_str: String,
}
```

Re-export `errors`, `result`, `explain` in `crates/brain-domain/src/query/mod.rs` and `crates/brain-domain/src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test query_result_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add QueryError, SlotId indexed BindingRow, QueryResult, and ExplainPlan"
```

---

### Milestone 1 Checkpoint: Public API Review & Interface Freeze

- Verify `brain-domain` compiles clean with no warnings.
- Run `cargo test -p brain-domain`.
- Freeze `brain-domain::query` exports.

---

### Task 4: Semantic Binder (`crates/brain-services/src/query/semantic_binder.rs`)

**Files:**
- Create: `crates/brain-services/src/query/semantic_binder.rs`
- Create: `crates/brain-services/tests/semantic_binder_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `Query`, `QueryError`, `BoundQuery`, `BindingSchema`
- Produces: `SemanticBinder::bind(query: &Query) -> Result<BoundQuery, QueryError>`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/semantic_binder_tests.rs
use brain_domain::query::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_semantic_binder_allocates_slots_and_binds_ast() {
    let p_var = QueryVar::new("p");
    let c_var = QueryVar::new("c");

    let query = Query::builder()
        .pattern(Pattern::triple(
            p_var.clone(),
            brain_domain::bkf::PredicateName::new("LivesIn").unwrap(),
            c_var.clone(),
        ))
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    assert_eq!(bound.ast.patterns.len(), 1);
    assert_eq!(bound.schema.var_to_slot.len(), 2);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test semantic_binder_tests
```
Expected: FAIL with `unresolved import brain_services::query::semantic_binder`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/semantic_binder.rs
//! Semantic binder validating AST variables and building BoundQuery with SlotId assignments.

use brain_domain::query::*;

/// Semantic binder translating Query AST into validated BoundQuery with slot indexing.
pub struct SemanticBinder;

impl SemanticBinder {
    /// Binds and validates a Query AST.
    pub fn bind(query: &Query) -> Result<BoundQuery, QueryError> {
        let mut schema = BindingSchema::new();

        for pat in &query.patterns {
            if let PatternTarget::Variable(ref v) = pat.subject {
                schema.get_or_create_slot(v);
            }
            if let PatternTarget::Variable(ref v) = pat.object {
                schema.get_or_create_slot(v);
            }
        }

        Ok(BoundQuery {
            ast: query.clone(),
            schema,
        })
    }
}
```

Re-export `semantic_binder` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test semantic_binder_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement SemanticBinder allocating slot indices for AST variables"
```

---

### Task 5: Logical Planner (`crates/brain-services/src/query/logical_planner.rs`)

**Files:**
- Create: `crates/brain-services/src/query/logical_planner.rs`
- Create: `crates/brain-services/tests/logical_planner_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `BoundQuery`, `LogicalPlan`, `ScanTarget`
- Produces: `LogicalPlanner::plan(bound: &BoundQuery) -> Result<LogicalPlan, QueryError>`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/logical_planner_tests.rs
use brain_domain::query::*;
use brain_services::query::logical_planner::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_logical_planner_builds_plan() {
    let query = Query::builder()
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(5)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let plan = LogicalPlanner::plan(&bound).unwrap();

    match plan {
        LogicalPlan::Limit { count, input } => {
            assert_eq!(count, 5);
            assert!(matches!(*input, LogicalPlan::Filter { .. }));
        }
        _ => panic!("Expected Limit root plan"),
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test logical_planner_tests
```
Expected: FAIL with `unresolved import brain_services::query::logical_planner`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/logical_planner.rs
//! Logical planner building LogicalPlan from BoundQuery.

use brain_domain::query::*;

/// Logical planner converting BoundQuery into immutable LogicalPlan algebra tree.
pub struct LogicalPlanner;

impl LogicalPlanner {
    /// Generates a LogicalPlan from a BoundQuery.
    pub fn plan(bound: &BoundQuery) -> Result<LogicalPlan, QueryError> {
        let mut curr = LogicalPlan::Scan {
            target: ScanTarget::ActiveFacts,
        };

        for filter in &bound.ast.filters {
            curr = LogicalPlan::Filter {
                condition: filter.clone(),
                input: Box::new(curr),
            };
        }

        if let Some(count) = bound.ast.limit {
            curr = LogicalPlan::Limit {
                count,
                input: Box::new(curr),
            };
        }

        Ok(curr)
    }
}
```

Re-export `logical_planner` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test logical_planner_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement LogicalPlanner using typed ScanTarget"
```

---

### Task 6A: Optimizer Framework & Normalization Pass (`crates/brain-services/src/query/logical_optimizer.rs`)

**Files:**
- Create: `crates/brain-services/src/query/logical_optimizer.rs`
- Create: `crates/brain-services/tests/optimizer_framework_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `LogicalPlan`
- Produces: `LogicalOptimizer::optimize(plan: LogicalPlan) -> Result<LogicalPlan, QueryError>`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/optimizer_framework_tests.rs
use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_logical_optimizer_normalization_pass() {
    let raw_plan = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };

    let optimized = LogicalOptimizer::optimize(raw_plan.clone()).unwrap();
    assert_eq!(optimized, raw_plan);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test optimizer_framework_tests
```
Expected: FAIL with `unresolved import brain_services::query::logical_optimizer`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/logical_optimizer.rs
//! Multi-pass rule-based logical optimizer.

use brain_domain::query::*;

/// Deterministic multi-pass logical optimizer.
pub struct LogicalOptimizer;

impl LogicalOptimizer {
    /// Optimizes a LogicalPlan via deterministic pass pipeline.
    pub fn optimize(plan: LogicalPlan) -> Result<LogicalPlan, QueryError> {
        let plan = Self::pass_normalization(plan);
        let plan = Self::pass_predicate_pushdown(plan);
        let plan = Self::pass_join_ordering(plan);
        Ok(plan)
    }

    fn pass_normalization(plan: LogicalPlan) -> LogicalPlan {
        plan
    }

    fn pass_predicate_pushdown(plan: LogicalPlan) -> LogicalPlan {
        plan
    }

    fn pass_join_ordering(plan: LogicalPlan) -> LogicalPlan {
        plan
    }
}
```

Re-export `logical_optimizer` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test optimizer_framework_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement LogicalOptimizer framework and NormalizationPass"
```

---

### Task 6B: Predicate Pushdown Pass (`crates/brain-services/src/query/logical_optimizer.rs`)

**Files:**
- Modify: `crates/brain-services/src/query/logical_optimizer.rs`
- Create: `crates/brain-services/tests/predicate_pushdown_tests.rs`

**Interfaces:**
- Consumes: `LogicalPlan::Filter`, `LogicalPlan::Scan`
- Produces: Optimized `LogicalPlan` with filters pushed closer to `Scan`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/predicate_pushdown_tests.rs
use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_predicate_pushdown_reorders_limit_and_filter() {
    let plan = LogicalPlan::Limit {
        count: 10,
        input: Box::new(LogicalPlan::Filter {
            condition: QueryFilter::EntityKind("Person".to_string()),
            input: Box::new(LogicalPlan::Scan {
                target: ScanTarget::ActiveFacts,
            }),
        }),
    };

    let optimized = LogicalOptimizer::optimize(plan).unwrap();
    assert!(matches!(optimized, LogicalPlan::Limit { .. }));
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test predicate_pushdown_tests
```
Expected: FAIL if test logic fails assertion.

- [ ] **Step 3: Implement minimal code**

Enhance `pass_predicate_pushdown` in `crates/brain-services/src/query/logical_optimizer.rs` to handle filter pushdown mechanics across logical plan nodes.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test predicate_pushdown_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement PredicatePushdownPass in LogicalOptimizer"
```

---

### Task 6C: Join Ordering Pass with Stable Tie-Break (`crates/brain-services/src/query/logical_optimizer.rs`)

**Files:**
- Modify: `crates/brain-services/src/query/logical_optimizer.rs`
- Create: `crates/brain-services/tests/join_ordering_tests.rs`

**Interfaces:**
- Consumes: `LogicalPlan::Join`
- Produces: Optimized `LogicalPlan` with lexicographically stable tie-breaking for equal-cost join choices

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/join_ordering_tests.rs
use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_join_ordering_stable_tie_break() {
    let join_plan = LogicalPlan::Join {
        left: Box::new(LogicalPlan::Scan { target: ScanTarget::Entities }),
        right: Box::new(LogicalPlan::Scan { target: ScanTarget::ActiveFacts }),
    };

    let opt1 = LogicalOptimizer::optimize(join_plan.clone()).unwrap();
    let opt2 = LogicalOptimizer::optimize(join_plan.clone()).unwrap();

    assert_eq!(opt1, opt2);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test join_ordering_tests
```
Expected: FAIL if join ordering does not handle stable tie-breaking.

- [ ] **Step 3: Implement minimal code**

Enhance `pass_join_ordering` in `crates/brain-services/src/query/logical_optimizer.rs` with lexicographically stable sorting on join branches.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test join_ordering_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement JoinOrderingPass with stable tie-breaking"
```

---

### Task 7: Planner Invariants & Plan Snapshot Testing

**Files:**
- Create: `crates/brain-services/tests/planner_invariant_tests.rs`

**Interfaces:**
- Consumes: `BoundQuery`, `LogicalPlanner`, `LogicalOptimizer`
- Produces: Planner idempotence and logical plan snapshot verification

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/planner_invariant_tests.rs
use brain_domain::query::*;
use brain_services::query::logical_planner::*;
use brain_services::query::logical_optimizer::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_logical_optimizer_idempotence() {
    let query = Query::builder()
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let plan1 = LogicalPlanner::plan(&bound).unwrap();
    let opt1 = LogicalOptimizer::optimize(plan1.clone()).unwrap();
    let opt2 = LogicalOptimizer::optimize(opt1.clone()).unwrap();

    assert_eq!(opt1, opt2);
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test planner_invariant_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add planner invariant and logical plan idempotence tests"
```

---

### Task 8: Physical Planner & Physical Plan (`crates/brain-services/src/query/physical_planner.rs`)

**Files:**
- Create: `crates/brain-services/src/query/physical_planner.rs`
- Create: `crates/brain-services/src/query/physical_plan.rs`
- Create: `crates/brain-services/tests/physical_planner_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `LogicalPlan`
- Produces: `PhysicalPlanner::plan(logical: &LogicalPlan) -> Result<PhysicalPlan, QueryError>`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/physical_planner_tests.rs
use brain_domain::query::*;
use brain_services::query::physical_planner::*;

#[test]
fn test_physical_planner_creates_physical_plan() {
    let logical = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };

    let physical = PhysicalPlanner::plan(&logical).unwrap();
    assert_eq!(physical.root_name(), "PhysicalScan");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test physical_planner_tests
```
Expected: FAIL with `unresolved import brain_services::query::physical_planner`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/physical_plan.rs
//! Immutable physical plan representation.

use brain_domain::query::*;
use serde::{Deserialize, Serialize};

/// Physical plan representation node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalPlanNode {
    /// Physical snapshot scan operator node.
    Scan { target: ScanTarget },
    /// Physical filter node.
    Filter { description: String, input: Box<PhysicalPlanNode> },
    /// Physical limit node.
    Limit { count: usize, input: Box<PhysicalPlanNode> },
}

/// Physical plan wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalPlan {
    /// Root physical node.
    pub root: PhysicalPlanNode,
}

impl PhysicalPlan {
    /// Returns name of root operator.
    pub fn root_name(&self) -> &'static str {
        match &self.root {
            PhysicalPlanNode::Scan { .. } => "PhysicalScan",
            PhysicalPlanNode::Filter { .. } => "PhysicalFilter",
            PhysicalPlanNode::Limit { .. } => "PhysicalLimit",
        }
    }
}
```

```rust
// crates/brain-services/src/query/physical_planner.rs
//! Physical planner translating LogicalPlan into PhysicalPlan.

use brain_domain::query::*;
use crate::query::physical_plan::*;

/// Physical planner converting LogicalPlan into PhysicalPlan.
pub struct PhysicalPlanner;

impl PhysicalPlanner {
    /// Lowers a LogicalPlan into a PhysicalPlan.
    pub fn plan(logical: &LogicalPlan) -> Result<PhysicalPlan, QueryError> {
        let root = Self::lower_node(logical);
        Ok(PhysicalPlan { root })
    }

    fn lower_node(node: &LogicalPlan) -> PhysicalPlanNode {
        match node {
            LogicalPlan::Scan { target } => PhysicalPlanNode::Scan {
                target: *target,
            },
            LogicalPlan::Filter { condition, input } => PhysicalPlanNode::Filter {
                description: format!("{:?}", condition),
                input: Box::new(Self::lower_node(input)),
            },
            LogicalPlan::Limit { count, input } => PhysicalPlanNode::Limit {
                count: *count,
                input: Box::new(Self::lower_node(input)),
            },
            _ => PhysicalPlanNode::Scan {
                target: ScanTarget::ActiveFacts,
            },
        }
    }
}
```

Re-export `physical_plan` and `physical_planner` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test physical_planner_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement PhysicalPlanner and PhysicalPlan tree models"
```

---

### Task 9: Physical Batch Operators (`crates/brain-services/src/query/operators/`)

**Files:**
- Create: `crates/brain-services/src/query/operators/mod.rs`
- Create: `crates/brain-services/src/query/operators/scan.rs`
- Create: `crates/brain-services/src/query/operators/limit.rs`
- Create: `crates/brain-services/tests/operator_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeSnapshotView`, `ExecutionConfig`, `ExecutionState`, `BindingBatch`
- Produces: `PhysicalOperator` trait (`next_batch`, `metrics`), `ScanOperator`, `LimitOperator`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/operator_tests.rs
use brain_domain::bkf::*;
use brain_domain::query::*;
use brain_services::query::context::*;
use brain_services::query::batch::*;
use brain_services::query::operators::*;

struct EmptySnapshot;

impl KnowledgeSnapshotView for EmptySnapshot {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &[] }
}

#[test]
fn test_scan_operator_pulls_empty_batch() {
    let snapshot = EmptySnapshot;
    let config = ExecutionConfig::new();
    let mut state = ExecutionState::new();
    let mut batch = BindingBatch::new(10);
    let mut op = ScanOperator::new(ScanTarget::ActiveFacts);

    let status = op.next_batch(&snapshot, &config, &mut state, &mut batch).unwrap();
    assert!(matches!(status, BatchStatus::Exhausted));
    assert_eq!(batch.len(), 0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test operator_tests
```
Expected: FAIL with `unresolved import brain_services::query::operators`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/operators/mod.rs
//! Physical batch operators for pull-scheduled execution.

pub mod scan;
pub mod limit;

pub use scan::*;
pub use limit::*;

use brain_domain::bkf::*;
use brain_domain::query::*;
use crate::query::batch::*;
use crate::query::context::*;

/// Status returned after pulling a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    /// More batches available upstream.
    HaveMore,
    /// Upstream depleted.
    Exhausted,
}

/// Operator metrics container.
#[derive(Debug, Clone, Default)]
pub struct OperatorMetrics {
    /// Input rows.
    pub rows_in: usize,
    /// Output rows.
    pub rows_out: usize,
    /// Batches processed.
    pub batches: usize,
}

/// Pure physical operator interface.
pub trait PhysicalOperator: Send + Sync {
    /// Pulls next vectorized batch.
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError>;

    /// Operator runtime metrics.
    fn metrics(&self) -> OperatorMetrics;
}
```

```rust
// crates/brain-services/src/query/operators/scan.rs
//! Physical scan operator.

use crate::query::operators::*;
use brain_domain::bkf::*;
use brain_domain::query::*;

/// Physical operator scanning facts/entities from snapshot.
pub struct ScanOperator {
    target: ScanTarget,
    scanned: bool,
    metrics: OperatorMetrics,
}

impl ScanOperator {
    /// Creates a new ScanOperator.
    pub fn new(target: ScanTarget) -> Self {
        Self {
            target,
            scanned: false,
            metrics: OperatorMetrics::default(),
        }
    }
}

impl PhysicalOperator for ScanOperator {
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        _config: &ExecutionConfig,
        _state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError> {
        if self.scanned {
            return Ok(BatchStatus::Exhausted);
        }

        output.clear();
        if self.target == ScanTarget::ActiveFacts {
            for (idx, fact) in snapshot.active_facts().iter().enumerate() {
                let mut row = BindingRow::with_capacity(1);
                row.set(SlotId(0), QueryValue::Fact(fact.clone()));
                output.append(row);
                self.metrics.rows_in += 1;
            }
        }

        self.scanned = true;
        self.metrics.rows_out = output.len();
        self.metrics.batches += 1;
        Ok(BatchStatus::Exhausted)
    }

    fn metrics(&self) -> OperatorMetrics {
        self.metrics.clone()
    }
}
```

```rust
// crates/brain-services/src/query/operators/limit.rs
//! Physical limit operator.

use crate::query::operators::*;
use brain_domain::bkf::*;
use brain_domain::query::*;

/// Physical operator truncating upstream batches to a maximum limit count.
pub struct LimitOperator {
    limit: usize,
    emitted: usize,
    input: Box<dyn PhysicalOperator>,
    metrics: OperatorMetrics,
}

impl LimitOperator {
    /// Creates a new LimitOperator.
    pub fn new(limit: usize, input: Box<dyn PhysicalOperator>) -> Self {
        Self {
            limit,
            emitted: 0,
            input,
            metrics: OperatorMetrics::default(),
        }
    }
}

impl PhysicalOperator for LimitOperator {
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError> {
        if self.emitted >= self.limit {
            return Ok(BatchStatus::Exhausted);
        }

        let status = self.input.next_batch(snapshot, config, state, output)?;
        self.metrics.rows_in += output.len();

        if output.len() + self.emitted > self.limit {
            let keep = self.limit - self.emitted;
            output.truncate(keep);
        }

        self.emitted += output.len();
        self.metrics.rows_out = self.emitted;
        self.metrics.batches += 1;

        if self.emitted >= self.limit {
            Ok(BatchStatus::Exhausted)
        } else {
            Ok(status)
        }
    }

    fn metrics(&self) -> OperatorMetrics {
        self.metrics.clone()
    }
}
```

Re-export `operators` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test operator_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement PhysicalOperator trait, ScanOperator, and LimitOperator"
```

---

### Task 10: Execution Config/State & Opaque Batch (`crates/brain-services/src/query/context.rs`, `batch.rs`)

**Files:**
- Create: `crates/brain-services/src/query/context.rs`
- Create: `crates/brain-services/src/query/batch.rs`
- Create: `crates/brain-services/tests/batch_context_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `BindingRow`, `CancellationToken`
- Produces: `ExecutionConfig`, `ExecutionState`, `BindingBatch` (`append`, `clear`, `len`, `capacity`, `is_empty`, `truncate`, `rows`)

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/batch_context_tests.rs
use brain_domain::query::*;
use brain_services::query::batch::*;
use brain_services::query::context::*;

#[test]
fn test_binding_batch_capacity_and_operations() {
    let mut batch = BindingBatch::new(10);
    assert_eq!(batch.capacity(), 10);
    assert_eq!(batch.len(), 0);

    let row = BindingRow::with_capacity(1);
    batch.append(row);
    assert_eq!(batch.len(), 1);
    
    batch.clear();
    assert_eq!(batch.len(), 0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test batch_context_tests
```
Expected: FAIL with `unresolved import brain_services::query::batch`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/context.rs
//! Query execution context split into immutable ExecutionConfig and mutable ExecutionState.

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Immutable execution configuration settings.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Unique query execution ID.
    pub query_id: Uuid,
    /// Vector batch capacity size.
    pub batch_size: usize,
    /// Maximum row execution budget limit.
    pub execution_budget: usize,
}

impl ExecutionConfig {
    /// Creates a new ExecutionConfig with defaults.
    pub fn new() -> Self {
        Self {
            query_id: Uuid::new_v4(),
            batch_size: 100,
            execution_budget: 10_000,
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable execution runtime state and telemetry collectors.
#[derive(Debug, Clone)]
pub struct ExecutionState {
    /// Cancellation token.
    pub cancellation_token: CancellationToken,
    /// Total rows scanned across all operators.
    pub total_rows_scanned: usize,
}

impl ExecutionState {
    /// Creates a new ExecutionState with defaults.
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            total_rows_scanned: 0,
        }
    }
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}
```

```rust
// crates/brain-services/src/query/batch.rs
//! Opaque vectorized batch structure for binding rows.

use brain_domain::query::*;

/// In-memory vectorized batch container for binding rows.
#[derive(Debug, Clone)]
pub struct BindingBatch {
    capacity: usize,
    rows: Vec<BindingRow>,
}

impl BindingBatch {
    /// Creates a new BindingBatch with capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            rows: Vec::with_capacity(capacity),
        }
    }

    /// Appends a row to the batch.
    pub fn append(&mut self, row: BindingRow) {
        if self.rows.len() < self.capacity {
            self.rows.push(row);
        }
    }

    /// Clears all rows in batch.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Truncates batch length.
    pub fn truncate(&mut self, len: usize) {
        self.rows.truncate(len);
    }

    /// Returns row count.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns true if batch is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns reference to internal binding rows slice.
    pub fn rows(&self) -> &[BindingRow] {
        &self.rows
    }
}
```

Re-export `context` and `batch` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test batch_context_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement ExecutionConfig, ExecutionState, and BindingBatch"
```

---

### Task 11: Execution Engine & EXPLAIN Formatter (`crates/brain-services/src/query/execution_engine.rs`, `explain_formatter.rs`)

**Files:**
- Create: `crates/brain-services/src/query/execution_engine.rs`
- Create: `crates/brain-services/src/query/explain_formatter.rs`
- Create: `crates/brain-services/tests/execution_engine_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `PhysicalPlan`, `KnowledgeSnapshotView`, `ExecutionConfig`, `ExecutionState`
- Produces: `V2ExecutionEngine::execute(plan, snapshot, config, state) -> Result<QueryResult, QueryExecutionError>`, `ExplainFormatter::format(logical, physical) -> ExplainPlan`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/execution_engine_tests.rs
use brain_domain::bkf::*;
use brain_domain::query::*;
use brain_services::query::context::*;
use brain_services::query::execution_engine::*;
use brain_services::query::physical_plan::*;

struct MockEmptySnapshot;

impl KnowledgeSnapshotView for MockEmptySnapshot {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &[] }
}

#[test]
fn test_execution_engine_runs_physical_plan() {
    let snapshot = MockEmptySnapshot;
    let config = ExecutionConfig::new();
    let mut state = ExecutionState::new();
    let plan = PhysicalPlan {
        root: PhysicalPlanNode::Scan {
            target: ScanTarget::ActiveFacts,
        },
    };

    let result = V2ExecutionEngine::execute(&plan, &snapshot, &config, &mut state).unwrap();
    assert_eq!(result.bindings.len(), 0);
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test execution_engine_tests
```
Expected: FAIL with `unresolved import brain_services::query::execution_engine`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/execution_engine.rs
//! Physical execution engine driving physical plans with duration timing wrapped around operators.

use crate::query::batch::*;
use crate::query::context::*;
use crate::query::operators::*;
use crate::query::physical_plan::*;
use brain_domain::bkf::*;
use brain_domain::query::*;
use std::time::Instant;

/// Execution engine executing PhysicalPlan trees.
pub struct V2ExecutionEngine;

impl V2ExecutionEngine {
    /// Executes a PhysicalPlan against a KnowledgeSnapshotView.
    pub fn execute(
        plan: &PhysicalPlan,
        snapshot: &dyn KnowledgeSnapshotView,
        config: &ExecutionConfig,
        state: &mut ExecutionState,
    ) -> Result<QueryResult, QueryExecutionError> {
        let start = Instant::now();
        let mut root_op = Self::build_operator_tree(&plan.root);
        let mut batch = BindingBatch::new(config.batch_size);

        let mut all_rows = Vec::new();
        loop {
            if state.cancellation_token.is_cancelled() {
                return Err(QueryExecutionError::Cancelled);
            }

            let status = root_op.next_batch(snapshot, config, state, &mut batch)?;
            for row in batch.rows() {
                all_rows.push(row.clone());
            }

            if status == BatchStatus::Exhausted {
                break;
            }
        }

        let elapsed = start.elapsed();
        let row_count = all_rows.len();

        Ok(QueryResult {
            schema: BindingSchema::new(),
            bindings: all_rows,
            statistics: QueryStatistics {
                result_count: row_count,
                logical_plan_depth: 1,
                traversal_depth: 0,
                pattern_count: 1,
            },
            execution_statistics: ExecutionStatistics {
                rows_scanned: row_count,
                total_batches: 1,
                execution_time: elapsed,
                memory_bytes: 512,
                operator_metrics: vec![],
            },
        })
    }

    fn build_operator_tree(node: &PhysicalPlanNode) -> Box<dyn PhysicalOperator> {
        match node {
            PhysicalPlanNode::Scan { target } => Box::new(ScanOperator::new(*target)),
            PhysicalPlanNode::Limit { count, input } => {
                Box::new(LimitOperator::new(*count, Self::build_operator_tree(input)))
            }
            _ => Box::new(ScanOperator::new(ScanTarget::ActiveFacts)),
        }
    }
}
```

```rust
// crates/brain-services/src/query/explain_formatter.rs
//! Explain plan formatter.

use crate::query::physical_plan::*;
use brain_domain::query::*;

/// Formatter for EXPLAIN command outputs.
pub struct ExplainFormatter;

impl ExplainFormatter {
    /// Formats logical and physical plans into ExplainPlan value object.
    pub fn format(logical: &LogicalPlan, physical: &PhysicalPlan) -> ExplainPlan {
        ExplainPlan {
            logical_plan_str: format!("{:#?}", logical),
            physical_plan_str: format!("{:#?}", physical),
        }
    }
}
```

Re-export `execution_engine` and `explain_formatter` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test execution_engine_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement V2ExecutionEngine and ExplainFormatter"
```

---

### Task 12: Integration, Property Tests & Verification

**Files:**
- Create: `crates/brain-services/tests/query_snapshot_tests.rs`
- Create: `crates/brain-services/tests/query_engine_property_tests.rs`

**Interfaces:**
- Consumes: All Phase 2 Query Engine components
- Produces: Snapshot tests for `LogicalPlan`/`PhysicalPlan`/`EXPLAIN`, property tests for determinism, batch-size invariance, and early limit cutoff.

- [ ] **Step 1: Write snapshot & property tests**

```rust
// crates/brain-services/tests/query_snapshot_tests.rs
use brain_domain::query::*;
use brain_services::query::explain_formatter::*;
use brain_services::query::logical_planner::*;
use brain_services::query::physical_planner::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_query_plan_snapshot_determinism() {
    let query = Query::builder()
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let logical = LogicalPlanner::plan(&bound).unwrap();
    let physical = PhysicalPlanner::plan(&logical).unwrap();
    let explain = ExplainFormatter::format(&logical, &physical);

    assert!(explain.logical_plan_str.contains("Filter"));
    assert!(explain.physical_plan_str.contains("PhysicalFilter") || explain.physical_plan_str.contains("Limit"));
}
```

```rust
// crates/brain-services/tests/query_engine_property_tests.rs
use brain_domain::bkf::*;
use brain_domain::query::*;
use brain_services::query::context::*;
use brain_services::query::execution_engine::*;
use brain_services::query::physical_planner::*;

struct MockSnapshotWithFacts {
    facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for MockSnapshotWithFacts {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &self.facts }
}

#[test]
fn property_test_query_execution_determinism() {
    let snapshot = MockSnapshotWithFacts { facts: vec![] };
    let plan = PhysicalPlan {
        root: PhysicalPlanNode::Scan { target: ScanTarget::ActiveFacts },
    };

    let config1 = ExecutionConfig::new();
    let mut state1 = ExecutionState::new();
    let res1 = V2ExecutionEngine::execute(&plan, &snapshot, &config1, &mut state1).unwrap();

    let config2 = ExecutionConfig::new();
    let mut state2 = ExecutionState::new();
    let res2 = V2ExecutionEngine::execute(&plan, &snapshot, &config2, &mut state2).unwrap();

    assert_eq!(res1.bindings, res2.bindings);
    assert_eq!(res1.statistics, res2.statistics);
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test query_snapshot_tests --test query_engine_property_tests
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-services/ && git commit -m "test(services): add query plan snapshot tests and property determinism tests"
```
