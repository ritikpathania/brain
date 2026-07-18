use brain_domain::NodeId;
use std::collections::HashSet;

/// Computes Precision@K: fraction of retrieved top K nodes that are in expected or acceptable.
pub fn compute_precision_at_k(
    retrieved: &[NodeId],
    expected: &[NodeId],
    acceptable: &[NodeId],
    k: usize,
) -> f64 {
    let limit = std::cmp::min(k, retrieved.len());
    if limit == 0 {
        return 0.0;
    }
    let mut relevant_set = HashSet::new();
    for id in expected {
        relevant_set.insert(id);
    }
    for id in acceptable {
        relevant_set.insert(id);
    }

    let mut hits = 0;
    for node_id in &retrieved[..limit] {
        if relevant_set.contains(node_id) {
            hits += 1;
        }
    }
    hits as f64 / limit as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precision_empty() {
        let expected = vec![NodeId::new()];
        assert_eq!(compute_precision_at_k(&[], &expected, &[], 5), 0.0);
    }

    #[test]
    fn test_precision_happy() {
        let n1 = NodeId::new();
        let n2 = NodeId::new();
        let n3 = NodeId::new();
        let retrieved = vec![n1, n2, n3];
        let expected = vec![n1];
        let acceptable = vec![n3];

        // At K=1, retrieved: [n1]. Relevant: {n1, n3}. Hit: n1. Precision = 1 / 1 = 1.0
        assert_eq!(
            compute_precision_at_k(&retrieved, &expected, &acceptable, 1),
            1.0
        );
        // At K=2, retrieved: [n1, n2]. Hit: n1. Precision = 1 / 2 = 0.5
        assert_eq!(
            compute_precision_at_k(&retrieved, &expected, &acceptable, 2),
            0.5
        );
        // At K=3, retrieved: [n1, n2, n3]. Hits: n1, n3. Precision = 2 / 3 = 0.6666...
        let prec3 = compute_precision_at_k(&retrieved, &expected, &acceptable, 3);
        assert!((prec3 - 2.0 / 3.0).abs() < 1e-9);
    }
}
