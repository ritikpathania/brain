//! Property recovery for memory/search: the application-layer projection
//! drops node properties (the `SearchMetadata` enum carries none), so
//! relations/excerpt/scope are read back from the stored node here.

use brain_core::repositories::{EdgeRepository, NodeRepository};
use brain_domain::NodeId;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

/// Node properties as persisted on [`brain_domain::Node`].
type NodeProps = HashMap<String, Value>;

/// Materializes connected relations for a node from both canonical graph edges
/// and legacy property fallbacks, resolving neighbor labels dynamically.
pub fn materialize_relations<S>(
    node_id: Option<&NodeId>,
    props: Option<&NodeProps>,
    metadata_fallback: Option<&str>,
    storage: &S,
) -> Vec<Value>
where
    S: EdgeRepository + NodeRepository,
{
    // Key: (relation, target_id) -> (Value, is_canonical)
    let mut relation_map: BTreeMap<(String, String), (Value, bool)> = BTreeMap::new();

    // 1. Canonical graph edges from EdgeRepository
    if let Some(nid) = node_id {
        if let Ok(connections) = storage.get_connections(nid) {
            for edge in connections {
                let is_outgoing = edge.source == *nid;
                let neighbor_id = if is_outgoing {
                    edge.target
                } else {
                    edge.source
                };
                let rel_str = edge.relation.id().as_str().to_string();
                let target_id_str = neighbor_id.to_string();

                let mut rel_obj = json!({
                    "relation": rel_str,
                    "target_id": target_id_str,
                });

                // Best-effort neighbor label resolution
                if let Ok(Some(neighbor)) = NodeRepository::find_by_id(storage, &neighbor_id) {
                    if !neighbor.label.trim().is_empty() {
                        rel_obj["target_label"] = json!(neighbor.label);
                    }
                }

                relation_map.insert((rel_str, target_id_str), (rel_obj, true));
            }
        }
    }

    // 2. Legacy property-based relations
    let legacy_rels = extract_relations(props, metadata_fallback);
    for raw in legacy_rels {
        if let Some(obj) = raw.as_object() {
            let rel_str = obj
                .get("relation")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to")
                .to_string();
            let target_id_str = obj
                .get("target_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            if target_id_str.is_empty() {
                continue;
            }

            let key = (rel_str.clone(), target_id_str.clone());
            if let std::collections::btree_map::Entry::Vacant(entry) = relation_map.entry(key) {
                let mut enriched = raw.clone();
                // If target_label is missing, attempt dynamic resolution if target_id is a valid UUID
                if enriched.get("target_label").is_none() {
                    if let Ok(uuid) = uuid::Uuid::parse_str(&target_id_str) {
                        let target_nid = NodeId(uuid);
                        if let Ok(Some(neighbor)) = NodeRepository::find_by_id(storage, &target_nid)
                        {
                            if !neighbor.label.trim().is_empty() {
                                enriched["target_label"] = json!(neighbor.label);
                            }
                        }
                    }
                }
                entry.insert((enriched, false));
            }
        }
    }

    // Return deterministically sorted values
    relation_map.into_values().map(|(val, _)| val).collect()
}

/// Resolve relations from a stored node's property map, tolerating native
/// JSON arrays and legacy string-encoded JSON, falling back to the summary
/// metadata string when the node carries nothing usable. Non-array garbage
/// falls through; never errors.
pub fn extract_relations(props: Option<&NodeProps>, metadata_fallback: Option<&str>) -> Vec<Value> {
    let decode = |raw: &Value| -> Option<Vec<Value>> {
        match raw {
            Value::Array(items) => Some(items.clone()),
            Value::String(s) => serde_json::from_str::<Vec<Value>>(s).ok(),
            _ => None,
        }
    };
    if let Some(map) = props {
        if let Some(raw) = map.get("relations") {
            if let Some(rels) = decode(raw) {
                return rels;
            }
        }
    }
    if let Some(encoded) = metadata_fallback {
        if let Ok(rels) = serde_json::from_str::<Vec<Value>>(encoded) {
            return rels;
        }
    }
    Vec::new()
}

/// Prefer the stored `content` property as the excerpt; fall back to whatever
/// body the retrieval pipeline produced (today: the node label).
pub fn preferred_excerpt(props: Option<&NodeProps>, fallback_body: &str) -> String {
    props
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_body.to_string())
}

