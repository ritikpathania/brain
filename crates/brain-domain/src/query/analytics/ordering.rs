use crate::identifiers::NodeId;
use crate::query::analytics::{DegreeCentrality, RelationDistribution, ClosenessResult};
use std::cmp::Ordering;

/// Orders nodes canonically in a slice.
pub fn sort_nodes_canonically(nodes: &mut [NodeId]) {
    nodes.sort();
}

/// Orders connected components canonically (each component sorted, and components sorted lexicographically).
pub fn sort_components_canonically(components: &mut [Vec<NodeId>]) {
    for comp in components.iter_mut() {
        comp.sort();
    }
    components.sort();
}

/// Orders degree centrality results descending by score, and secondarily lexicographically by node ID.
pub fn sort_centrality_canonically(centrality: &mut [DegreeCentrality]) {
    centrality.sort_by(|c1, c2| {
        let score_cmp = c2.score.cmp(&c1.score); // descending
        if score_cmp != Ordering::Equal {
            return score_cmp;
        }
        c1.node.cmp(&c2.node)
    });
}

/// Orders relation distributions descending by count, and secondarily lexicographically by relation ID.
pub fn sort_distribution_canonically(dist: &mut [RelationDistribution]) {
    dist.sort_by(|r1, r2| {
        let count_cmp = r2.count.cmp(&r1.count); // descending
        if count_cmp != Ordering::Equal {
            return count_cmp;
        }
        r1.relation.cmp(&r2.relation)
    });
}

/// Orders closeness centrality results descending by score, and secondarily lexicographically by node ID.
pub fn sort_closeness_canonically(closeness: &mut [ClosenessResult]) {
    closeness.sort_by(|c1, c2| {
        let score_cmp = c2.score.partial_cmp(&c1.score).unwrap_or(Ordering::Equal); // descending
        if score_cmp != Ordering::Equal {
            return score_cmp;
        }
        c1.node.cmp(&c2.node)
    });
}
