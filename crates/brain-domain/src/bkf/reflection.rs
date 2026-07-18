use crate::bkf::ir::CompiledKnowledge;
use serde::{Deserialize, Serialize};

/// A single finding identified during the offline critique phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FindingItem {
    /// Two or more nodes refer to the same semantic concept.
    RedundantNodes {
        /// Identifiers of the redundant nodes.
        nodes: Vec<String>,
        /// The recommended canonical name/id.
        suggested_canonical: String,
    },
    /// A relationship edge between two nodes is weak and candidate for pruning or decay.
    WeakConnection {
        /// Source node ID.
        source: String,
        /// Target node ID.
        target: String,
        /// Current edge weight.
        weight: f32,
    },
    /// An edge expresses a relationship that violates schema or logic constraints.
    InvalidRelation {
        /// Source node ID.
        source: String,
        /// Target node ID.
        target: String,
        /// Relation name.
        relation: String,
        /// Rationale for why it is invalid.
        reason: String,
    },
    /// A node is missing critical metadata attributes.
    MissingContext {
        /// Node ID.
        node_id: String,
        /// Key of the missing attribute.
        key: String,
    },
}

/// Structured and versioned report containing offline analysis findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionFindings {
    /// Findings format version for future compatibility.
    pub findings_version: String,
    /// List of finding items.
    pub items: Vec<FindingItem>,
}

/// Composable rewrite operations representing precise modifications to the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RewriteOperation {
    /// Merge a source node into a target node, re-routing incoming/outgoing edges.
    MergeNodes {
        /// ID of node to be merged and removed.
        source: String,
        /// ID of the destination node.
        target: String,
    },
    /// Change the label/name of a node.
    RenameNode {
        /// ID of the node to rename.
        id: String,
        /// New label value.
        new_label: String,
    },
    /// Delete a node and all of its associated edges.
    DeleteNode {
        /// ID of the node to delete.
        id: String,
    },
    /// Strengthen the weight of a relationship.
    StrengthenEdge {
        /// Source node ID.
        source: String,
        /// Target node ID.
        target: String,
        /// Increment weight.
        amount: f32,
    },
    /// Weaken the weight of a relationship.
    WeakenEdge {
        /// Source node ID.
        source: String,
        /// Target node ID.
        target: String,
        /// Decrement weight.
        amount: f32,
    },
    /// Append or overwrite a metadata attribute key/value pair on a node.
    AddMetadata {
        /// ID of the node.
        node_id: String,
        /// Attribute key.
        key: String,
        /// Attribute value.
        value: String,
    },
}

/// Versioned and composable rewrite plan containing a series of sequential mutations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewritePlan {
    /// Schema version of the plan.
    pub plan_version: String,
    /// Ordered list of rewrite operations to apply.
    pub operations: Vec<RewriteOperation>,
    /// Rationale explaining the critique and why these rewrites are requested.
    pub rationale: String,
}

/// Pure domain Reflection Engine. Analyzes CompiledKnowledge and produces ReflectionFindings.
#[derive(Debug, Clone, Default)]
pub struct ReflectionEngine;

impl ReflectionEngine {
    /// Creates a new ReflectionEngine.
    pub fn new() -> Self {
        Self
    }

    /// Performs heuristic analysis on CompiledKnowledge.
    pub fn analyze(&self, graph: &CompiledKnowledge) -> ReflectionFindings {
        let mut items = Vec::new();

        // 1. Detect redundant/duplicate nodes based on simple lowercase label matching
        let mut seen_labels: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for node in &graph.nodes {
            let norm = node.label.to_lowercase();
            seen_labels.entry(norm).or_default().push(node.id.clone());
        }

        for (_label, ids) in seen_labels {
            if ids.len() > 1 {
                items.push(FindingItem::RedundantNodes {
                    nodes: ids.clone(),
                    suggested_canonical: ids[0].clone(),
                });
            }
        }

        // 2. Detect weak connections (weight < 0.2)
        for edge in &graph.edges {
            if edge.weight < 0.2 {
                items.push(FindingItem::WeakConnection {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    weight: edge.weight,
                });
            }
        }

        ReflectionFindings {
            findings_version: "1.0.0".to_string(),
            items,
        }
    }
}

/// Pure domain Planner. Converts ReflectionFindings into a RewritePlan.
#[derive(Debug, Clone, Default)]
pub struct Planner;

impl Planner {
    /// Creates a new Planner.
    pub fn new() -> Self {
        Self
    }

    /// Forms a RewritePlan from findings.
    pub fn plan(&self, findings: &ReflectionFindings) -> RewritePlan {
        let mut operations = Vec::new();
        let mut rationales = Vec::new();

        for finding in &findings.items {
            match finding {
                FindingItem::RedundantNodes {
                    nodes,
                    suggested_canonical,
                } => {
                    for node in nodes {
                        if node != suggested_canonical {
                            operations.push(RewriteOperation::MergeNodes {
                                source: node.clone(),
                                target: suggested_canonical.clone(),
                            });
                        }
                    }
                    rationales.push(format!("Merge duplicate nodes: {:?}", nodes));
                }
                FindingItem::WeakConnection {
                    source,
                    target,
                    weight,
                } => {
                    operations.push(RewriteOperation::WeakenEdge {
                        source: source.clone(),
                        target: target.clone(),
                        amount: 0.1_f32,
                    });
                    rationales.push(format!(
                        "Weaken weak edge {}->{} (weight={})",
                        source, target, weight
                    ));
                }
                FindingItem::InvalidRelation {
                    source,
                    target,
                    relation,
                    reason,
                } => {
                    rationales.push(format!(
                        "Invalid relation {}->{} ({}): {}",
                        source, target, relation, reason
                    ));
                }
                FindingItem::MissingContext { node_id, key } => {
                    rationales.push(format!("Node {} is missing attribute {}", node_id, key));
                }
            }
        }

        RewritePlan {
            plan_version: "1.0.0".to_string(),
            operations,
            rationale: rationales.join("; "),
        }
    }
}
