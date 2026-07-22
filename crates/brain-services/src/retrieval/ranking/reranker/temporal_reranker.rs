use brain_core::errors::BrainError;
use brain_core::retrieval::{DecayModel, RerankContext, Reranker};
use brain_domain::Node;

/// A reranker that applies temporal recency decay to post-fusion candidate lists.
#[derive(Debug, Clone, Default)]
pub struct TemporalReranker;

impl TemporalReranker {
    /// Creates a new `TemporalReranker`.
    pub fn new() -> Self {
        Self
    }
}

impl Reranker for TemporalReranker {
    fn rerank(
        &self,
        candidates: Vec<Node>,
        context: &RerankContext<'_>,
    ) -> Result<Vec<Node>, BrainError> {
        if !context.config.enabled || candidates.is_empty() {
            return Ok(candidates);
        }

        let mut scored_nodes: Vec<(Node, f64)> = candidates
            .into_iter()
            .enumerate()
            .map(|(i, node)| {
                // S_RRF = 1.0 / (60.0 + rank)
                let rank = (i + 1) as f64;
                let base_score = 1.0 / (60.0 + rank);

                let dt = context.reference_time.saturating_sub(node.updated_at);

                let raw_decay = match context.config.model {
                    DecayModel::Exponential => {
                        let half_life = if context.config.half_life_seconds > 0 {
                            context.config.half_life_seconds as f64
                        } else {
                            86400.0
                        };
                        let lambda = f64::ln(2.0) / half_life;
                        f64::exp(-lambda * (dt as f64))
                    }
                    DecayModel::Logarithmic => 1.0 / (1.0 + f64::ln(1.0 + (dt as f64))),
                    DecayModel::Linear => {
                        let w = if context.config.half_life_seconds > 0 {
                            context.config.half_life_seconds as f64
                        } else {
                            86400.0
                        };
                        f64::max(0.0, 1.0 - (dt as f64) / w)
                    }
                    DecayModel::Uniform => 1.0,
                };

                let scaling = context.config.scaling_factor.clamp(0.0, 1.0);
                let decay = 1.0 - scaling * (1.0 - raw_decay);
                let final_score = base_score * decay;

                (node, final_score)
            })
            .collect();

        // Sort by score descending, with lexicographical NodeId tie-breaker
        scored_nodes.sort_by(|(node_a, score_a), (node_b, score_b)| {
            score_b
                .partial_cmp(score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| node_a.id.0.cmp(&node_b.id.0))
        });

        let final_nodes = scored_nodes.into_iter().map(|(node, _)| node).collect();
        Ok(final_nodes)
    }
}
