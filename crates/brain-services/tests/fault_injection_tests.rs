//! Integration & Chaos Test Suite for `PartitionSimulator`, `NetworkDelaySimulator`, `PacketDropSimulator`, and `FaultInjectionHarness` (Phase 15 Milestone 15.2).

use brain_services::planning::{
    FaultInjectionHarness, FaultInjector, NetworkDelaySimulator, NodeId, PacketDropSimulator,
    PartitionSimulator,
};
use uuid::Uuid;

#[test]
fn test_partition_simulator_node_isolation_and_healing() {
    let simulator = PartitionSimulator::new();
    let node_a = NodeId(Uuid::new_v4());
    let node_b = NodeId(Uuid::new_v4());
    let node_c = NodeId(Uuid::new_v4());

    // 1. Initial topology -> Pass-through (connected)
    assert!(!simulator.is_partitioned(&node_a, &node_b));
    assert!(!simulator.is_partitioned(&node_b, &node_c));

    // 2. Partition link between node_a and node_b
    simulator.partition_nodes(node_a, node_b);
    assert!(simulator.is_partitioned(&node_a, &node_b));
    assert!(simulator.is_partitioned(&node_b, &node_a)); // Bidirectional!
    assert!(!simulator.is_partitioned(&node_b, &node_c)); // Unaffected!

    // 3. Isolate node_c from peers [node_a, node_b]
    simulator.isolate_node(node_c, &[node_a, node_b]);
    assert!(simulator.is_partitioned(&node_c, &node_a));
    assert!(simulator.is_partitioned(&node_c, &node_b));

    // 4. Heal link between node_a and node_b
    simulator.heal_nodes(node_a, node_b);
    assert!(!simulator.is_partitioned(&node_a, &node_b));
    assert!(simulator.is_partitioned(&node_c, &node_a)); // node_c still isolated!

    // 5. Heal all partitions
    simulator.heal_all();
    assert!(!simulator.is_partitioned(&node_c, &node_a));
    assert!(!simulator.is_partitioned(&node_c, &node_b));
}

#[test]
fn test_seeded_packet_drop_simulator_reproducibility() {
    let node_a = NodeId(Uuid::new_v4());
    let node_b = NodeId(Uuid::new_v4());

    // Fixed seed = 42, drop_pct = 50
    let drop1 = PacketDropSimulator::new(50, 42);
    let drop2 = PacketDropSimulator::new(50, 42);

    let mut samples1 = Vec::new();
    let mut samples2 = Vec::new();

    for _ in 0..20 {
        samples1.push(drop1.evaluate_fault(&node_a, &node_b).drop_packet);
        samples2.push(drop2.evaluate_fault(&node_a, &node_b).drop_packet);
    }

    // 100% deterministic reproducibility!
    assert_eq!(samples1, samples2);
}

#[test]
fn test_fault_injection_harness_composable_effects_aggregation() {
    let node_a = NodeId(Uuid::new_v4());
    let node_b = NodeId(Uuid::new_v4());

    let mut harness = FaultInjectionHarness::new();

    let partition = PartitionSimulator::new();
    partition.partition_nodes(node_a, node_b);
    harness.register_injector(partition);

    let delay = NetworkDelaySimulator::new(150);
    harness.register_injector(delay);

    let drop = PacketDropSimulator::new(100, 12345); // 100% drop
    harness.register_injector(drop);

    let effects = harness.evaluate(&node_a, &node_b);

    // Composable aggregation verification!
    assert!(effects.partitioned);
    assert_eq!(effects.delay_ms, Some(150));
    assert!(effects.drop_packet);
    assert!(!effects.is_pass_through());
}
