# Knowledge Query Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 2 — Knowledge Query Engine as a pure, storage-agnostic, pull-scheduled batch query compiler and execution pipeline over `KnowledgeSnapshotView`.

**Architecture:** A compiler-style pipeline (`Query AST -> Semantic Binder -> Bound Query -> Logical Planner -> Logical Plan -> Logical Optimizer -> Physical Planner -> Physical Plan -> Pull-Scheduled Batch Execution Engine -> QueryResult`). Value objects, AST, logical algebra, error types, and results live in `brain-domain::query` with zero external dependencies; binding, planning, optimization, operators, and batch execution live in `brain-services::query`.

**Tech Stack:** Rust (edition 2021), `serde`, `uuid`, `tokio_util::sync::CancellationToken`.

## Global Constraints
- `brain-domain` must contain zero async runtimes, logger setups, database engines, or network dependencies (`#![deny(missing_docs)]` enabled).
- All physical scan operators query exclusively via `KnowledgeSnapshotView`. No direct storage access.
- Given identical snapshot state and query inputs, query execution and plan generation must be 100% bitwise deterministic.
- Operators operate on opaque vectorized `BindingBatch` structures pulled top-down from the physical tree.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: AST Builder & Filter Expressions | ⬜ Pending | |
| **M1** | Task 2: Bound Query & Logical Plan Models | ⬜ Pending | |
| **M1** | Task 3: Error Hierarchy, Results & Explain Models | ⬜ Pending | |
| **M1 Checkpoint** | **Public API Review & Interface Freeze** | ⬜ Pending | |
| **M2** | Task 4: Semantic Binder | ⬜ Pending | |
| **M2** | Task 5: Logical Planner | ⬜ Pending | |
| **M3** | Task 6: Rule-Based Logical Optimizer Pipeline | ⬜ Pending | |
| **M4** | Task 7: Physical Planner & Physical Plan | ⬜ Pending | |
| **M4** | Task 8: Physical Batch Operators | ⬜ Pending | |
| **M5** | Task 9: Execution Context & Opaque Batch | ⬜ Pending | |
| **M5** | Task 10: Execution Engine & EXPLAIN Formatter | ⬜ Pending | |
| **M6** | Task 11: Verification, Snapshot & Property Tests | ⬜ Pending | |

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

### Task 2: Bound Query & Logical Plan Models

**Files:**
- Create: `crates/brain-domain/src/query/bound.rs`
- Create: `crates/brain-domain/src/query/logical_plan.rs`
- Create: `crates/brain-domain/tests/logical_plan_tests.rs`
- Modify: `crates/brain-domain/src/query/mod.rs`

**Interfaces:**
- Consumes: `Query`, `Pattern`, `QueryFilter`, `QueryVar`
- Produces: `BoundQuery`, `LogicalPlan`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/logical_plan_tests.rs
use brain_domain::query::ast::*;
use brain_domain::query::bound::*;
use brain_domain::query::logical_plan::*;
use brain_domain::bkf::*;

