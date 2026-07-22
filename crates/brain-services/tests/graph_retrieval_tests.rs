/// Integration tests for graph-aware retrieval (Phase 8A).
///
/// Key invariant verified here:
///
/// > `graph_depth == None` is **observationally equivalent** to the existing
/// > source behaviour (depth 1), meaning:
/// >
/// > ```text
/// > LtmMemorySource(query, graph_depth=None)
/// > ==
/// > LtmMemorySource(query, graph_depth=Some(1))
/// > ```
///
/// Secondary properties verified:
/// - `graph_depth = Some(0)` produces flat retrieval (no expansion beyond direct hits).
/// - `graph_depth = Some(2)` surfaces nodes two hops away.
/// - All results remain deterministic across repeated calls.
/// - `exclude_ids` is respected even for nodes discovered via graph expansion.
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{MemorySource, RetrievalRequest};
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind};
use brain_services::retrieval::source::LtmMemorySource;
use brain_storage::TestStorage;
use std::collections::HashSet;
use std::sync::Arc;

fn node(id: NodeId, label: &str) -> Node {
    Node::new(id, label.to_string(), NodeType::Concept)
}

fn base_request(query: &str) -> RetrievalRequest {
    RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: query.to_string(),
        limit: 20,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false, // default — equivalent to depth 1
        reference_time: None,
    }
}

/// Build a small graph:
///
/// ```text
///  [rust]  ──(Uses)──▶  [memory]  ──(Uses)──▶  [heap]
///                   └──(Uses)──▶  [lifetime]
/// ```
///
/// FTS will hit "rust" directly. "memory" is at depth 1, "heap"/"lifetime" at
/// depth 2.
fn build_three_hop_graph() -> (
    TestStorage,
    NodeId, // rust
    NodeId, // memory
    NodeId, // heap
    NodeId, // lifetime
) {
    let test_store = TestStorage::new();
    let store = test_store.store(); // Arc<SqliteStorage> — implements RepositorySet

    let rust_id = NodeId::new();
    let memory_id = NodeId::new();
    let heap_id = NodeId::new();
    let lifetime_id = NodeId::new();

    store.nodes().save(&node(rust_id, "rust")).unwrap();
    store.nodes().save(&node(memory_id, "memory")).unwrap();
    store.nodes().save(&node(heap_id, "heap")).unwrap();
    store.nodes().save(&node(lifetime_id, "lifetime")).unwrap();

    store
        .edges()
        .save(&Edge::new(rust_id, memory_id, RelationKind::Uses, 1.0))
        .unwrap();
    store
        .edges()
        .save(&Edge::new(memory_id, heap_id, RelationKind::Uses, 1.0))
        .unwrap();
    store
        .edges()
        .save(&Edge::new(memory_id, lifetime_id, RelationKind::Uses, 1.0))
        .unwrap();

    (test_store, rust_id, memory_id, heap_id, lifetime_id)
}

fn make_source(test_store: &TestStorage) -> LtmMemorySource {
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    LtmMemorySource::new(test_store.store(), registry)
}

fn node_ids(result: &brain_core::retrieval::MemorySourceResult) -> HashSet<NodeId> {
    result.nodes.iter().map(|n| n.id).collect()
}

// ─── Equivalence invariant ──────────────────────────────────────────────────

#[test]
fn test_graph_depth_none_equivalent_to_depth_one() {
    let (test_store, rust_id, memory_id, _heap_id, _lifetime_id) = build_three_hop_graph();
    let source = make_source(&test_store);

    let req_none = base_request("rust");
    let req_one = RetrievalRequest {
        reference_time: None,
        graph_depth: Some(1),
        ..base_request("rust")
    };

    let ids_none = node_ids(&source.retrieve(&req_none).unwrap());
    let ids_one = node_ids(&source.retrieve(&req_one).unwrap());

    assert_eq!(
        ids_none, ids_one,
        "graph_depth=None must be observationally equivalent to graph_depth=Some(1)"
    );
    assert!(
        ids_none.contains(&rust_id),
        "direct FTS hit 'rust' must appear"
    );
    assert!(
        ids_none.contains(&memory_id),
        "depth-1 neighbour 'memory' must appear with default depth"
    );
}

// ─── Flat retrieval (depth 0) ────────────────────────────────────────────────

