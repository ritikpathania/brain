use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_domain::{FindingEvidence, ReflectionFinding, Normalizer};
use crate::reflection::{ReflectionContext, ReflectionPass};

/// Analysis pass to identify potential duplicate concept nodes in the graph.
pub struct DuplicateDetectionPass;

impl DuplicateDetectionPass {
    /// Creates a new `DuplicateDetectionPass`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionPass for DuplicateDetectionPass {
    fn run(
        &self,
        snapshot: &dyn RepositorySet,
        context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError> {
        let mut nodes = snapshot.nodes().list_all()?;
        
        // Truncate to obey maximum node capacity limits in the context
        if nodes.len() > context.max_nodes {
            nodes.truncate(context.max_nodes);
        }

        let mut findings = Vec::new();

        for i in 0..nodes.len() {
            // Check cancellation token
            if context.cancellation_token.is_cancelled() {
                return Err(BrainError::Validation {
                    message: "Reflection duplicate pass aborted by cancellation token".to_string(),
                });
            }

            for j in (i + 1)..nodes.len() {
                let node_a = &nodes[i];
                let node_b = &nodes[j];

                // Ensure we only compare matching node types (e.g., Concepts)
                if node_a.node_type != node_b.node_type {
                    continue;
                }

                let label_a = Normalizer::normalize(&node_a.label);
                let label_b = Normalizer::normalize(&node_b.label);

                // 1. Calculate syntactic similarity using Levenshtein distance
                let syntactic_sim = if label_a == label_b {
                    1.0
                } else {
                    let edit_dist = levenshtein_distance(&label_a, &label_b);
                    let max_len = std::cmp::max(label_a.chars().count(), label_b.chars().count());
                    if max_len > 0 {
                        1.0 - (edit_dist as f64 / max_len as f64)
                    } else {
                        0.0
                    }
                };

                // Fast pruning: skip pairs with very low syntactic similarity
                if syntactic_sim < 0.6 {
                    continue;
                }

                // 2. Fetch embeddings for semantic similarity comparison
                let emb_a = snapshot.embeddings().find_by_node_id(&node_a.id)?;
                let emb_b = snapshot.embeddings().find_by_node_id(&node_b.id)?;

                let (semantic_sim, confidence) = match (emb_a, emb_b) {
                    (Some(e_a), Some(e_b)) => {
                        let sim = cosine_similarity(&e_a.vector, &e_b.vector);
                        // Blend syntactic & semantic scores equally
                        let blended = 0.5 * syntactic_sim + 0.5 * sim;
                        (Some(sim), blended)
                    }
                    _ => (None, syntactic_sim),
                };

                // 3. Register finding if combined confidence score meets the threshold
                if confidence >= 0.85 {
                    let details = format!(
                        "Syntactic similarity: {:.2}%, Semantic similarity: {}",
                        syntactic_sim * 100.0,
                        semantic_sim.map(|s| format!("{:.2}%", s * 100.0)).unwrap_or_else(|| "N/A".to_string())
                    );

                    let edit_dist = levenshtein_distance(&label_a, &label_b);
                    
                    findings.push(ReflectionFinding::DuplicateFound {
                        node_a: node_a.id,
                        node_b: node_b.id,
                        evidence: FindingEvidence {
                            confidence,
                            semantic_similarity: semantic_sim,
                            edit_distance: Some(edit_dist),
                            overlap_ratio: None,
                            details,
                        },
                    });
                }
            }
        }

        Ok(findings)
    }
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();
    
    if len1 == 0 { return len2; }
    if len2 == 0 { return len1; }
    
    let mut dp = vec![0; len2 + 1];
    for j in 0..=len2 {
        dp[j] = j;
    }
    
    for i in 1..=len1 {
        let mut prev = dp[0];
        dp[0] = i;
        for j in 1..=len2 {
            let temp = dp[j];
            if v1[i-1] == v2[j-1] {
                dp[j] = prev;
            } else {
                dp[j] = 1 + std::cmp::min(
                    prev,
                    std::cmp::min(dp[j], dp[j-1])
                );
            }
            prev = temp;
        }
    }
    dp[len2]
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f64 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (a, b) in v1.iter().zip(v2.iter()) {
        dot += (a * b) as f64;
        norm_a += (a * a) as f64;
        norm_b += (b * b) as f64;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}