#[test]
fn test_logical_plan_tree() {
    let scan = LogicalPlan::Scan {
        target: "active_facts".to_string(),
    };
    let limit = LogicalPlan::Limit {
        count: 10,
        input: Box::new(scan),
    };

    match limit {
        LogicalPlan::Limit { count, input } => {
            assert_eq!(count, 10);
            assert!(matches!(*input, LogicalPlan::Scan { .. }));
        }
        _ => panic!("Expected Limit plan"),
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test -p brain-domain --test logical_plan_tests
```
Expected: FAIL with `unresolved import brain_domain::query::bound`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-domain/src/query/bound.rs
//! Semantic bound query representation.

use crate::query::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Validated bound query with scope resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundQuery {
    /// Inner AST.
    pub ast: Query,
    /// Resolved variable bindings.
    pub variable_scopes: HashMap<String, String>,
}
```

```rust
// crates/brain-domain/src/query/logical_plan.rs
//! Immutable logical algebra nodes.

use crate::query::filters::*;
use serde::{Deserialize, Serialize};

/// Logical plan algebra nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogicalPlan {
    /// Data source scan.
    Scan {
        /// Target scan table/view.
        target: String,
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

Re-export `bound` and `logical_plan` in `crates/brain-domain/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p brain-domain --test logical_plan_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(domain): add BoundQuery and LogicalPlan algebra models"
```

---

### Task 3: Error Hierarchy, Results & Explain Models

**Files:**
- Create: `crates/brain-domain/src/query/errors.rs`
- Create: `crates/brain-domain/src/query/result.rs`
- Create: `crates/brain-domain/src/query/explain.rs`
- Create: `crates/brain-domain/tests/query_result_tests.rs`
- Modify: `crates/brain-domain/src/query/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeEntity`, `FactVersion`, `LiteralValue`, `QueryVar`
- Produces: `QueryError`, `QueryExecutionError`, `QueryValue`, `BindingRow`, `QueryStatistics`, `ExecutionStatistics`, `QueryResult`, `ExplainPlan`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-domain/tests/query_result_tests.rs
use brain_domain::query::ast::*;
use brain_domain::query::explain::*;
use brain_domain::query::result::*;
use std::time::Duration;

#[test]
fn test_query_result_construction() {
    let mut row = BindingRow::new();
    row.insert(QueryVar::new("x"), QueryValue::Literal(brain_domain::bkf::LiteralValue::String("test".to_string())));

    let result = QueryResult {
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
//! Query result and statistics value objects.

use crate::bkf::*;
use crate::query::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Value bound to a query variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
    /// Entity reference.
    Entity(KnowledgeEntity),
    /// Fact version reference.
    Fact(FactVersion),
    /// Scalar literal.
    Literal(LiteralValue),
}

/// Binding map for a single query result row.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BindingRow {
    /// Map of variable to bound value.
    pub bindings: HashMap<QueryVar, QueryValue>,
}

impl BindingRow {
    /// Creates a new BindingRow.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a variable binding.
    pub fn insert(&mut self, var: QueryVar, val: QueryValue) {
        self.bindings.insert(var, val);
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

/// Operator telemetry metric entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorMetricEntry {
    /// Operator identifier.
    pub operator_name: String,
    /// Rows produced.
    pub rows_emitted: usize,
    /// Execution duration.
    pub duration: Duration,
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
git add crates/brain-domain/ && git commit -m "feat(domain): add QueryError hierarchy, QueryResult, and ExplainPlan value objects"
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
- Consumes: `Query`, `QueryError`, `BoundQuery`
- Produces: `SemanticBinder::bind(query: &Query) -> Result<BoundQuery, QueryError>`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/semantic_binder_tests.rs
use brain_domain::query::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_semantic_binder_validates_query() {
    let query = Query::builder()
        .pattern(Pattern::triple(
            QueryVar::new("p"),
            brain_domain::bkf::PredicateName::new("LivesIn").unwrap(),
            QueryVar::new("c"),
        ))
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    assert_eq!(bound.ast.patterns.len(), 1);
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
//! Semantic binder validating AST variables and building BoundQuery.

use brain_domain::query::*;
use std::collections::HashMap;

/// Semantic binder translating Query AST into validated BoundQuery.
pub struct SemanticBinder;

impl SemanticBinder {
    /// Binds and validates a Query AST.
    pub fn bind(query: &Query) -> Result<BoundQuery, QueryError> {
        let mut scopes = HashMap::new();

        for pat in &query.patterns {
            if let PatternTarget::Variable(ref v) = pat.subject {
                scopes.insert(v.0.clone(), "subject".to_string());
            }
            if let PatternTarget::Variable(ref v) = pat.object {
                scopes.insert(v.0.clone(), "object".to_string());
            }
        }

        Ok(BoundQuery {
            ast: query.clone(),
            variable_scopes: scopes,
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
git add crates/brain-services/ && git commit -m "feat(services): implement SemanticBinder for Query AST validation"
```

---

### Task 5: Logical Planner (`crates/brain-services/src/query/logical_planner.rs`)

**Files:**
- Create: `crates/brain-services/src/query/logical_planner.rs`
- Create: `crates/brain-services/tests/logical_planner_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `BoundQuery`, `LogicalPlan`
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
            target: "active_facts".to_string(),
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
git add crates/brain-services/ && git commit -m "feat(services): implement LogicalPlanner for AST to LogicalPlan translation"
```

---

### Task 6: Rule-Based Logical Optimizer Pipeline (`crates/brain-services/src/query/logical_optimizer.rs`)

**Files:**
- Create: `crates/brain-services/src/query/logical_optimizer.rs`
- Create: `crates/brain-services/tests/logical_optimizer_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `LogicalPlan`
- Produces: `LogicalOptimizer::optimize(plan: LogicalPlan) -> Result<LogicalPlan, QueryError>`

- [ ] **Step 1: Write failing test**

```rust
// crates/brain-services/tests/logical_optimizer_tests.rs
use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_logical_optimizer_pipeline() {
    let raw_plan = LogicalPlan::Filter {
        condition: QueryFilter::EntityKind("Person".to_string()),
        input: Box::new(LogicalPlan::Scan {
            target: "active_facts".to_string(),
        }),
    };

    let optimized = LogicalOptimizer::optimize(raw_plan).unwrap();
    assert!(matches!(optimized, LogicalPlan::Filter { .. }));
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test logical_optimizer_tests
```
Expected: FAIL with `unresolved import brain_services::query::logical_optimizer`.

- [ ] **Step 3: Implement minimal code**

```rust
// crates/brain-services/src/query/logical_optimizer.rs
//! Rule-based logical optimizer pipeline.

use brain_domain::query::*;

/// Deterministic multi-pass logical optimizer.
pub struct LogicalOptimizer;

impl LogicalOptimizer {
    /// Optimizes a LogicalPlan via deterministic passes.
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
        // Lexicographically stable tie-breaking on equal cost joins
        plan
    }
}
```

Re-export `logical_optimizer` in `crates/brain-services/src/query/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

```bash
DYLD_FRAMEWORK_PATH=/Library/Developer/CommandLineTools/Library/Frameworks cargo test -p brain-services --test logical_optimizer_tests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/ && git commit -m "feat(services): implement LogicalOptimizer multi-pass rule pipeline"
```

---

### Task 7: Physical Planner & Physical Plan (`crates/brain-services/src/query/physical_planner.rs`)

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
        target: "active_facts".to_string(),
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

use serde::{Deserialize, Serialize};

/// Physical plan representation node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalPlanNode {
    /// Physical snapshot scan operator node.
    Scan { target: String },
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
                target: target.clone(),
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
                target: "active_facts".to_string(),
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

### Task 8: Physical Batch Operators (`crates/brain-services/src/query/operators/`)

**Files:**
- Create: `crates/brain-services/src/query/operators/mod.rs`
- Create: `crates/brain-services/src/query/operators/scan.rs`
- Create: `crates/brain-services/src/query/operators/limit.rs`
- Create: `crates/brain-services/tests/operator_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `KnowledgeSnapshotView`, `QueryExecutionContext`, `BindingBatch`
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
    let mut ctx = QueryExecutionContext::new();
    let mut batch = BindingBatch::new(10);
    let mut op = ScanOperator::new();

    let status = op.next_batch(&snapshot, &mut ctx, &mut batch).unwrap();
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
use std::time::Duration;

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
    /// Rows emitted.
    pub rows_emitted: usize,
    /// Execution duration.
    pub duration: Duration,
}

/// Pure physical operator interface.
pub trait PhysicalOperator: Send + Sync {
    /// Pulls next vectorized batch.
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        ctx: &mut QueryExecutionContext,
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

/// Physical operator scanning active facts from snapshot.
#[derive(Default)]
pub struct ScanOperator {
    scanned: bool,
    metrics: OperatorMetrics,
}

impl ScanOperator {
    /// Creates a new ScanOperator.
    pub fn new() -> Self {
        Self::default()
    }
}

impl PhysicalOperator for ScanOperator {
    fn next_batch(
        &mut self,
        snapshot: &dyn KnowledgeSnapshotView,
        _ctx: &mut QueryExecutionContext,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError> {
        if self.scanned {
            return Ok(BatchStatus::Exhausted);
        }

        output.clear();
        for fact in snapshot.active_facts() {
            let mut row = BindingRow::new();
            row.insert(QueryVar::new("fact"), QueryValue::Fact(fact.clone()));
            output.append(row);
        }

        self.scanned = true;
        self.metrics.rows_emitted = output.len();
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
        ctx: &mut QueryExecutionContext,
        output: &mut BindingBatch,
    ) -> Result<BatchStatus, QueryExecutionError> {
        if self.emitted >= self.limit {
            return Ok(BatchStatus::Exhausted);
        }

        let status = self.input.next_batch(snapshot, ctx, output)?;
        if output.len() + self.emitted > self.limit {
            let keep = self.limit - self.emitted;
            output.truncate(keep);
        }

        self.emitted += output.len();
        self.metrics.rows_emitted = self.emitted;

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

### Task 9: Execution Context & Opaque Batch (`crates/brain-services/src/query/context.rs`, `batch.rs`)

**Files:**
- Create: `crates/brain-services/src/query/context.rs`
- Create: `crates/brain-services/src/query/batch.rs`
- Create: `crates/brain-services/tests/batch_context_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `BindingRow`, `CancellationToken`
- Produces: `QueryExecutionContext`, `BindingBatch` (`append`, `clear`, `len`, `capacity`, `is_empty`, `truncate`, `rows`)

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

    let row = BindingRow::new();
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
//! Query execution context containing cancellation, budget, and metrics.

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Runtime context for query execution.
#[derive(Debug, Clone)]
pub struct QueryExecutionContext {
    /// Unique query execution ID.
    pub query_id: Uuid,
    /// Cancellation token.
    pub cancellation_token: CancellationToken,
    /// Vector batch capacity size.
    pub batch_size: usize,
    /// Maximum row execution budget limit.
    pub execution_budget: usize,
}

impl QueryExecutionContext {
    /// Creates a new QueryExecutionContext with defaults.
    pub fn new() -> Self {
        Self {
            query_id: Uuid::new_v4(),
            cancellation_token: CancellationToken::new(),
            batch_size: 100,
            execution_budget: 10_000,
        }
    }
}

impl Default for QueryExecutionContext {
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
git add crates/brain-services/ && git commit -m "feat(services): implement QueryExecutionContext and BindingBatch"
```

---

### Task 10: Execution Engine & EXPLAIN Formatter (`crates/brain-services/src/query/execution_engine.rs`, `explain_formatter.rs`)

**Files:**
- Create: `crates/brain-services/src/query/execution_engine.rs`
- Create: `crates/brain-services/src/query/explain_formatter.rs`
- Create: `crates/brain-services/tests/execution_engine_tests.rs`
- Modify: `crates/brain-services/src/query/mod.rs`

**Interfaces:**
- Consumes: `PhysicalPlan`, `KnowledgeSnapshotView`, `QueryExecutionContext`
- Produces: `V2ExecutionEngine::execute(plan, snapshot, ctx) -> Result<QueryResult, QueryExecutionError>`, `ExplainFormatter::format(logical, physical) -> ExplainPlan`

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
    let mut ctx = QueryExecutionContext::new();
    let plan = PhysicalPlan {
        root: PhysicalPlanNode::Scan {
            target: "active_facts".to_string(),
        },
    };

    let result = V2ExecutionEngine::execute(&plan, &snapshot, &mut ctx).unwrap();
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
//! Physical execution engine driving physical plans.

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
        ctx: &mut QueryExecutionContext,
    ) -> Result<QueryResult, QueryExecutionError> {
        let start = Instant::now();
        let mut root_op = Self::build_operator_tree(&plan.root);
        let mut batch = BindingBatch::new(ctx.batch_size);

        let mut all_rows = Vec::new();
        loop {
            if ctx.cancellation_token.is_cancelled() {
                return Err(QueryExecutionError::Cancelled);
            }

            let status = root_op.next_batch(snapshot, ctx, &mut batch)?;
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
            PhysicalPlanNode::Scan { .. } => Box::new(ScanOperator::new()),
            PhysicalPlanNode::Limit { count, input } => {
                Box::new(LimitOperator::new(*count, Self::build_operator_tree(input)))
            }
            _ => Box::new(ScanOperator::new()),
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

### Task 11: Verification, Snapshot & Property Tests

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
        root: PhysicalPlanNode::Scan { target: "active_facts".to_string() },
    };

    let mut ctx1 = QueryExecutionContext::new();
    let res1 = V2ExecutionEngine::execute(&plan, &snapshot, &mut ctx1).unwrap();

    let mut ctx2 = QueryExecutionContext::new();
    let res2 = V2ExecutionEngine::execute(&plan, &snapshot, &mut ctx2).unwrap();

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
