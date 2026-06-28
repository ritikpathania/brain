use brain_domain::*;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_is_pinned_specification() {
    let mut node1 = Node::new(NodeId::new(), "Concept 1".to_string(), NodeType::Concept);
    node1.properties.insert("pinned".to_string(), json!(true));

    let mut node2 = Node::new(NodeId::new(), "Concept 2".to_string(), NodeType::Concept);
    node2.properties.insert("pinned".to_string(), json!(false));

    let node3 = Node::new(NodeId::new(), "Concept 3".to_string(), NodeType::Concept);

    let spec = IsPinned;
    assert!(spec.is_satisfied_by(&node1));
    assert!(!spec.is_satisfied_by(&node2));
    assert!(!spec.is_satisfied_by(&node3));
}

#[test]
fn test_is_expired_specification() {
    let source = NodeId::new();
    let target = NodeId::new();
    let mut edge = Edge::new(source, target, "connects".to_string(), 0.5);

    // Set updated_at to 1000 seconds ago
    edge.updated_at = 1000;

    // Current time: 2000, TTL: 500 -> elapsed 1000 > 500 -> expired
    let spec1 = IsExpired::new(500, 2000);
    assert!(spec1.is_satisfied_by(&edge));

    // Current time: 2000, TTL: 1500 -> elapsed 1000 <= 1500 -> not expired
    let spec2 = IsExpired::new(1500, 2000);
    assert!(!spec2.is_satisfied_by(&edge));
}

#[test]
fn test_specification_combinators() {
    let mut node = Node::new(NodeId::new(), "Concept".to_string(), NodeType::Concept);
    node.properties.insert("pinned".to_string(), json!(true));

    // A dummy specification that always matches Concepts
    struct IsConcept;
    impl Specification<Node> for IsConcept {
        fn is_satisfied_by(&self, n: &Node) -> bool {
            matches!(n.node_type, NodeType::Concept)
        }
    }

    // A dummy specification that matches Files
    struct IsFile;
    impl Specification<Node> for IsFile {
        fn is_satisfied_by(&self, n: &Node) -> bool {
            matches!(n.node_type, NodeType::File)
        }
    }

    // AND spec
    let spec_and = IsPinned.and(IsConcept);
    assert!(spec_and.is_satisfied_by(&node));

    let spec_and_fail = IsPinned.and(IsFile);
    assert!(!spec_and_fail.is_satisfied_by(&node));

    // OR spec
    let spec_or = IsPinned.or(IsFile);
    assert!(spec_or.is_satisfied_by(&node));

    let spec_or_fail = IsFile.or(NotSpecification { spec: IsPinned });
    assert!(!spec_or_fail.is_satisfied_by(&node));

    // NOT spec
    let spec_not = IsPinned.not();
    assert!(!spec_not.is_satisfied_by(&node));
}

#[test]
fn test_memory_merge_policy() {
    let id_a = NodeId::new();
    let id_b = NodeId::new();

    let mut props_a = HashMap::new();
    props_a.insert("importance".to_string(), json!(0.8));
    props_a.insert("tags".to_string(), json!(vec!["rust", "ddd"]));
    let mut details = HashMap::new();
    details.insert("author".to_string(), "Alice".to_string());
    props_a.insert("details".to_string(), json!(details));

    let mut props_b = HashMap::new();
    props_b.insert("importance".to_string(), json!(0.9));
    props_b.insert("score".to_string(), json!(100));
    let mut details_b = HashMap::new();
    details_b.insert("coauthor".to_string(), "Bob".to_string());
    details_b.insert("author".to_string(), "Charlie".to_string()); // conflict
    props_b.insert("details".to_string(), json!(details_b));

    // Case 1: Node A is newer
    let node_a = Node::new(id_a, "Node A".to_string(), NodeType::Concept)
        .with_properties(props_a.clone())
        .with_updated_at(1000);

    let node_b = Node::new(id_b, "Node B".to_string(), NodeType::Concept)
        .with_properties(props_b.clone())
        .with_updated_at(500);

    let merged_result = MemoryMergePolicy::merge(&node_a, &node_b);
    assert!(merged_result.is_ok());
    let merged = merged_result.unwrap();

    // Verify ID remains node_a's ID
    assert_eq!(merged.id, id_a);
    // Verify label is newer node's label
    assert_eq!(merged.label, "Node A");
    // Verify node_type matches
    assert_eq!(merged.node_type, NodeType::Concept);
    // Verify updated_at is max
    assert_eq!(merged.updated_at, 1000);

    // Verify merged properties
    assert_eq!(merged.properties.get("score").unwrap(), &json!(100));
    assert_eq!(merged.properties.get("importance").unwrap(), &json!(0.8)); // since A is newer, kept A's
    assert_eq!(merged.properties.get("tags").unwrap(), &json!(vec!["rust", "ddd"]));

    // Verify deep merged "details"
    let merged_details = merged.properties.get("details").unwrap().as_object().unwrap();
    assert_eq!(merged_details.get("author").unwrap().as_str().unwrap(), "Alice"); // from newer A
    assert_eq!(merged_details.get("coauthor").unwrap().as_str().unwrap(), "Bob"); // from older B

    // Case 2: Node B is newer
    let node_a_older = node_a.clone().with_updated_at(500);
    let node_b_newer = node_b.clone().with_updated_at(1000);

    let merged_result2 = MemoryMergePolicy::merge(&node_a_older, &node_b_newer);
    let merged2 = merged_result2.unwrap();
    assert_eq!(merged2.label, "Node B"); // B is newer
    assert_eq!(merged2.properties.get("importance").unwrap(), &json!(0.9)); // B's value

    let merged_details2 = merged2.properties.get("details").unwrap().as_object().unwrap();
    assert_eq!(merged_details2.get("author").unwrap().as_str().unwrap(), "Charlie"); // B is newer

    // Case 3: Node type mismatch
    let node_c = Node::new(NodeId::new(), "Node C".to_string(), NodeType::File)
        .with_updated_at(1000);

    let mismatch_result = MemoryMergePolicy::merge(&node_a, &node_c);
    assert!(mismatch_result.is_err());
}
