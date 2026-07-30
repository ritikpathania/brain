# Design Specification: Reflection Engine v2 (Phase 0 & Sub-Project 1)

**Author:** Antigravity AI & System Architecture Team  
**Date:** 2026-07-30  
**Status:** Approved / Design Complete  
**Scope:** `crates/brain-domain` (Core Models), `crates/brain-services` (Reflection Engine & Passes)

---

## 1. Executive Summary & Goals

Reflection Engine v2 transforms Brain from an event-reactive storage engine into a self-maintaining reasoning engine. 

Instead of mutating existing graph records in-place, Reflection Engine v2 relies on **Immutable Fact Versioning**, **Domain Value Objects**, and **Pure Observation Passes**. Reflection passes inspect read-only snapshots of the knowledge graph and propose declarative `RewritePlan`s. These plans are validated by a `RewriteValidator` and transactionally committed to the event log as immutable `FactEvent`s.

---

## 2. Core Invariants & Architectural Rules

1. **Strict Event-Sourced Immutability**: Reflection **never** edits historical fact versions or mutates database rows in place. Every change creates a new `FactVersion` that supersedes prior versions.
2. **Observational Pass Isolation**: Reflection passes are strictly observational. Passes inspect snapshots, perform analysis, and emit `RewritePlan` intent. Passes **must never** execute SQL, mutate storage, or emit events directly.
3. **Single-Sided Supercedence**: Lineage links exist single-sided via `supersedes: Option<FactVersionId>`. Reverse lineage (`superseded_by`) is dynamically derived during query projection to prevent double-sided state drift.
4. **Derivable Lifecycle States**: `Historical` state is derived from non-null `valid_to` or active `superseded` links, avoiding duplicate mutable flags.
5. **Atomic Event Log Verification**: The event store WAL is the single source of truth. Read model projections are updated only after events are transactionally committed to the event log.

---

## 3. Knowledge Lifecycle & Temporal Model

### Fact Lifecycle

```
Candidate  ──────►  Verified  ──────►  Archived
   │                   │
   └──────► (Superseded/Valid Window Closed) ──────► Historical (Derived)
```

* **Candidate**: Unverified fact extracted from raw observation.
* **Verified**: Confirmed by reflection critique passes or manual user validation.
* **Historical (Derived)**: Automatic status when `valid_to != None` or superseded by a newer `FactVersion`.
* **Archived**: Storage policy status for cold storage pruning. Physical deletion is reserved strictly for hard privacy/GDPR compliance.

### Temporal Boundaries

Every `FactVersion` carries a `TemporalWindow` enforcing the following invariants:

$$\text{asserted\_at} \le \text{observed\_at}$$
$$\text{valid\_from} \le \text{valid\_to} \quad (\text{if } \text{valid\_to} \text{ is present})$$

* **asserted_at**: Timestamp when the claim was originally stated.
* **observed_at**: Timestamp when Brain ingested/learned the claim.
* **valid_from**: Timestamp when the claim became true in reality.
* **valid_to**: Optional timestamp when the claim stopped being true (`None` = active).

---

## 4. Domain Architecture (`crates/brain-domain`)

```text
Entity (Identity)
    │
    ▼
SemanticAssertion (Subject + Predicate + Object)
    │
    ▼
FactVersion (TemporalWindow + Confidence + Provenance + Supersedes)
```

### 4.1 Value Objects & Entities

```rust
pub struct EntityId(pub Uuid);
pub struct AssertionId(pub Uuid);
pub struct FactVersionId(pub Uuid);
pub struct PredicateId(pub Uuid);
pub struct ReflectionPassId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub SystemTime);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f32); // Strictly 0.0 <= f32 <= 1.0

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityName(String); // Validated & normalized casing

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PredicateName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PredicateCardinality {
    Exclusive,   // Max 1 active value (e.g., LivesIn)
    MultiValued, // Multiple active values allowed (e.g., Knows)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Predicate {
    pub id: PredicateId,
    pub name: PredicateName,
    pub cardinality: PredicateCardinality,
    pub is_temporal: bool,
    pub inverse: Option<PredicateId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: EntityName,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiteralValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Timestamp(Timestamp),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssertionTarget {
    Entity(EntityId),
    Value(LiteralValue),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticAssertion {
    pub id: AssertionId,
    pub kind: AssertionKind,
    pub subject: EntityId,
    pub predicate: PredicateId,
    pub object: AssertionTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: ProvenanceSource,
    pub derived_from: Vec<FactVersionId>, // Lineage DAG
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactVersion {
    pub id: FactVersionId,
    pub assertion_id: AssertionId,
    pub lifecycle: FactLifecycle,
    pub confidence: Confidence,
    pub temporal: TemporalWindow,
    pub supersedes: Option<FactVersionId>, // Single-sided predecessor link
    pub provenance: Provenance,
}
```

### 4.2 Snapshot Trait & Declarative Intent

