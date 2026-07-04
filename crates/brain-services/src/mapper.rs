//! Data Transfer Object (DTO) mapper.

use brain_core::errors::BrainError;
use brain_domain::{Edge, EdgeDTO, MemoryDTO, Node, NodeDTO};

/// Maps a domain Node and its connections to a MemoryDTO.
pub fn to_memory_dto(node: &Node, connections: &[Edge]) -> Result<MemoryDTO, BrainError> {
    let mut incoming_edges = Vec::new();
    let mut outgoing_edges = Vec::new();

    for edge in connections {
        let edge_dto = EdgeDTO::new(
            edge.source.to_string(),
            edge.target.to_string(),
            edge.relation.to_string(),
            edge.weight,
        );
        if edge.target == node.id {
            incoming_edges.push(edge_dto);
        } else {
            outgoing_edges.push(edge_dto);
        }
    }

    let node_type_str = node.node_type.to_string();

    let node_dto = NodeDTO::new(
        node.id.to_string(),
        node.label.clone(),
        node_type_str,
        serde_json::to_value(&node.properties).unwrap_or_default(),
    );

    Ok(MemoryDTO::new(node_dto, incoming_edges, outgoing_edges))
}
