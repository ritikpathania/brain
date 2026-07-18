use crate::retrieval::models::Evidence;
use crate::temporal::{RecencyPolicy, TemporalEdge, TemporalQuery, TimePoint};

/// Evaluates and encapsulates a recency decay calculation context, ensuring consistent derived metrics by construction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RecencyEvaluation {
    /// Recency policy configuration.
    pub policy: RecencyPolicy,
    /// Edge observation timestamp.
    pub observed_at: TimePoint,
    /// Reference evaluation timestamp.
    pub reference_time: TimePoint,
    /// Calculated elapsed duration in seconds.
    pub elapsed_seconds: f64,
    /// Calculated decayed factor.
    pub decay_factor: f64,
}

impl RecencyEvaluation {
    /// Computes and returns a verified consistent `RecencyEvaluation`.
    pub fn new(policy: RecencyPolicy, observed_at: TimePoint, reference_time: TimePoint) -> Self {
        let elapsed = reference_time
            .unix_seconds()
            .saturating_sub(observed_at.unix_seconds()) as f64;
        let decay_factor = policy.compute_weight(1.0, observed_at, reference_time);
        Self {
            policy,
            observed_at,
            reference_time,
            elapsed_seconds: elapsed,
            decay_factor,
        }
    }
}

/// Stateless builder generating structured, append-only explainability evidence for temporal graph queries.
pub struct HistoricalExplanationBuilder;

impl HistoricalExplanationBuilder {
    /// Constructs visibility explainability evidence for a temporal projected edge.
    pub fn build_visibility_evidence(edge: &TemporalEdge, query: &TemporalQuery) -> Evidence {
        Evidence::TemporalVisibility {
            observed_at: edge.observed_at,
            validity_intervals: edge.validity.intervals().to_vec(),
            query_time: query.reference_time,
            visibility_mode: query.visibility,
        }
    }

    /// Constructs recency decay explainability evidence from a computed evaluation context.
    pub fn build_decay_evidence(eval: &RecencyEvaluation) -> Evidence {
        Evidence::RecencyDecay {
            policy: eval.policy,
            observed_at: eval.observed_at,
            reference_time: eval.reference_time,
            elapsed_seconds: eval.elapsed_seconds,
            decay_factor: eval.decay_factor,
        }
    }
}
