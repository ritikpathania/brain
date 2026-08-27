//! Default deterministic memory ranker, temporal decay evaluator, and channel deduplicator.

use brain_core::context::{MemoryRanker, RankedMemory, RetrievalCandidate, RetrievalChannel};
use brain_domain::NodeId;
use std::collections::HashMap;

/// Default implementation of `MemoryRanker`.
#[derive(Debug, Clone, Default)]
pub struct DefaultMemoryRanker {
    /// Half-life in seconds for temporal decay calculation (default: 7 days = 604,800s).
    pub half_life_secs: f64,
}

impl DefaultMemoryRanker {
    /// Creates a new `DefaultMemoryRanker` with default 7-day half-life.
    pub fn new() -> Self {
        Self {
            half_life_secs: 604_800.0,
        }
    }

    /// Creates a ranker with custom half-life.
    pub fn with_half_life(half_life_secs: f64) -> Self {
        Self { half_life_secs }
    }

    /// Calculates temporal decay multiplier between candidate timestamp and reference time.
    fn calculate_temporal_decay(&self, timestamp: u64, reference_time: u64) -> f64 {
        if reference_time <= timestamp || self.half_life_secs <= 0.0 {
            return 1.0;
        }
        let age_secs = (reference_time - timestamp) as f64;
        // Exponential decay: e^(-ln(2) * age / half_life)
        let decay = (-std::f64::consts::LN_2 * age_secs / self.half_life_secs).exp();
        decay.clamp(0.01, 1.0)
    }
}

impl MemoryRanker for DefaultMemoryRanker {
    fn rank_and_deduplicate(
        &self,
        candidates: Vec<RetrievalCandidate>,
        reference_time: u64,
    ) -> Vec<RankedMemory> {
        if candidates.is_empty() {
            return Vec::new();
        }

        // 1. Group candidates by NodeId to deduplicate across channels (Lexical, Semantic, Graph)
        let mut grouped: HashMap<NodeId, Vec<RetrievalCandidate>> = HashMap::new();
        for candidate in candidates {
            grouped
                .entry(candidate.node_id)
                .or_default()
                .push(candidate);
        }

        let mut ranked_items: Vec<RankedMemory> = Vec::with_capacity(grouped.len());

        for (node_id, mut items) in grouped {
            // Find highest raw score item among channels
            items.sort_by(|a, b| b.raw_score.total_cmp(&a.raw_score));
            let primary = &items[0];

            let decay = self.calculate_temporal_decay(primary.timestamp, reference_time);

            // Scope tier multiplier: Session (1.10x) > Workspace (1.05x) > Global (1.00x)
            let scope_multiplier = match primary.scope {
                brain_core::context::MemoryScope::Session(_) => 1.10,
                brain_core::context::MemoryScope::Workspace(_) => 1.05,
                brain_core::context::MemoryScope::Global => 1.00,
            };

            let mut final_score = (primary.raw_score * decay * scope_multiplier).clamp(0.0, 1.0);

            // Multi-channel fusion bonus: small boost if discovered via multiple channels
            if items.len() > 1 {
                let channel_bonus = 0.05 * (items.len() - 1) as f64;
                final_score = (final_score + channel_bonus).min(1.0);
            }

            let channel = if items.len() > 1 {
                RetrievalChannel::Hybrid
            } else {
                primary.channel
            };

            ranked_items.push(RankedMemory {
                node_id,
                label: primary.label.clone(),
                content: primary.content.clone(),
                channel,
                raw_score: primary.raw_score,
                final_score,
                timestamp: primary.timestamp,
                scope: primary.scope.clone(),
                metadata: primary.metadata.clone(),
            });
        }

        // 2. Deterministic sorting contract:
        //    1) final_score DESC (using total_cmp for strict IEEE 754 ordering)
        //    2) timestamp DESC
        //    3) node_id ASC
        ranked_items.sort_by(|a, b| {
            b.final_score
                .total_cmp(&a.final_score)
                .then_with(|| b.timestamp.cmp(&a.timestamp))
                .then_with(|| a.node_id.to_string().cmp(&b.node_id.to_string()))
        });

        ranked_items
    }
}
