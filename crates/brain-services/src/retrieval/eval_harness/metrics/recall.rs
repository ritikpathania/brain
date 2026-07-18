use brain_domain::NodeId;
use std::collections::HashSet;

/// Computes Recall@K: fraction of expected nodes retrieved in the top K results.
pub fn compute_recall_at_k(retrieved: &[NodeId], expected: &[NodeId], k: usize) -> f64 {
    if expected.is_empty() {
        return 0.0;
    }
    let limit = std::cmp::min(k, retrieved.len());
    let retrieved_set: HashSet<&NodeId> = retrieved[..limit].iter().collect();

    let mut hits = 0;
    for node_id in expected {
        if retrieved_set.contains(node_id) {
            hits += 1;
        }
    }
    hits as f64 / expected.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recall_empty() {
        let expected = vec![NodeId::new()];
        assert_eq!(compute_recall_at_k(&[], &expected, 5), 0.0);
        assert_eq!(compute_recall_at_k(&[], &[], 5), 0.0);
    }

    #[test]
    fn test_recall_happy() {
        let n1 = NodeId::new();
        let n2 = NodeId::new();
        let n3 = NodeId::new();
        let retrieved = vec![n1, n2, n3];
        let expected = vec![n1, n3];

        // At K=1, only n1 is retrieved. Hits: 1 (n1). Total expected: 2. Recall = 0.5
        assert_eq!(compute_recall_at_k(&retrieved, &expected, 1), 0.5);
        // At K=3, both n1 and n3 are retrieved. Hits: 2. Recall = 1.0
        assert_eq!(compute_recall_at_k(&retrieved, &expected, 3), 1.0);
    }
}
