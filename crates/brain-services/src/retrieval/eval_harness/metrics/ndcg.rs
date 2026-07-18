use brain_domain::NodeId;
use std::collections::HashSet;

/// Computes Normalized Discounted Cumulative Gain (nDCG@K).
/// Relevance scores:
/// - 1.0 if the retrieved node is in `expected` (primary targets)
/// - 0.5 if the retrieved node is in `acceptable` (alternative matches)
/// - 0.0 otherwise.
pub fn compute_ndcg_at_k(
    retrieved: &[NodeId],
    expected: &[NodeId],
    acceptable: &[NodeId],
    k: usize,
) -> f64 {
    if retrieved.is_empty() || expected.is_empty() || k == 0 {
        return 0.0;
    }

    let expected_set: HashSet<&NodeId> = expected.iter().collect();
    let acceptable_set: HashSet<&NodeId> = acceptable.iter().collect();

    // 1. Compute DCG@K
    let mut dcg = 0.0;
    let limit = std::cmp::min(retrieved.len(), k);
    for i in 0..limit {
        let node_id = &retrieved[i];
        let relevance = if expected_set.contains(node_id) {
            1.0
        } else if acceptable_set.contains(node_id) {
            0.5
        } else {
            0.0
        };

        if relevance > 0.0 {
            dcg += relevance / ((i + 2) as f64).log2();
        }
    }

    // 2. Compute IDCG@K (Ideal DCG@K)
    // Sort all available relevant items in descending order of their relevance.
    let mut all_relevances = Vec::new();
    for _ in expected {
        all_relevances.push(1.0);
    }
    for _ in acceptable {
        all_relevances.push(0.5);
    }
    all_relevances.sort_by(|a, b| b.partial_cmp(a).unwrap());

    let mut idcg = 0.0;
    let ideal_limit = std::cmp::min(all_relevances.len(), k);
    for i in 0..ideal_limit {
        idcg += all_relevances[i] / ((i + 2) as f64).log2();
    }

    if idcg == 0.0 {
        return 0.0;
    }

    dcg / idcg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ndcg_empty() {
        let expected = vec![NodeId::new()];
        assert_eq!(compute_ndcg_at_k(&[], &expected, &[], 5), 0.0);
    }

    #[test]
    fn test_ndcg_happy() {
        let n1 = NodeId::new();
        let n2 = NodeId::new();
        let n3 = NodeId::new();

        let retrieved = vec![n1, n2, n3];
        // expected: n1 (1.0), acceptable: n2 (0.5), n3 is other (0.0)
        // DCG@3 = 1.0/log2(2) + 0.5/log2(3) + 0.0/log2(4) = 1.0 + 0.5/1.5849625 = 1.0 + 0.31546487 = 1.31546487
        // IDCG@3 = 1.0/log2(2) + 0.5/log2(3) = 1.31546487
        // nDCG@3 = 1.0
        let ndcg = compute_ndcg_at_k(&retrieved, &vec![n1], &vec![n2], 3);
        assert!((ndcg - 1.0).abs() < 1e-9);

        // If retrieved order is swapped: n2, n1, n3
        // DCG@3 = 0.5/log2(2) + 1.0/log2(3) = 0.5 + 1.0/1.5849625 = 0.5 + 0.6309297 = 1.1309297
        // nDCG@3 = 1.1309297 / 1.31546487 = 0.8597186
        let ndcg2 = compute_ndcg_at_k(&vec![n2, n1, n3], &vec![n1], &vec![n2], 3);
        assert!((ndcg2 - 0.859718685).abs() < 1e-6);
    }
}
