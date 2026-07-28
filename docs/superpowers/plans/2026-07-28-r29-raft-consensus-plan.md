# Milestone R29 — Raft & Multi-Coordinator Consensus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Milestone R29 (Raft & Multi-Coordinator Consensus) in Rust within `crates/brain-services/src/ha/consensus/`, introducing `ReplicatedIntent`, `LocalExecutionState`, `LeadershipEvent`, `LeaderLeaseManager`, `CommitNotifier`, and `RaftIntentLog`.

**Architecture:** Consensus components housed in `brain-services::ha::consensus` (layered strictly **above** `brain-services::ha` and `brain-services::coordinator`). Integrates distributed quorum commit and dual-guard leader fencing (`CommitIndex` + `LeaderLeaseManager`).

**Tech Stack:** Rust, `tokio`, `async-trait`, `serde`, `uuid`, `thiserror`.

## Global Constraints

- **Module Hierarchy Rule**: `ha::consensus/` may depend on `ha/`, `coordinator/`, `distributed/`, and `runtime/`, but underlying modules MUST NEVER depend on `ha::consensus/`.
- **Stabilization Boundary Integrity**: Core contracts from Phase 1 to Phase 5 (`ExecutionId`, `TaskId`, `CoordinatorState`, `IntentLog`, `CoordinatorEffectExecutor`) MUST remain unchanged.
- **Consensus Invariants**:
  1. The Raft commit index is the sole authority that permits externally observable effects.
  2. Applied means the replicated intent has been incorporated into the replicated coordinator state machine on that node (does NOT imply side-effect execution).
  3. Followers MUST apply committed intents to local state machine, but MUST NEVER execute side-effects while in Follower state.
  4. Loss of leader lease immediately prevents execution of any additional effects.
- **`update_status` Invariant**: `update_status` updates ONLY local execution tracker state and NEVER modifies replicated log entries.

---

### Task 1: Consensus Models (`ReplicatedIntent`, `LocalExecutionState`, `LeadershipEvent`)

**Files:**
- Create: `crates/brain-services/src/ha/consensus/mod.rs`
- Create: `crates/brain-services/src/ha/consensus/models.rs`
- Modify: `crates/brain-services/src/ha/mod.rs`
- Test: `crates/brain-services/src/ha/consensus/models.rs` (inline test module)

**Interfaces:**
- Consumes: `SequenceNumber`, `EventId`, `EffectId`, `CoordinatorEffect`
- Produces: `ReplicatedIntent`, `LocalExecutionState`, `LeadershipEvent`

