use brain_core::repositories::{NodeRepository, EdgeRepository};
use brain_domain::{
    Node, Edge, NodeType, RelationKind, NodeId, ConsolidationPolicy
};
use brain_storage::TestStorage;
use brain_services::MemoryConsolidationService;

#[test]
fn test_memory_consolidation_sweep_invariants() {
    let test_store = TestStorage::new();
    let store = test_store.store();

    // 1. Create duplicate nodes with different casing/whitespace and properties
    let mut node_a_id = NodeId::new();
    let mut node_b_id = NodeId::new();
    if node_b_id < node_a_id {
        std::mem::swap(&mut node_a_id, &mut node_b_id);
    }

    let mut n1 = Node::new(node_a_id, "UniqueEntity".to_string(), NodeType::Concept);
    let mut props_a = std::collections::HashMap::new();
    props_a.insert("key_a".to_string(), serde_json::json!("value_a"));
    n1 = n1.with_properties(props_a);
    
    let mut n2 = Node::new(node_b_id, "uniqueentity  ".to_string(), NodeType::Concept);
    let mut props_b = std::collections::HashMap::new();
    props_b.insert("key_b".to_string(), serde_json::json!("value_b"));
    n2 = n2.with_properties(props_b);

    NodeRepository::save(store.as_ref(), &n1).unwrap();
    NodeRepository::save(store.as_ref(), &n2).unwrap();

    // 2. Create edges for promotion (weight 0.9) and pruning/archival (weight 0.05)
    let node_c_id = NodeId::new();
    let n3 = Node::new(node_c_id, "AnotherNode".to_string(), NodeType::Concept);
    NodeRepository::save(store.as_ref(), &n3).unwrap();

    // Episodic edge to promote
    let edge_promote = Edge::new(node_a_id, node_c_id, RelationKind::Uses, 0.9);
    EdgeRepository::save(store.as_ref(), &edge_promote).unwrap();

    // Episodic edge to archive (low weight)
    let node_d_id = NodeId::new();
    let n4 = Node::new(node_d_id, "LowActivityNode".to_string(), NodeType::Concept);
    NodeRepository::save(store.as_ref(), &n4).unwrap();
    
    let edge_archive = Edge::new(node_a_id, node_d_id, RelationKind::DependsOn, 0.05);
    EdgeRepository::save(store.as_ref(), &edge_archive).unwrap();

    // Count before consolidation
    let initial_nodes = NodeRepository::list_all(store.as_ref()).unwrap().len();
    let initial_edges = EdgeRepository::list_all(store.as_ref()).unwrap().len();

    let policy = ConsolidationPolicy {
        promotion_weight_threshold: 0.8,
        pruning_weight_threshold: 0.1,
        staleness_age_threshold_secs: 100,
    };
    let service = MemoryConsolidationService::new(store.clone(), policy);

    // Run sweep
    let actions = service.run_consolidation_sweep().unwrap();
    assert!(!actions.is_empty(), "Consolidation should have planned actions");

    // Extract canonical and redundant IDs from the planned MergeNodes action
    let (canonical_id, redundant_id) = actions.iter().find_map(|act| {
        if let brain_domain::ConsolidationActionType::MergeNodes { canonical_node_id, redundant_node_ids, .. } = &act.action {
            Some((*canonical_node_id, redundant_node_ids[0]))
        } else {
            None
        }
    }).expect("MergeNodes action should be planned");

    // Invariant 1: Property Monotonicity
    // The canonical node should aggregate the properties monotonically
    let canonical = NodeRepository::find_by_id(store.as_ref(), &canonical_id).unwrap().unwrap();
    assert_eq!(canonical.properties.get("key_a").unwrap(), &serde_json::json!("value_a"));
    assert_eq!(canonical.properties.get("key_b").unwrap(), &serde_json::json!("value_b"));

    // Redundant node is deleted
    assert!(NodeRepository::find_by_id(store.as_ref(), &redundant_id).unwrap().is_none());

    // Invariant 2: Conservation of Knowledge
    let final_nodes = NodeRepository::list_all(store.as_ref()).unwrap().len();
    let final_edges = EdgeRepository::list_all(store.as_ref()).unwrap().len();

    // 1 node merged (redundant deleted)
    assert_eq!(final_nodes, initial_nodes - 1);
    
    // 1 edge archived (edge_archive deleted from active edges)
    assert_eq!(final_edges, initial_edges - 1);

    // Invariant 3: Archive Isolation
    // Archived edge is removed from active edges table, but present in archived_edges partition
    let active_edges = EdgeRepository::list_all(store.as_ref()).unwrap();
    let has_archived_in_active = active_edges.iter().any(|e| {
        e.source == node_a_id && e.target == node_d_id && e.relation == RelationKind::DependsOn
    });
    assert!(!has_archived_in_active, "Archived edge must be isolated from active edges list");

    let is_archived = store.is_edge_archived(&node_a_id.to_string(), &node_d_id.to_string(), &RelationKind::DependsOn.to_string()).unwrap();
    assert!(is_archived, "Edge must exist in the archived_edges table");

    // Invariant 4: Idempotency
    // Running consolidation sweep again without changes yields zero new actions
    let second_sweep_actions = service.run_consolidation_sweep().unwrap();
    assert_eq!(second_sweep_actions.len(), 0, "Consolidation sweep must be idempotent and generate zero actions on consecutive runs");

    test_store.assert_clean();
}
