use crate::entities::{Edge, KnowledgeGraph};
use crate::identifiers::{EdgeId, NodeId};
use crate::query::PathQuery;
use std::collections::HashSet;

/// Service executing path search algorithms over the graph.
pub struct PathQueryService;

impl PathQueryService {
    /// Finds all paths connecting the source node to the target node matching the specified limits and filters.
    pub fn find_paths(
        graph: &KnowledgeGraph,
        source: &NodeId,
        target: &NodeId,
        query: &PathQuery,
    ) -> Vec<Vec<EdgeId>> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut results = Vec::new();

        Self::find_paths_dfs(
            graph,
            source,
            target,
            query,
            &mut visited,
            &mut path,
            &mut results,
        );

        results.sort_by(|p1, p2| {
            let len_cmp = p1.len().cmp(&p2.len());
            if len_cmp != std::cmp::Ordering::Equal {
                return len_cmp;
            }
            p1.cmp(p2)
        });

        results
    }

    fn find_paths_dfs(
        graph: &KnowledgeGraph,
        current: &NodeId,
        target: &NodeId,
        query: &PathQuery,
        visited: &mut HashSet<NodeId>,
        path: &mut Vec<EdgeId>,
        results: &mut Vec<Vec<EdgeId>>,
    ) {
        if current == target {
            if !path.is_empty() {
                results.push(path.clone());
            }
            return;
        }

        if let Some(max_depth) = query.limits.max_depth {
            if path.len() >= max_depth {
                return;
            }
        }

        visited.insert(*current);

        let mut outgoing: Vec<&Edge> = graph
            .edges
            .values()
            .filter(|e| e.source == *current)
            .collect();
        outgoing.sort_by_key(|e| EdgeId::new(e.source, e.target, e.relation.id()));

        for edge in outgoing {
            if visited.contains(&edge.target) {
                continue;
            }

            if let Some(ref filter) = query.filters.relation_filter {
                if !filter.contains(&edge.relation.id()) {
                    continue;
                }
            }

            let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
            path.push(edge_id);
            Self::find_paths_dfs(graph, &edge.target, target, query, visited, path, results);
            path.pop();
        }

        visited.remove(current);
    }
}
