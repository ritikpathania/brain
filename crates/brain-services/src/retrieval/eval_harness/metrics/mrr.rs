use brain_domain::NodeId;
use std::collections::HashSet;

/// Computes Mean Reciprocal Rank (MRR): reciprocal rank of the first relevant retrieved candidate.
pub fn compute_mrr(retrieved: &[NodeId], expected: &[NodeId], acceptable: &[NodeId]) -> f64 {
    let mut relevant_set = HashSet::new();
    for id in expected {
        relevant_set.insert(id);
    }
    for id in acceptable {
        relevant_set.insert(id);
    }

    for (idx, node_id) in retrieved.iter().enumerate() {
        if relevant_set.contains(node_id) {
            return 1.0 / (idx + 1) as f64;
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mrr_empty() {
        let expected = vec![NodeId::new()];
        assert_eq!(compute_mrr(&[], &expected, &[]), 0.0);
    }

    #[test]
    fn test_mrr_happy() {
        let n1 = NodeId::new();
        let n2 = NodeId::new();
        let n3 = NodeId::new();
        let retrieved = vec![n1, n2, n3];
        
        // Expected is n2. Index is 1 (2nd item). MRR = 0.5
        assert_eq!(compute_mrr(&retrieved, &vec![n2], &[]), 0.5);
        // Expected is n3. Index is 2 (3rd item). MRR = 0.3333333333333333
        let mrr3 = compute_mrr(&retrieved, &vec![n3], &[]);
        assert!((mrr3 - 1.0/3.0).abs() < 1e-9);
    }
}
