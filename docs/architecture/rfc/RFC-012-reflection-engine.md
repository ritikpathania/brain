# RFC-012: Knowledge Graph Reflection Engine

**Author:** Antigravity  
**Date:** 2026-07-22  
**Status:** Proposed  
**Reference RFCs:** RFC-001 / RFC-007 / RFC-010 / RFC-011

---

## 1. Executive Summary

Today, the Brain Knowledge Graph behaves as a passive database: it ingests raw observations and retrieves nodes. To function as a true long-term memory, the system must actively consolidate and repair itself.

This RFC proposes the **Reflection Engine**—a read-only background consolidation pipeline that runs periodic "reflection passes" over an immutable snapshot of the knowledge graph. Following strict Command Query Responsibility Segregation (CQRS) and Domain-Driven Design (DDD) principles:
1. **Reflection is Read-Only**: Reflection passes analyze the graph snapshot to produce **Findings** representing potential optimization opportunities (e.g., duplicates, contradictions, missing relationships).
2. **Decoupled Resolution**: A **Reflection Planner** consumes these findings and generates **Domain Commands** representing the intent to modify the graph.
3. **Immutable Auditing**: Applying a command inside a write transaction mutates storage and publishes **Domain Events** capturing the immutable facts of the consolidation.

---

## 2. Affected Subsystems

- **`crates/brain-domain`**:
  - Add domain entities/aggregates: `ReflectionFinding`, `FindingEvidence`, `ReflectionContext`, `ReflectionPlan`, and reflection commands/events.
  - Define domain commands: `MergeConceptsCommand`, `ResolveContradictionCommand`, `CreateInferredRelationCommand`.
  - Define domain events: `ConceptMerged`, `ContradictionResolved`, `RelationInferred`.
- **`crates/brain-services`**:
  - Implement the `ReflectionRegistry` module to manage registered passes.
  - Implement the `ReflectionEngine` orchestrator, `ReflectionPass` trait, and default passes.
  - Implement the `ReflectionPlanner` to translate findings to commands based on confidence thresholds and rules.
  - Implement the background worker daemon scheduling triggers.
- **`crates/brain-storage`**:
  - Add database schema to persist reflection findings (for queueing user review) and job metadata.

---

## 3. Proposal Details

### A. The Reflection & Planning Pipeline

```
          Knowledge Graph
                 │
                 ▼
        Immutable Snapshot
                 │
                 ▼
       ┌──────────────────┐
       │ ReflectionEngine │ ◄── Read-Only context running passes from ReflectionRegistry
       │  (Read-Only)     │
       └─────────┬────────┘
                 │
                 ▼ (Produces)
        Reflection Findings (DuplicateFound, ContradictionFound, LinkSuggested)
                 │
                 ▼
       ┌──────────────────┐
       │ReflectionPlanner │ ◄── Evaluates thresholds & constructs ReflectionPlan
       │  (Decision)      │
       └─────────┬────────┘
                 │
                 ▼ (Generates)
          Domain Commands (MergeConceptsCommand, CreateInferredRelationCommand)
                 │
                 ▼
       ┌──────────────────┐
       │   Transaction    │ ◄── Executes write commands & persists
       │   (Write Path)   │
       └─────────┬────────┘
                 │
                 ▼ (Emits)
           Domain Events (ConceptMerged, RelationInferred)
```

### B. The ReflectionPass & Finding Abstractions

Reflection passes run on an immutable snapshot and do not have write access. The execution order and active pass lists are explicitly managed by a `ReflectionRegistry`.

```rust
pub trait ReflectionPass: Send + Sync {
    /// Evaluates the graph snapshot and returns identified findings.
    fn run(
        &self,
        snapshot: &dyn RepositorySet,
        context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError>;
}

/// Explicit registry governing the registration and execution sequence of passes.
pub struct ReflectionRegistry {
    passes: Vec<Box<dyn ReflectionPass>>,
}

impl ReflectionRegistry {
    /// Registers a pass inside the registry.
    pub fn register(&mut self, pass: Box<dyn ReflectionPass>) {
        self.passes.push(pass);
    }

    /// Returns a slice of the registered passes.
    pub fn passes(&self) -> &[Box<dyn ReflectionPass>] {
        &self.passes
    }
}
```

Findings separate the issue description from structured diagnostic evidence:

