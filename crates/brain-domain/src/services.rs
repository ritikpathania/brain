use crate::entities::Node;
use crate::errors::DomainError;
use std::collections::HashMap;

/// Pure domain services for the Brain domain model.
pub struct MemoryMergePolicy;

impl MemoryMergePolicy {
    /// Merges two nodes into a single node using a defined conflict resolution strategy.
    /// Returns the merged node, keeping `a.id` as the primary identifier.
    ///
    /// Validates that both nodes have the same NodeType.
    pub fn merge(a: &Node, b: &Node) -> Result<Node, DomainError> {
        if a.node_type != b.node_type {
            return Err(DomainError::InvalidEdgeWeight(format!(
                "Cannot merge nodes of different types: {:?} and {:?}",
                a.node_type, b.node_type
            )));
        }

        // Determine which node is newer (conflict resolution)
        let (newer, _older) = if a.updated_at >= b.updated_at {
            (a, b)
        } else {
            (b, a)
        };

        // Merge properties
        let mut merged_properties = HashMap::new();
        // Insert all properties from the older node first
        for (k, v) in &_older.properties {
            merged_properties.insert(k.clone(), v.clone());
        }
        // Overwrite or deep-merge with properties from the newer node
        for (k, v) in &newer.properties {
            if let Some(old_val) = merged_properties.get_mut(k) {
                // If both are JSON objects, merge them
                if let (Some(old_obj), Some(new_obj)) = (old_val.as_object_mut(), v.as_object()) {
                    for (sub_k, sub_v) in new_obj {
                        old_obj.insert(sub_k.clone(), sub_v.clone());
                    }
                } else {
                    // Otherwise, the newer node's value overwrites
                    *old_val = v.clone();
                }
            } else {
                merged_properties.insert(k.clone(), v.clone());
            }
        }

        let updated_at = std::cmp::max(a.updated_at, b.updated_at);

        Ok(Node {
            id: a.id, // Primary identifier remains `a.id`
            label: newer.label.clone(),
            node_type: a.node_type.clone(),
            properties: merged_properties,
            updated_at,
        })
    }
}
