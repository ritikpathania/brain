use brain_core::errors::BrainError;
use brain_core::graph::RelationshipExpansionDTO;
use brain_core::repositories::RepositorySet;
use brain_domain::dtos::EdgeDTO;
use std::sync::Arc;

/// A post-retrieval service that enriches the ranked candidate list with first-order relationship context.
pub struct RelationshipExpander {
    repos: Arc<dyn RepositorySet>,
}

impl RelationshipExpander {
    /// Creates a new `RelationshipExpander`.
    pub fn new(repos: Arc<dyn RepositorySet>) -> Self {
        Self { repos }
    }

    /// Expands the first-order relationships for each of the given nodes, returning DTO representations.
    pub fn expand(
        &self,
        nodes: &[brain_domain::Node],
    ) -> Result<Vec<RelationshipExpansionDTO>, BrainError> {
        let mut expansions = Vec::with_capacity(nodes.len());
        for node in nodes {
            let connections = self.repos.edges().get_connections(&node.id)?;
            let mut incoming = Vec::new();
            let mut outgoing = Vec::new();

            for edge in connections {
                let edge_dto = EdgeDTO::new(
                    edge.source.0.to_string(),
                    edge.target.0.to_string(),
                    edge.relation.to_string(),
                    edge.weight,
                );
                if edge.target == node.id {
                    incoming.push(edge_dto);
                } else if edge.source == node.id {
                    outgoing.push(edge_dto);
                }
            }

            expansions.push(RelationshipExpansionDTO::new(
                node.id.0.to_string(),
                incoming,
                outgoing,
            ));
        }
        Ok(expansions)
    }
}