```rust
pub trait KnowledgeSnapshotView: Send + Sync {
    fn entities(&self) -> &[Entity];
    fn assertions(&self) -> &[SemanticAssertion];
    fn predicates(&self) -> &[Predicate];
    fn active_facts(&self) -> &[FactVersion];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewriteReason {
    Contradiction,
    Duplicate,
    ConfidenceIncrease,
    ConfidenceDecrease,
    TemporalExpiration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewritePlan {
    pub pass_id: ReflectionPassId,
    pub reason: RewriteReason,
    pub rationale: String,
    pub execution_cost: u32,
    pub operations: Vec<RewriteOperation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RewriteOperation {
    RecordFact(FactVersion),
    SupersedeFact {
        old_fact_id: FactVersionId,
        new_fact_id: FactVersionId,
        closed_at: Timestamp,
    },
    MergeFacts {
        source_fact_ids: Vec<FactVersionId>,
        target_fact_id: FactVersionId,
    },
    ArchiveFact {
        fact_id: FactVersionId,
        archived_at: Timestamp,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FactEvent {
    FactRecorded {
        fact: FactVersion,
    },
    FactSuperseded {
        old_fact_id: FactVersionId,
        new_fact_id: FactVersionId,
        superseded_at: Timestamp,
    },
    FactArchived {
        fact_id: FactVersionId,
        archived_at: Timestamp,
    },
}
```

---

## 5. Reflection Engine & Service Pipeline (`crates/brain-services::reflection`)

```text
               Knowledge Snapshot
                       │
                       ▼
            Reflection Engine (DAG)
                       │
  ┌────────────────────┼────────────────────┐
  ▼                    ▼                    ▼
Canonicalization   Contradictions      Duplicates
  │                    │                    │
  └────────────────────┼────────────────────┘
                       ▼
                Confidence Pass
                       │
                       ▼
            Rewrite Plans (Intent)
                       │
                       ▼
            Rewrite Validation (Rules)
                       │
                       ▼
           Fact Events (Immutable Log)
                       │
                       ▼
         Event Store (Source of Truth)
                       │
                       ▼
                Read Model Updates
```

### 5.1 Reflection Pass Trait & Execution Pipeline

```rust
pub enum DiagnosticSeverity { Info, Warning, Error }

pub struct PassDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

pub struct ReflectionOutcome {
    pub plan: RewritePlan,
    pub diagnostics: Vec<PassDiagnostic>,
    pub execution_time_ms: u64,
}

pub trait ReflectionPass: Send + Sync {
    fn id(&self) -> ReflectionPassId;
    fn dependencies(&self) -> &[ReflectionPassId];
    fn analyze(&self, snapshot: &dyn KnowledgeSnapshotView) -> Result<Option<ReflectionOutcome>, BrainServiceError>;
}
```

### 5.2 Pass Inventory & Dependencies

1. **`CanonicalizationPass`** (Dependencies: `[]`)
   - Normalizes text names, trims whitespace, standardizes casing and aliases before semantic processing.
2. **`ContradictionPass`** (Dependencies: `["canonicalization"]`)
   - Identifies active facts linked to `PredicateCardinality::Exclusive` predicates. Closes the temporal window (`valid_to = new.valid_from`) of older facts when a newer conflicting fact is asserted.
3. **`DuplicateConsolidationPass`** (Dependencies: `["canonicalization"]`)
   - Detects redundant assertions/entities and generates `RewriteOperation::MergeFacts` pointing to canonical facts.
4. **`StaleKnowledgePass`** (Dependencies: `["contradiction"]`)
   - Detects expired temporal bounds and marks obsolete facts for archiving via `ArchiveFact`.
5. **`ConfidenceRecalculationPass`** (Dependencies: `["duplicate_consolidation", "contradiction"]`)
   - Recalculates confidence based on lineage DAG corroboration across multiple independent sources.

### 5.3 Transactional Executor & Validation Pipeline

```rust
pub struct RewriteValidator;

impl RewriteValidator {
    pub fn validate(plan: &RewritePlan, snapshot: &dyn KnowledgeSnapshotView) -> Result<(), BrainServiceError> {
        // Enforce temporal ordering, valid predecessor IDs, acyclic lineage
    }
}

pub struct RewriteExecutor {
    storage: Arc<dyn Storage>,
}

impl RewriteExecutor {
    pub fn execute(&self, plan: &RewritePlan, snapshot: &dyn KnowledgeSnapshotView) -> Result<Vec<FactEvent>, BrainServiceError> {
        RewriteValidator::validate(plan, snapshot)?;
        
        self.storage.run_transaction(&mut |tx| {
            let mut events = Vec::new();
            for op in &plan.operations {
                let event = self.lower_op_to_event(op)?;
                tx.append_event(&event)?;
                events.push(event);
            }
            tx.update_read_models(&events)?;
            Ok(events)
        })
    }
}
```

---

## 6. Verification & Testing Strategy

1. **Unit Tests**:
   - `Confidence::new` boundary checks ([0.0, 1.0]).
   - `TemporalWindow::new` invariant validation (`asserted_at <= observed_at`, `valid_from <= valid_to`).
   - Single-sided supercedence derivation.
2. **Pass Integration Tests**:
   - `ContradictionPass` test: Asserting `John LivesIn Delhi` then `John LivesIn Mumbai` closes Delhi's validity window and links supercedence.
   - `DuplicateConsolidationPass` test: Merging two duplicate assertions generates a single `MergeFacts` operation.
3. **Replay & Determinism Verification**:
   - Replaying identical `FactEvent` sequences from sequence 0 yields identical snapshot state.
   - Non-mutation invariant: Verify no direct SQL UPDATE statements exist for historical fact rows.