/// Resolve scope from properties, falling back to the provided default.
pub fn preferred_scope(props: Option<&NodeProps>, fallback: &str) -> String {
    props
        .and_then(|m| m.get("scope"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_core::errors::BrainError;
    use brain_domain::{Edge, EdgeId, Node, NodeKind, RelationKind};
    use serde_json::json;

    fn map(v: Value) -> NodeProps {
        v.as_object().unwrap().clone().into_iter().collect()
    }

    #[derive(Default)]
    struct MockStorage {
        nodes: HashMap<NodeId, Node>,
        edges: Vec<Edge>,
    }

    impl NodeRepository for MockStorage {
        fn save(&self, _node: &Node) -> Result<(), BrainError> {
            Ok(())
        }
        fn save_batch(&self, _nodes: &[Node]) -> Result<(), BrainError> {
            Ok(())
        }
        fn find_by_id(&self, id: &NodeId) -> Result<Option<Node>, BrainError> {
            Ok(self.nodes.get(id).cloned())
        }
        fn find_by_tokens(&self, _tokens: &[String]) -> Result<Vec<Node>, BrainError> {
            Ok(Vec::new())
        }
        fn find_by_fts(&self, _query: &str) -> Result<Vec<(Node, f64)>, BrainError> {
            Ok(Vec::new())
        }
        fn delete(&self, _id: &NodeId) -> Result<(), BrainError> {
            Ok(())
        }
        fn list_all(&self) -> Result<Vec<Node>, BrainError> {
            Ok(self.nodes.values().cloned().collect())
        }
    }

    impl EdgeRepository for MockStorage {
        fn save(&self, _edge: &Edge) -> Result<(), BrainError> {
            Ok(())
        }
        fn save_batch(&self, _edges: &[Edge]) -> Result<(), BrainError> {
            Ok(())
        }
        fn find_by_id(&self, _id: &EdgeId) -> Result<Option<Edge>, BrainError> {
            Ok(None)
        }
        fn delete(&self, _id: &EdgeId) -> Result<(), BrainError> {
            Ok(())
        }
        fn get_connections(&self, node_id: &NodeId) -> Result<Vec<Edge>, BrainError> {
            Ok(self
                .edges
                .iter()
                .filter(|e| e.source == *node_id || e.target == *node_id)
                .cloned()
                .collect())
        }
        fn list_all(&self) -> Result<Vec<Edge>, BrainError> {
            Ok(self.edges.clone())
        }
    }

    #[test]
    fn outgoing_canonical_edge_materialized_with_label() {
        let node_a = Node::new(NodeId::new(), "Alpha Node".to_string(), NodeKind::Concept);
        let node_b = Node::new(NodeId::new(), "Beta Target".to_string(), NodeKind::Concept);
        let edge = Edge::new(node_a.id, node_b.id, RelationKind::Uses, 1.0);

        let mut storage = MockStorage::default();
        storage.nodes.insert(node_a.id, node_a.clone());
        storage.nodes.insert(node_b.id, node_b.clone());
        storage.edges.push(edge);

        let rels = materialize_relations(Some(&node_a.id), None, None, &storage);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["relation"], "uses");
        assert_eq!(rels[0]["target_id"], node_b.id.to_string());
        assert_eq!(rels[0]["target_label"], "Beta Target");
    }

    #[test]
    fn incoming_canonical_edge_materialized_with_label() {
        let node_a = Node::new(NodeId::new(), "Source Node".to_string(), NodeKind::Concept);
        let node_b = Node::new(NodeId::new(), "Target Node".to_string(), NodeKind::Concept);
        let edge = Edge::new(node_a.id, node_b.id, RelationKind::DependsOn, 1.0);

        let mut storage = MockStorage::default();
        storage.nodes.insert(node_a.id, node_a.clone());
        storage.nodes.insert(node_b.id, node_b.clone());
        storage.edges.push(edge);

        // Querying node_b should surface the connection pointing to node_a
        let rels = materialize_relations(Some(&node_b.id), None, None, &storage);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["relation"], "depends_on");
        assert_eq!(rels[0]["target_id"], node_a.id.to_string());
        assert_eq!(rels[0]["target_label"], "Source Node");
    }

    #[test]
    fn dangling_target_retains_relation_without_label() {
        let node_a = Node::new(NodeId::new(), "Alpha Node".to_string(), NodeKind::Concept);
        let dangling_id = NodeId::new();
        let edge = Edge::new(node_a.id, dangling_id, RelationKind::CommunicatesVia, 1.0);

        let mut storage = MockStorage::default();
        storage.nodes.insert(node_a.id, node_a.clone());
        // Do NOT insert dangling_id into storage.nodes
        storage.edges.push(edge);

        let rels = materialize_relations(Some(&node_a.id), None, None, &storage);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["relation"], "communicates_via");
        assert_eq!(rels[0]["target_id"], dangling_id.to_string());
        assert!(rels[0].get("target_label").is_none());
    }

    #[test]
    fn property_only_relation_retained() {
        let target_id = NodeId::new();
        let props = map(json!({
            "relations": [
                {"relation": "custom_rel", "target_id": target_id.to_string(), "target_label": "Property Label"}
            ]
        }));
        let storage = MockStorage::default();

        let rels = materialize_relations(None, Some(&props), None, &storage);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["relation"], "custom_rel");
        assert_eq!(rels[0]["target_id"], target_id.to_string());
        assert_eq!(rels[0]["target_label"], "Property Label");
    }

    #[test]
    fn canonical_edge_takes_precedence_over_property_duplicate() {
        let node_a = Node::new(NodeId::new(), "Alpha".to_string(), NodeKind::Concept);
        let node_b = Node::new(
            NodeId::new(),
            "Beta Fresh Label".to_string(),
            NodeKind::Concept,
        );
        let edge = Edge::new(node_a.id, node_b.id, RelationKind::Uses, 1.0);

        let mut storage = MockStorage::default();
        storage.nodes.insert(node_a.id, node_a.clone());
        storage.nodes.insert(node_b.id, node_b.clone());
        storage.edges.push(edge);

        // Legacy property has stale label for same relation and target_id
        let props = map(json!({
            "relations": [
                {"relation": "uses", "target_id": node_b.id.to_string(), "target_label": "Beta Stale Label"}
            ]
        }));

        let rels = materialize_relations(Some(&node_a.id), Some(&props), None, &storage);
        assert_eq!(
            rels.len(),
            1,
            "Duplicate (relation, target_id) must be deduplicated"
        );
        assert_eq!(rels[0]["relation"], "uses");
        assert_eq!(rels[0]["target_id"], node_b.id.to_string());
        assert_eq!(
            rels[0]["target_label"], "Beta Fresh Label",
            "Canonical label must win over property stale label"
        );
    }

    #[test]
    fn multiple_relations_sorted_deterministically() {
        let node_a = Node::new(NodeId::new(), "Alpha".to_string(), NodeKind::Concept);
        let node_b = Node::new(NodeId::new(), "Zeta".to_string(), NodeKind::Concept);
        let node_c = Node::new(NodeId::new(), "Gamma".to_string(), NodeKind::Concept);

        let edge1 = Edge::new(node_a.id, node_b.id, RelationKind::Uses, 1.0);
        let edge2 = Edge::new(node_a.id, node_c.id, RelationKind::DependsOn, 1.0);

        let mut storage = MockStorage::default();
        storage.nodes.insert(node_a.id, node_a.clone());
        storage.nodes.insert(node_b.id, node_b.clone());
        storage.nodes.insert(node_c.id, node_c.clone());
        storage.edges.push(edge1);
        storage.edges.push(edge2);

        let rels = materialize_relations(Some(&node_a.id), None, None, &storage);
        assert_eq!(rels.len(), 2);
        // "depends_on" < "uses" alphabetically
        assert_eq!(rels[0]["relation"], "depends_on");
        assert_eq!(rels[0]["target_label"], "Gamma");
        assert_eq!(rels[1]["relation"], "uses");
        assert_eq!(rels[1]["target_label"], "Zeta");
    }

    #[test]
    fn relations_from_native_array_property() {
        let props = map(json!({"relations": [{"relation": "supports", "target_id": "beta-1"}]}));
        let rels = extract_relations(Some(&props), None);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0]["target_id"], "beta-1");
    }

    #[test]
    fn relations_from_legacy_string_encoded_property() {
        let props = map(json!({"relations": "[{\"relation\":\"supports\"}]"}));
        assert_eq!(extract_relations(Some(&props), None).len(), 1);
    }

    #[test]
    fn relations_fall_back_to_summary_metadata_string() {
        let encoded = "[{\"relation\":\"supports\",\"target_id\":\"x\"}]";
        assert_eq!(extract_relations(None, Some(encoded)).len(), 1);
    }

    #[test]
    fn relations_empty_when_nothing_carries_them() {
        let props = map(json!({"content": "just prose"}));
        let rels = extract_relations(Some(&props), Some("not json"));
        assert!(rels.is_empty());
    }

    #[test]
    fn relations_garbage_property_falls_through_to_metadata() {
        let props = map(json!({"relations": 42}));
        let rels = extract_relations(Some(&props), Some("[]"));
        assert!(rels.is_empty());
    }

    #[test]
    fn empty_array_is_preserved_not_treated_as_missing() {
        let props = map(json!({"relations": []}));
        assert!(extract_relations(Some(&props), Some("[{\"relation\":\"x\"}]")).is_empty());
    }

    #[test]
    fn excerpt_prefers_content_over_label_body() {
        let props = map(json!({"content": "real prose body"}));
        assert_eq!(
            preferred_excerpt(Some(&props), "Alpha Label"),
            "real prose body"
        );
    }

    #[test]
    fn excerpt_blank_content_falls_back() {
        let props = map(json!({"content": "   "}));
        assert_eq!(
            preferred_excerpt(Some(&props), "Alpha Label"),
            "Alpha Label"
        );
    }

    #[test]
    fn excerpt_without_props_returns_fallback() {
        assert_eq!(preferred_excerpt(None, "Alpha Label"), "Alpha Label");
    }

    #[test]
    fn scope_prefers_property_then_default() {
        let props = map(json!({"scope": "compiler"}));
        assert_eq!(preferred_scope(Some(&props), "workspace"), "compiler");
        assert_eq!(preferred_scope(None, "workspace"), "workspace");
    }
}
