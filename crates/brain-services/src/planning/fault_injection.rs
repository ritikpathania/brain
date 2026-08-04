//! Chaos Testing & Fault Injection Engine (Phase 15 Milestone 15.2).
//!
//! ### Architectural Invariants:
//! 1. Composable Fault Effects: `FaultEffects` combines multiple simultaneous network anomalies (partitioning, delay, packet loss) in a single evaluation.
//! 2. Deterministic Reproducibility: Simulators support deterministic seeded evaluation for 100% replayable chaos runs.
//! 3. Topology Isolation: `PartitionSimulator` manages pairwise connectivity graphs independently of test scenario scheduling.

use crate::planning::cluster::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;

/// Composable representation of network fault effects evaluated between node pairs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FaultEffects {
    /// `true` if network link between nodes is partitioned.
    pub partitioned: bool,
    /// Optional transport delay to inject in milliseconds.
    pub delay_ms: Option<u64>,
    /// `true` if transport packet should be dropped.
    pub drop_packet: bool,
}

impl FaultEffects {
    /// Returns `true` if no faults are active.
    pub fn is_pass_through(&self) -> bool {
        !self.partitioned && self.delay_ms.is_none() && !self.drop_packet
    }
}

/// Abstract fault evaluation policy interface.
pub trait FaultInjector: Send + Sync {
    /// Evaluates active fault effects between source and target nodes.
    fn evaluate_fault(&self, source_node: &NodeId, target_node: &NodeId) -> FaultEffects;
}

/// Topology manager tracking pairwise node network connectivity and partition states.
#[derive(Debug, Default)]
pub struct PartitionSimulator {
    partitioned_pairs: Mutex<HashSet<(NodeId, NodeId)>>,
}

impl PartitionSimulator {
    /// Instantiates a new `PartitionSimulator`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Partitions bidirectional network link between node_a and node_b.
    pub fn partition_nodes(&self, node_a: NodeId, node_b: NodeId) {
        let mut guard = self.partitioned_pairs.lock().unwrap();
        guard.insert((node_a, node_b));
        guard.insert((node_b, node_a));
    }

    /// Isolates a node from all communication in cluster topology.
    pub fn isolate_node(&self, node: NodeId, peers: &[NodeId]) {
        for peer in peers {
            if *peer != node {
                self.partition_nodes(node, *peer);
            }
        }
    }

    /// Heals network link between node_a and node_b.
    pub fn heal_nodes(&self, node_a: NodeId, node_b: NodeId) {
        let mut guard = self.partitioned_pairs.lock().unwrap();
        guard.remove(&(node_a, node_b));
        guard.remove(&(node_b, node_a));
    }

    /// Heals all active network partitions in cluster topology.
    pub fn heal_all(&self) {
        let mut guard = self.partitioned_pairs.lock().unwrap();
        guard.clear();
    }

    /// Returns `true` if link between source and target is partitioned.
    pub fn is_partitioned(&self, source: &NodeId, target: &NodeId) -> bool {
        let guard = self.partitioned_pairs.lock().unwrap();
        guard.contains(&(*source, *target))
    }
}

impl FaultInjector for PartitionSimulator {
    fn evaluate_fault(&self, source_node: &NodeId, target_node: &NodeId) -> FaultEffects {
        let is_part = self.is_partitioned(source_node, target_node);
        FaultEffects {
            partitioned: is_part,
            delay_ms: None,
            drop_packet: false,
        }
    }
}

/// Simulator injecting configurable or jittered transport delay.
#[derive(Debug, Default)]
pub struct NetworkDelaySimulator {
    delay_ms: Mutex<u64>,
}

impl NetworkDelaySimulator {
    /// Instantiates a new `NetworkDelaySimulator` with specified delay in milliseconds.
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms: Mutex::new(delay_ms),
        }
    }

    /// Updates injected transport delay in milliseconds.
    pub fn set_delay_ms(&self, delay_ms: u64) {
        let mut guard = self.delay_ms.lock().unwrap();
        *guard = delay_ms;
    }
}

impl FaultInjector for NetworkDelaySimulator {
    fn evaluate_fault(&self, _source_node: &NodeId, _target_node: &NodeId) -> FaultEffects {
        let delay = *self.delay_ms.lock().unwrap();
        FaultEffects {
            partitioned: false,
            delay_ms: if delay > 0 { Some(delay) } else { None },
            drop_packet: false,
        }
    }
}

/// Deterministic, seeded probabilistic packet drop simulator.
#[derive(Debug)]
pub struct PacketDropSimulator {
    drop_pct: u8,
    seed: Mutex<u64>,
}

impl PacketDropSimulator {
    /// Instantiates a new `PacketDropSimulator` with drop percentage and initial random seed.
    pub fn new(drop_pct: u8, seed: u64) -> Self {
        Self {
            drop_pct: drop_pct.min(100),
            seed: Mutex::new(seed),
        }
    }

    /// Evaluates pseudo-random deterministic drop decision.
    fn should_drop(&self) -> bool {
        if self.drop_pct == 0 {
            return false;
        }
        if self.drop_pct >= 100 {
            return true;
        }

        let mut s = self.seed.lock().unwrap();
        // Linear congruential pseudo-random generator
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let sample = ((*s >> 32) % 100) as u8;
        sample < self.drop_pct
    }
}

impl FaultInjector for PacketDropSimulator {
    fn evaluate_fault(&self, _source_node: &NodeId, _target_node: &NodeId) -> FaultEffects {
        let drop = self.should_drop();
        FaultEffects {
            partitioned: false,
            delay_ms: None,
            drop_packet: drop,
        }
    }
}

/// Chaos testing orchestrator combining multiple `FaultInjector` policy engines.
pub struct FaultInjectionHarness {
    injectors: Vec<Box<dyn FaultInjector>>,
}

impl Default for FaultInjectionHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl FaultInjectionHarness {
    /// Instantiates a new `FaultInjectionHarness`.
    pub fn new() -> Self {
        Self {
            injectors: Vec::new(),
        }
    }

    /// Registers a fault injector in the harness pipeline.
    pub fn register_injector<I: FaultInjector + 'static>(&mut self, injector: I) {
        self.injectors.push(Box::new(injector));
    }

    /// Evaluates aggregated `FaultEffects` across all registered injectors.
    pub fn evaluate(&self, source_node: &NodeId, target_node: &NodeId) -> FaultEffects {
        let mut aggregated = FaultEffects::default();

        for injector in &self.injectors {
            let eff = injector.evaluate_fault(source_node, target_node);
            if eff.partitioned {
                aggregated.partitioned = true;
            }
            if eff.drop_packet {
                aggregated.drop_packet = true;
            }
            if let Some(d) = eff.delay_ms {
                let current = aggregated.delay_ms.unwrap_or(0);
                aggregated.delay_ms = Some(current + d);
            }
        }

        aggregated
    }
}
