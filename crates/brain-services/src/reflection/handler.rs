use brain_core::errors::BrainError;
use brain_core::repositories::StorageTransaction;
use brain_domain::{
    Derivation, Edge, EdgeId, MemoryMergePolicy, ReflectionDomainCommand, ReflectionDomainEvent,
    RuleId,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Handles execution of reflection domain commands within a transaction boundary.
pub struct ReflectionCommandHandler;

impl Default for ReflectionCommandHandler {
    fn default() -> Self {
        Self
    }
}

impl ReflectionCommandHandler {
    /// Creates a new `ReflectionCommandHandler`.
    pub fn new() -> Self {
        Self
    }

    /// Executes a single reflection command, mutating the repositories and returning the generated event.
    pub fn handle(
        &self,
        tx: &dyn StorageTransaction,
        command: ReflectionDomainCommand,
    ) -> Result<ReflectionDomainEvent, BrainError> {
        let repos = tx.repositories();

        match command {
            ReflectionDomainCommand::MergeConcepts {
                canonical_id,
                duplicate_id,
            } => {
                let canonical = repos.nodes().find_by_id(&canonical_id)?.ok_or_else(|| {
                    BrainError::Validation {
                        message: format!("Canonical node {:?} not found", canonical_id),
                    }
                })?;
                let duplicate = repos.nodes().find_by_id(&duplicate_id)?.ok_or_else(|| {
                    BrainError::Validation {
                        message: format!("Duplicate node {:?} not found", duplicate_id),
                    }
                })?;

                // 1. Merge the nodes using domain policy
                let (merged_node, _) =
                    MemoryMergePolicy::merge(&canonical, &duplicate).map_err(|e| {
                        BrainError::Validation {
                            message: format!("Domain merge logic failed: {:?}", e),
                        }
                    })?;

                // 2. Persist the merged canonical node
                repos.nodes().save(&merged_node)?;

                // 3. Find and relink edges connected to the duplicate
                let connections = repos.edges().get_connections(&duplicate_id)?;
                for mut edge in connections {
                    // Delete old edge
                    let old_edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
                    repos.edges().delete(&old_edge_id)?;

                    let new_source = if edge.source == duplicate_id {
                        canonical_id
                    } else {
                        edge.source
                    };
                    let new_target = if edge.target == duplicate_id {
                        canonical_id
                    } else {
                        edge.target
                    };

                    if new_source == new_target {
                        // Skip creating self-loops
                        continue;
                    }

                    let new_edge_id = EdgeId::new(new_source, new_target, edge.relation.id());
                    if let Some(mut existing_edge) = repos.edges().find_by_id(&new_edge_id)? {
                        existing_edge.weight = f64::max(existing_edge.weight, edge.weight);
                        existing_edge.updated_at = current_time_secs();
                        repos.edges().save(&existing_edge)?;
                    } else {
                        edge.source = new_source;
                        edge.target = new_target;
                        edge.updated_at = current_time_secs();
                        repos.edges().save(&edge)?;
                    }
                }

                // 4. Delete the duplicate node
                repos.nodes().delete(&duplicate_id)?;

                let provenance = format!(
                    "Merged concept {} into {}",
                    duplicate.label, canonical.label
                );

                Ok(ReflectionDomainEvent::ConceptMerged {
                    canonical_id,
                    merged_id: duplicate_id,
                    provenance,
                })
            }
            ReflectionDomainCommand::CreateInferredRelation {
                source_id,
                target_id,
                relation_kind,
                confidence,
            } => {
                let edge = Edge::new_derived(
                    source_id,
                    target_id,
                    relation_kind,
                    confidence,
                    Derivation {
                        rule: RuleId::Transitive,
                        supporting_edges: Vec::new(),
                    },
                );
                repos.edges().save(&edge)?;

                Ok(ReflectionDomainEvent::RelationInferred {
                    source_id,
                    target_id,
                    relation_kind,
                })
            }
        }
    }
}

fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
