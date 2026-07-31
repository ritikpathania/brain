# Milestone R29 — Raft & Multi-Coordinator Consensus Architecture Specification

## Executive Summary

Milestone **R29 (Raft & Multi-Coordinator Consensus)** integrates distributed consensus and multi-coordinator cluster management for `brain`. Building directly on Milestone R28's [`IntentLog`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/superpowers/specs/2026-07-28-r28-ha-foundations-design.md#3-intent-log-storage-trait--persistence) WAL interface and [`CoordinatorEffectExecutor`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/superpowers/specs/2026-07-28-r28-ha-foundations-design.md#4-effect-executor--side-effect-routing), R29 introduces OpenRaft cluster replication (`RaftIntentLog`), leader election, follower state machine replication, leader lease fencing (`LeaderLeaseManager`), and post-commit side-effect execution.

---

## 1. Architecture & Four-Stage Commitment Lifecycle

The **Consensus Pipeline** isolates decision durability within the Raft cluster state machine. Every `ReplicatedIntent` entry progresses through 4 strict lifecycle stages:

```text
                        CoordinatorState Decision
                                   │
                    CoordinatorDecisionMaterializer
                                   │
                   Stage 1: Replicated (Raft Quorum)
                                   │
                   Stage 2: Committed (Commit Index)
                                   │
               Stage 3: Applied (All Cluster Nodes)
                                   │
              ┌────────────────────┴────────────────────┐
              ▼                                         ▼
   Active Leader Node (Term N)                  Follower Nodes
              │                                         │
 4. Validate Leader Lease (Dual Guard)             STOP (Apply State)
              │                                (No Side Effects)
 5. Leader Execute (EffectExecutor)
```

### Central Consensus Invariants
> **1. The Raft commit index is the sole authority that permits externally observable effects. A `CoordinatorEffect` MUST NOT be executed until the corresponding `ReplicatedIntent` is committed by the active Raft leader.**  
> **2. Every committed `ReplicatedIntent` MUST be applied to the replicated coordinator state on every node in identical sequence order before any leader is permitted to execute its associated `CoordinatorEffect`.**  
> **3. Applied means the replicated intent has been incorporated into the replicated coordinator state machine on that node. It does NOT imply side-effect execution.**  
> **4. Followers MUST apply committed intents to their local state machine, but MUST NEVER execute side-effects while in Follower state.**  
> **5. Loss of leader lease immediately prevents execution of any additional effects, even if those effects are already committed.**

---

## 2. Replicated Intent Schema vs Local Execution Tracker

Consensus replicates immutable intent payloads across the cluster; local execution progress (`LocalExecutionState`) is maintained node-locally:

```rust
use crate::ha::models::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicatedIntent {
    pub sequence: SequenceNumber,
    pub event_id: EventId,
    pub effect_id: EffectId,
    pub created_at: u64,
    pub effect: CoordinatorEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalExecutionState {
    Committed,
    Executing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadershipEvent {
    BecameLeader { term: u64 },
    BecameFollower { term: u64 },
}
```

### Leadership Event Semantics
- **`BecameLeader`**: Triggers intent replay and enables post-commit effect execution.
- **`BecameFollower`**: Immediately disables new effect execution across all threads.

---

## 3. `RaftIntentLog` & OpenRaft Storage Integration

`RaftIntentLog` implements `trait IntentLog` by wrapping an OpenRaft consensus instance:

```rust
use async_trait::async_trait;
use crate::ha::intent_log::*;
use crate::ha::models::*;
use openraft::Raft;

pub struct RaftTypeConfig; // Configures OpenRaft NodeId, LogEntry, Response

pub struct RaftIntentLog {
    raft: Raft<RaftTypeConfig>,
}

#[async_trait]
impl IntentLog for RaftIntentLog {
    async fn append_record(&self, record: &IntentRecord) -> Result<(), IntentLogError> {
        // Appends ReplicatedIntent to Raft log and waits for quorum commit
        Ok(())
    }

    async fn update_status(&self, effect_id: EffectId, status: IntentStatus) -> Result<(), IntentLogError> {
        // Updates ONLY the LocalExecutionTracker and NEVER modifies replicated log entries.
        Ok(())
    }

    async fn load_from(&self, sequence: SequenceNumber) -> Result<Vec<IntentRecord>, IntentLogError> {
        // Reads committed Raft log entries starting from sequence
        Ok(vec![])
    }

    async fn scan_pending(&self) -> Result<Vec<IntentRecord>, IntentLogError> {
        // Scans committed Raft log entries that are not marked Completed in local tracker
        Ok(vec![])
    }
}
```

---

## 4. Leader Lease Fencing & Dual Guard Validation

Side-effect execution requires dual verification by the `LeaderLeaseManager` and the `CommitNotifier`:

```rust
pub struct LeaderLeaseManager {
    current_term: u64,
    is_leader: bool,
    lease_expires_at: u64,
}

impl LeaderLeaseManager {
    pub fn is_leader(&self, now: u64) -> bool {
        self.is_leader && now < self.lease_expires_at
    }

    pub fn handle_event(&mut self, event: LeadershipEvent, now: u64, duration_secs: u64) {
        match event {
            LeadershipEvent::BecameLeader { term } => {
                self.current_term = term;
                self.is_leader = true;
                self.lease_expires_at = now + duration_secs;
            }
            LeadershipEvent::BecameFollower { term } => {
                self.current_term = term;
                self.is_leader = false;
                self.lease_expires_at = 0;
            }
        }
    }
}
```

---

## 5. Failover Recovery & Replay Engine

When a leader steps down or crashes, the newly elected Raft leader processes recovery without mutating consensus logs:

```text
LeadershipEvent::BecameLeader ──► Validate Leader Lease ──► Replay Engine Scans Committed Log
                                                                       │
                 ┌─────────────────────────────────────────────────────┼─────────────────────────────────────────────────────┐
                 ▼                                                     ▼                                                     ▼
    LocalExecutionState::Completed                 LocalExecutionState::Executing                    LocalExecutionState::Failed
                 │                                                     │                                                     │
                 ▼                                                     ▼                                                     ▼
           Skip Record                                     Re-execute Safely                                    Pass to Retry Policy
       (Already Executed)                               (Idempotent via EffectId)                            (Retry Policy Engine)
```

---

## Out of Scope (Deferred to Future Milestones)
- Dynamic cluster membership changes
- Joint consensus transitions
- Live node addition/removal APIs

---

## Verification & Test Plan

1. **Consensus Unit Tests (`crates/brain-services/src/ha/consensus/`)**:
   - `RaftIntentLog` commit lifecycle tests.
   - `LeaderLeaseManager` dual-guard and lease expiration tests.
   - `LeadershipEvent` state transition tests.
2. **Cluster Integration Tests (`crates/brain-services/tests/r29_raft_consensus_tests.rs`)**:
   - **Scenario 1**: Leader election with committed unexecuted intents — verify new leader replays and executes unexecuted effects.
   - **Scenario 2**: Leader step-down during effect execution — verify lease loss immediately stops effect execution.
   - **Scenario 3**: Follower promotion — verify follower applies state machine commands but executes zero side effects.
   - **Scenario 4**: Duplicate replay validation — verify `EffectId` idempotency prevents duplicate RPC dispatches.
   - **Scenario 5**: Network partition — verify partitioned leader steps down and aborts effect dispatches.