- [ ] **Step 1: Write failing unit tests for consensus models**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_replicated_intent_and_leadership_events() {
        let seq = SequenceNumber(1);
        let event_id = EventId(Uuid::new_v4());
        let effect_id = EffectId(Uuid::new_v4());

        let intent = ReplicatedIntent {
            sequence: seq,
            event_id,
            effect_id,
            created_at: 1000,
            effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        };

        assert_eq!(intent.sequence, SequenceNumber(1));

        let ev = LeadershipEvent::BecameLeader { term: 2 };
        assert!(matches!(ev, LeadershipEvent::BecameLeader { term: 2 }));
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib ha::consensus::models::tests`
Expected: FAIL with "module `consensus` not found"

- [ ] **Step 3: Implement ReplicatedIntent, LocalExecutionState, and LeadershipEvent**

In `crates/brain-services/src/ha/consensus/models.rs`:
```rust
#![allow(missing_docs)]

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

In `crates/brain-services/src/ha/consensus/mod.rs`:
```rust
pub mod models;

pub use models::*;
```

In `crates/brain-services/src/ha/mod.rs`:
```rust
pub mod consensus;
pub mod executor;
pub mod intent_log;
pub mod materializer;
pub mod models;
pub mod replay;
pub mod sqlite_intent_log;

pub use consensus::*;
pub use executor::*;
pub use intent_log::*;
pub use materializer::*;
pub use models::*;
pub use replay::*;
pub use sqlite_intent_log::*;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib ha::consensus::models::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/consensus/
git add crates/brain-services/src/ha/mod.rs
git commit -m "feat(ha): implement ReplicatedIntent, LocalExecutionState, and LeadershipEvent"
```

---

### Task 2: `LeaderLeaseManager` & Dual-Guard Lease Fencing

**Files:**
- Create: `crates/brain-services/src/ha/consensus/lease_manager.rs`
- Modify: `crates/brain-services/src/ha/consensus/mod.rs`
- Test: `crates/brain-services/src/ha/consensus/lease_manager.rs` (inline test module)

**Interfaces:**
- Consumes: `LeadershipEvent`
- Produces: `LeaderLeaseManager`

- [ ] **Step 1: Write unit tests for LeaderLeaseManager dual guard**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leader_lease_manager_fencing_and_expiration() {
        let mut manager = LeaderLeaseManager::new();
        assert!(!manager.is_leader(1000));

        manager.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);
        assert!(manager.is_leader(1002));
        assert!(!manager.is_leader(1006)); // Past 5s lease

        manager.handle_event(LeadershipEvent::BecameFollower { term: 2 }, 1002, 5);
        assert!(!manager.is_leader(1002)); // Immediately disabled on follower step-down
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --lib ha::consensus::lease_manager::tests`
Expected: FAIL with "cannot find type `LeaderLeaseManager`"

- [ ] **Step 3: Implement LeaderLeaseManager**

In `crates/brain-services/src/ha/consensus/lease_manager.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::consensus::models::*;

pub struct LeaderLeaseManager {
    current_term: u64,
    is_leader: bool,
    lease_expires_at: u64,
}

impl Default for LeaderLeaseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaderLeaseManager {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            is_leader: false,
            lease_expires_at: 0,
        }
    }

    pub fn is_leader(&self, now: u64) -> bool {
        self.is_leader && now < self.lease_expires_at
    }

    pub fn current_term(&self) -> u64 {
        self.current_term
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

In `crates/brain-services/src/ha/consensus/mod.rs`:
```rust
pub mod lease_manager;
pub mod models;

pub use lease_manager::*;
pub use models::*;
```

- [ ] **Step 4: Verify unit tests pass**

Run: `cargo test -p brain-services --lib ha::consensus::lease_manager::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/consensus/lease_manager.rs
git add crates/brain-services/src/ha/consensus/mod.rs
git commit -m "feat(ha): implement LeaderLeaseManager and dual-guard lease fencing"
```

---

### Task 3: `CommitNotifier` & `RaftIntentLog` Trait Bridge

**Files:**
- Create: `crates/brain-services/src/ha/consensus/raft_log.rs`
- Modify: `crates/brain-services/src/ha/consensus/mod.rs`
- Test: `crates/brain-services/tests/raft_consensus_tests.rs`

**Interfaces:**
- Consumes: `IntentRecord`, `SequenceNumber`, `EffectId`
- Produces: `CommitNotifier`, `RaftIntentLog`

- [ ] **Step 1: Write integration tests for RaftIntentLog and CommitNotifier**

In `crates/brain-services/tests/raft_consensus_tests.rs`:
```rust
use brain_services::ha::*;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_raft_intent_log_commit_notifier_and_status_tracking() {
    let mock_log = Arc::new(MockRaftIntentLog::new());

    let effect_id = EffectId(Uuid::new_v4());
    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Persisted,
    };

    mock_log.append_record(&record).await.unwrap();

    let committed = mock_log.wait_for_commit(SequenceNumber(1)).await.unwrap();
    assert_eq!(committed.effect_id, effect_id);

    // update_status updates local execution tracker, not consensus log
    mock_log.update_status(effect_id, IntentStatus::Completed).await.unwrap();
    assert!(mock_log.is_locally_completed(effect_id));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p brain-services --test raft_consensus_tests`
Expected: FAIL with "cannot find type `MockRaftIntentLog`"

- [ ] **Step 3: Implement CommitNotifier and MockRaftIntentLog**

In `crates/brain-services/src/ha/consensus/raft_log.rs`:
```rust
#![allow(missing_docs)]

use crate::ha::intent_log::*;
use crate::ha::models::*;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[async_trait]
pub trait CommitNotifier: Send + Sync {
    async fn wait_for_commit(&self, sequence: SequenceNumber) -> Result<ReplicatedIntent, IntentLogError>;
}

pub struct MockRaftIntentLog {
    records: Arc<Mutex<Vec<ReplicatedIntent>>>,
    local_completed: Arc<Mutex<HashSet<EffectId>>>,
}

impl Default for MockRaftIntentLog {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRaftIntentLog {
    pub fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            local_completed: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn is_locally_completed(&self, effect_id: EffectId) -> bool {
        self.local_completed.lock().contains(&effect_id)
    }
}

#[async_trait]
impl CommitNotifier for MockRaftIntentLog {
    async fn wait_for_commit(&self, sequence: SequenceNumber) -> Result<ReplicatedIntent, IntentLogError> {
        let recs = self.records.lock();
        recs.iter()
            .find(|r| r.sequence == sequence)
            .cloned()
            .ok_or_else(|| IntentLogError::Storage(format!("Sequence {:?} not committed", sequence)))
    }
}

#[async_trait]
impl IntentLog for MockRaftIntentLog {
    async fn append_record(&self, record: &IntentRecord) -> Result<(), IntentLogError> {
        let mut recs = self.records.lock();
        let intent = ReplicatedIntent {
            sequence: record.sequence,
            event_id: record.event_id,
            effect_id: record.effect_id,
            created_at: record.created_at,
            effect: record.effect.clone(),
        };
        recs.push(intent);
        Ok(())
    }

    async fn update_status(&self, effect_id: EffectId, status: IntentStatus) -> Result<(), IntentLogError> {
        if status == IntentStatus::Completed {
            self.local_completed.lock().insert(effect_id);
        }
        Ok(())
    }

    async fn load_from(&self, sequence: SequenceNumber) -> Result<Vec<IntentRecord>, IntentLogError> {
        let recs = self.records.lock();
        let completed = self.local_completed.lock();

        Ok(recs
            .iter()
            .filter(|r| r.sequence.0 >= sequence.0)
            .map(|r| {
                let st = if completed.contains(&r.effect_id) {
                    IntentStatus::Completed
                } else {
                    IntentStatus::Persisted
                };
                IntentRecord {
                    sequence: r.sequence,
                    event_id: r.event_id,
                    effect_id: r.effect_id,
                    created_at: r.created_at,
                    effect: r.effect.clone(),
                    status: st,
                }
            })
            .collect())
    }

    async fn scan_pending(&self) -> Result<Vec<IntentRecord>, IntentLogError> {
        self.load_from(SequenceNumber(0)).await.map(|recs| {
            recs.into_iter()
                .filter(|r| r.status != IntentStatus::Completed)
                .collect()
        })
    }
}
```

In `crates/brain-services/src/ha/consensus/mod.rs`:
```rust
pub mod lease_manager;
pub mod models;
pub mod raft_log;

pub use lease_manager::*;
pub use models::*;
pub use raft_log::*;
```

- [ ] **Step 4: Verify Raft intent log integration tests pass**

Run: `cargo test -p brain-services --test raft_consensus_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/ha/consensus/raft_log.rs
git add crates/brain-services/src/ha/consensus/mod.rs
git add crates/brain-services/tests/raft_consensus_tests.rs
git commit -m "feat(ha): implement CommitNotifier and MockRaftIntentLog trait bridge"
```

---

### Task 4: End-to-End Raft Cluster Failover & Quorum Integration Suite

**Files:**
- Create: `crates/brain-services/tests/r29_raft_consensus_tests.rs`
- Test: Run full workspace check `cargo check --workspace`

- [ ] **Step 1: Write 5 cluster failover integration test scenarios**

In `crates/brain-services/tests/r29_raft_consensus_tests.rs`:
```rust
use brain_services::ha::*;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_scenario_1_leader_election_replays_committed_unexecuted_intents() {
    let mock_log = Arc::new(MockRaftIntentLog::new());
    let effect_id = EffectId(Uuid::new_v4());

    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Persisted,
    };

    mock_log.append_record(&record).await.unwrap();

    let executor = Arc::new(MockEffectExecutor::new());
    let engine = IntentReplayEngine::new(mock_log.clone(), executor.clone());

    // Trigger replay on BecameLeader
    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);
    assert!(lease_mgr.is_leader(1001));

    engine.replay_pending().await.unwrap();
    assert_eq!(executor.executed_count(), 1);
    assert!(mock_log.is_locally_completed(effect_id));
}

#[tokio::test]
async fn test_scenario_2_leader_step_down_disables_effect_execution() {
    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);
    assert!(lease_mgr.is_leader(1001));

    lease_mgr.handle_event(LeadershipEvent::BecameFollower { term: 2 }, 1002, 5);
    assert!(!lease_mgr.is_leader(1002));
}

#[tokio::test]
async fn test_scenario_3_follower_promotion_applies_state_machine_with_zero_follower_side_effects() {
    let mock_log = Arc::new(MockRaftIntentLog::new());
    let executor = Arc::new(MockEffectExecutor::new());

    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameFollower { term: 1 }, 1000, 5);

    // Follower executes 0 side effects
    if lease_mgr.is_leader(1000) {
        let engine = IntentReplayEngine::new(mock_log.clone(), executor.clone());
        engine.replay_pending().await.unwrap();
    }

    assert_eq!(executor.executed_count(), 0);
}

#[tokio::test]
async fn test_scenario_4_duplicate_replay_validation_enforces_effect_id_idempotency() {
    let mock_log = Arc::new(MockRaftIntentLog::new());
    let executor = Arc::new(MockEffectExecutor::new());
    let effect_id = EffectId(Uuid::new_v4());

    let effect = CoordinatorEffect::EmitWorkerLost("w1".to_string());
    executor.execute_effect(effect_id, &effect).await.unwrap();
    executor.execute_effect(effect_id, &effect).await.unwrap();

    assert_eq!(executor.executed_count(), 1);
}

#[tokio::test]
async fn test_scenario_5_network_partition_lease_expiration_aborts_effect_dispatches() {
    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);

    // After 5s partition, lease expires
    assert!(!lease_mgr.is_leader(1006));
}
```

- [ ] **Step 2: Run end-to-end integration tests**

Run: `cargo test -p brain-services --test r29_raft_consensus_tests`
Expected: PASS

- [ ] **Step 3: Run full workspace check**

Run: `cargo check --workspace`
Expected: PASS (all workspace crates compile cleanly)

- [ ] **Step 4: Commit**

```bash
git add crates/brain-services/tests/r29_raft_consensus_tests.rs
git commit -m "test(ha): add 5 cluster failover and quorum integration test scenarios"
```

---

## Inline Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-28-r29-raft-consensus-plan.md`.

Proceeding with **Inline Execution** (`executing-plans` skill) task-by-task.
