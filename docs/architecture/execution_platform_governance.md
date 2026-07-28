# Execution Platform Governance & Architecture Policy

## Executive Summary

Milestones **R23 through R29** constitute `brain`'s **Stable Execution Platform**. This document formalizes the architectural governance policy for these layers, guaranteeing long-term contract stability, strict dependency direction, and performance preservation as higher-level product features (Workflow Engine, DAG Orchestrator, Cluster Operations) are built on top.

---

## 1. System Layering Hierarchy

```text
                    Product Layer
──────────────────────────────────────────────────────
 Workflow Engine
 DAG Orchestration
 Cluster Management & Operations
 Scheduling Policies & Quotas
 Telemetry & Observability
 Public APIs & SDKs
──────────────────────────────────────────────────────
         Stable Execution Platform (R23–R29)
──────────────────────────────────────────────────────
 R29  Consensus (OpenRaft, ReplicatedIntent, LeaderLeaseManager)
 R28  Intent Log & Replay (IntentLog WAL, Materializer, ReplayEngine)
 R27  Coordinator Orchestration (CoordinatorState, QueueManager, SchedulingEngine)
 R26  Worker Runtime (TaskExecutor, ArtifactStore, Decorators)
 R25  Distributed Runtime (WorkerRegistry, WorkerTransport, SchedulingPolicy)
 R23/R24 Durable Execution (WAL Journal, ExecutionRepository, RecoveryEngine)
──────────────────────────────────────────────────────
            Domain Model (crates/brain-domain)
```

---

## 2. Core Architectural Governance Rules

### Rule 1: Platform Stability Rule
The Execution Substrate (R23–R29) evolves **only** for:
- Correctness & bug fixes
- Security vulnerability remediation
- Performance optimizations
- Operational robustness & telemetry improvements

It MUST NOT be modified to accommodate ad-hoc product layer features.

### Rule 2: Strict Dependency Direction
Every future capability MUST depend on the Execution Platform; the Execution Platform MUST NEVER depend on higher-level product layers or DSLs.

```text
CORRECT:   Workflow Engine ──► CoordinatorRuntime ──► IntentLog ──► Consensus
INCORRECT: CoordinatorRuntime ──► WorkflowDSL
```

### Rule 3: Frozen Contract Invariant
The following core contracts and trait boundaries are effectively **frozen**:
- `ExecutionId`, `TaskId`, `SequenceNumber`, `EventId`, `EffectId`
- `CoordinatorState` root aggregate
- `trait IntentLog`, `trait CoordinatorEffectExecutor`, `trait TaskExecutor`
- `SchedulingPolicy`, `WorkerDescriptor`, `WorkerStatus`, `TaskAssignment`

If future capabilities require new information, extensions MUST be implemented via additive payload fields or adapter wrappers rather than altering frozen traits.

---

## 3. Governance Policies & Change Procedures

1. **Backward Compatibility by Default**: Any proposed change to frozen substrate contracts requires documented justification proving that additive extension is insufficient.
2. **Architecture Decision Record (ADR)**: Any modification touching coordinator semantics, replay policies, intent logging, or consensus rules MUST be preceded by an approved ADR in `docs/architecture/adr/`.
3. **Performance Budgets**: Memory footprint, frame draw/lock latencies, and transaction throughput of the substrate are versioned qualities. CI regression checks validate these budgets.
4. **Compatibility & Replay Verification**: All substrate changes MUST pass the full automated integration test suite (`r23` through `r29` tests), verifying zero replay regressions or consensus split-brain edge cases.