#[test]
fn test_graph_depth_zero_is_flat_retrieval() {
    let (test_store, rust_id, memory_id, heap_id, lifetime_id) = build_three_hop_graph();
    let source = make_source(&test_store);

    let req = RetrievalRequest {
        reference_time: None,
        graph_depth: Some(0),
        ..base_request("rust")
    };

    let result_ids = node_ids(&source.retrieve(&req).unwrap());

    assert!(
        result_ids.contains(&rust_id),
        "direct match 'rust' must appear"
    );
    assert!(
        !result_ids.contains(&memory_id),
        "depth-1 neighbour must NOT appear with graph_depth=0"
    );
    assert!(
        !result_ids.contains(&heap_id),
        "depth-2 'heap' must NOT appear with graph_depth=0"
    );
    assert!(
        !result_ids.contains(&lifetime_id),
        "depth-2 'lifetime' must NOT appear with graph_depth=0"
    );
}

// ─── Depth-2 expansion ───────────────────────────────────────────────────────

#[test]
fn test_graph_depth_two_surfaces_second_hop_nodes() {
    let (test_store, rust_id, memory_id, heap_id, lifetime_id) = build_three_hop_graph();
    let source = make_source(&test_store);

    let req = RetrievalRequest {
        reference_time: None,
        graph_depth: Some(2),
        ..base_request("rust")
    };

    let result_ids = node_ids(&source.retrieve(&req).unwrap());

    assert!(result_ids.contains(&rust_id), "direct hit must appear");
    assert!(result_ids.contains(&memory_id), "depth-1 must appear");
    assert!(
        result_ids.contains(&heap_id) || result_ids.contains(&lifetime_id),
        "at least one depth-2 neighbour must appear with graph_depth=2"
    );
}

// ─── Determinism ─────────────────────────────────────────────────────────────

#[test]
fn test_graph_depth_results_are_deterministic() {
    let (test_store, ..) = build_three_hop_graph();
    let source = make_source(&test_store);
    let req = base_request("rust");

    let ids1: Vec<NodeId> = source
        .retrieve(&req)
        .unwrap()
        .nodes
        .iter()
        .map(|n| n.id)
        .collect();
    let ids2: Vec<NodeId> = source
        .retrieve(&req)
        .unwrap()
        .nodes
        .iter()
        .map(|n| n.id)
        .collect();

    assert_eq!(
        ids1, ids2,
        "graph-aware retrieval must be deterministic across repeated calls"
    );
}

// ─── Exclude IDs are respected across expansion ──────────────────────────────

#[test]
fn test_graph_depth_respects_exclude_ids() {
    let (test_store, _rust_id, memory_id, _heap_id, _lifetime_id) = build_three_hop_graph();
    let source = make_source(&test_store);

    let mut exclude = HashSet::new();
    exclude.insert(memory_id);

    let req = RetrievalRequest {
        reference_time: None,
        exclude_ids: exclude,
        graph_depth: Some(1),
        ..base_request("rust")
    };

    let result = source.retrieve(&req).unwrap();
    assert!(
        !node_ids(&result).contains(&memory_id),
        "excluded node must not appear even via graph expansion"
    );
}

// ─── Monotonic traversal invariant ───────────────────────────────────────────
//
// For increasing graph_depth values the reachable candidate set must be
// monotonically non-decreasing:
//
//   Results(depth=0) ⊆ Results(depth=1) ⊆ Results(depth=2)
//
// This property catches subtle traversal regressions that determinism tests
// alone cannot surface.

#[test]
fn test_graph_depth_monotonic_candidate_set() {
    let (test_store, ..) = build_three_hop_graph();
    let source = make_source(&test_store);

    let ids_at_depth = |d: usize| -> HashSet<NodeId> {
        let req = RetrievalRequest {
            reference_time: None,
            graph_depth: Some(d),
            ..base_request("rust")
        };
        node_ids(&source.retrieve(&req).unwrap())
    };

    let depth0 = ids_at_depth(0);
    let depth1 = ids_at_depth(1);
    let depth2 = ids_at_depth(2);

    // Results(0) ⊆ Results(1)
    for id in &depth0 {
        assert!(
            depth1.contains(id),
            "node {:?} present at depth=0 must also appear at depth=1 (monotonicity)",
            id
        );
    }

    // Results(1) ⊆ Results(2)
    for id in &depth1 {
        assert!(
            depth2.contains(id),
            "node {:?} present at depth=1 must also appear at depth=2 (monotonicity)",
            id
        );
    }

    // Depth-2 should strictly expand beyond depth-1 in our three-hop graph.
    assert!(
        depth2.len() >= depth1.len(),
        "depth=2 candidate set must be at least as large as depth=1"
    );
    assert!(
        depth1.len() > depth0.len(),
        "depth=1 candidate set must be strictly larger than depth=0 in a connected graph"
    );
}
