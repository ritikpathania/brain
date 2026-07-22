use crate::reflection::ReflectionContext;
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_domain::{
    EdgeId, FindingEvidence, InferenceEngine, KnowledgeGraph, ReflectionFinding, RelationRegistry,
};

/// Pass suggestion transitive/inverse links using the domain InferenceEngine.
pub struct LinkSuggestionPass;

impl LinkSuggestionPass {
    /// Creates a new `LinkSuggestionPass`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinkSuggestionPass {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::reflection::ReflectionPass for LinkSuggestionPass {
    fn run(
        &self,
        snapshot: &dyn RepositorySet,
        _context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError> {
        // 1. Gather all nodes and edges into KnowledgeGraph
        let mut graph = KnowledgeGraph::new();
        for node in snapshot.nodes().list_all()? {
            graph.add_node(node);
        }

        let mut existing_edges = std::collections::HashSet::new();
        for edge in snapshot.edges().list_all()? {
            let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
            existing_edges.insert(edge_id);
            let _ = graph.add_edge(edge);
        }

        // 2. Invoke domain InferenceEngine
        let registry = RelationRegistry::default_embedded();
        let inferred_edges = InferenceEngine::infer(&graph, &registry);

        // 3. Find newly inferred edges and yield LinkSuggested findings
        let mut findings = Vec::new();
        for edge in inferred_edges {
            let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
            if !existing_edges.contains(&edge_id) {
                findings.push(ReflectionFinding::LinkSuggested {
                    source_id: edge.source,
                    target_id: edge.target,
                    relation_kind: edge.relation,
                    evidence: FindingEvidence {
                        confidence: edge.weight,
                        semantic_similarity: None,
                        edit_distance: None,
                        overlap_ratio: None,
                        details: format!(
                            "Inferred transitive path or inverse relationship of type {:?}",
                            edge.relation
                        ),
                    },
                });
            }
        }

        Ok(findings)
    }
}
