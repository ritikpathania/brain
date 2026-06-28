/// Domain events emitted by the Brain domain model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum DomainEvent {
    /// A new memory node has been created.
    MemoryCreated {
        /// The unique ID of the created node.
        node_id: String,
    },
    /// Two memory nodes have been merged.
    MemoryMerged {
        /// The target node ID that remains.
        target_id: String,
        /// The node ID that was merged/absorbed.
        merged_id: String,
    },
    /// A memory node has been promoted (e.g. to a higher status or importance).
    MemoryPromoted {
        /// The promoted node ID.
        node_id: String,
        /// Reason for promotion.
        reason: String,
    },
    /// A memory node has been forgotten.
    MemoryForgotten {
        /// The forgotten node ID.
        node_id: String,
    },
    /// A conversation thread has been archived.
    ConversationArchived {
        /// The archived conversation ID.
        conversation_id: String,
    },
    /// A relationship edge has been reinforced/strengthened.
    RelationshipStrengthened {
        /// The source node ID.
        source: String,
        /// The target node ID.
        target: String,
        /// The relationship label.
        relation: String,
        /// The new weight of the relationship.
        new_weight: f64,
    },
}