```rust
/// Structured evidence backing up a reflection finding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FindingEvidence {
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f64,
    /// Cosine similarity from semantic embedding comparisons, if applicable.
    pub semantic_similarity: Option<f64>,
    /// Edit distance (e.g. Levenshtein) from name/label matching, if applicable.
    pub edit_distance: Option<usize>,
    /// Quantified ratio of shared references or overlaps, if applicable.
    pub overlap_ratio: Option<f64>,
    /// Narrative description of why the finding was raised.
    pub details: String,
}

pub enum ReflectionFinding {
    DuplicateFound {
        node_a: NodeId,
        node_b: NodeId,
        evidence: FindingEvidence,
    },
    ContradictionFound {
        node_id: NodeId,
        property_key: String,
        values: Vec<serde_json::Value>,
        evidence: FindingEvidence,
    },
    LinkSuggested {
        source_id: NodeId,
        target_id: NodeId,
        relation_kind: RelationKind,
        evidence: FindingEvidence,
    },
}
```

### C. Trigger Conditions & Rollout Phasing

To manage deployment complexity, scheduling triggers are rolled out incrementally:
- **Phase 1: Manual Trigger**: Executed on-demand by an administrator via explicit CLI command (`brain reflect`).
- **Phase 2: Observation Threshold**: Triggered when the number of new observations ingested since the last run exceeds $N$ (default: `50`).
- **Phase 3: Idle Timer**: Triggered if no workspace queries occur for an idle window (default: `10 minutes`).
- **Phase 4: Nightly Maintenance**: Triggered automatically during low-traffic maintenance periods.

---

## 4. Contract Changes (Interfaces & Types)

### Rust Crate Signature Changes

#### [`crates/brain-domain/src/reflection/mod.rs`](../../../crates/brain-domain/src/reflection/mod.rs)

```rust
/// Execution constraints and identifiers for a reflection run.
#[derive(Debug, Clone)]
pub struct ReflectionContext {
    /// Unique execution task ID.
    pub execution_id: uuid::Uuid,
    /// Active Session ID target for consolidation.
    pub session_id: SessionId,
    /// Cutoff epoch representing the historical window snapshot.
    pub cutoff_epoch: u64,
    /// Maximum number of nodes to load into memory for analysis.
    pub max_nodes: usize,
    /// Time budget in milliseconds.
    pub time_budget_ms: u64,
    /// Cancellation channel indicator for daemon shutdown signals.
    pub cancellation_token: tokio_util::sync::CancellationToken,
}

/// An aggregated decision plan produced by the planner.
pub struct ReflectionPlan {
    /// Commands resolved and queued for transaction execution.
    pub commands: Vec<ReflectionDomainCommand>,
    /// Count of total findings evaluated.
    pub findings_processed: usize,
    /// Logs of findings that were skipped (e.g. low confidence).
    pub skipped_findings: Vec<(ReflectionFinding, String)>,
}

/// Commands describing intent to mutate the graph after reflection planning.
pub enum ReflectionDomainCommand {
    /// Merge duplicate nodes into a canonical model.
    MergeConcepts {
        canonical_id: NodeId,
        duplicate_id: NodeId,
    },
    /// Create an inferred transitive relationship.
    CreateInferredRelation {
        source_id: NodeId,
        target_id: NodeId,
        relation_kind: RelationKind,
        confidence: f64,
    },
}

/// Events documenting facts of completed reflection modifications.
pub enum ReflectionDomainEvent {
    ConceptMerged {
        canonical_id: NodeId,
        merged_id: NodeId,
        provenance: String,
    },
    RelationInferred {
        source_id: NodeId,
        target_id: NodeId,
        relation_kind: RelationKind,
    },
}
```

---

## 5. Backward Compatibility Plan

Since reflection is completely decoupled from active ingestion, older databases are compatible. All mutations requested by the planner are routed through standard commands, ensuring that triggers, constraints, and audit logs are consistently written.

---

## 6. Verification & Testing Plan

### Automated Tests
1. **Read-Only Purity**: Verify that `ReflectionPass::run` does not execute queries that mutate the sqlite database.
2. **Planner Determinism**: Verify that running the planner on the same set of findings yields identical `ReflectionPlan` structures.
3. **Command Idempotency**: Verify that executing the same command twice does not create duplicate entries or cause failures.
4. **No Mutations on Empty Plan**: Verify that if the planner generates zero commands, the write transaction is bypassed and no database writes occur.
5. **Provenance Integrity**: Verify that merging concepts concatenates source conversation identifiers monotonically.
6. **Budget Constraints**: Verify passes terminate gracefully on cancellation token signals.
7. **Pass Isolation & Atomicity**: A failure or panic inside one pass must not corrupt intermediate states. The entire cycle rolls back gracefully (transaction atomicity), reporting the execution error logs clearly.
