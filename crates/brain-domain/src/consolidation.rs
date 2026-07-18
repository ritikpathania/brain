//! Pure domain consolidation models, policies, and analytical logic.

use crate::entities::KnowledgeGraph;
use crate::identifiers::{EdgeId, NodeId};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Diagnostic errors for metric bounds validations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum MetricConstructionError {
    /// Error returned when a metric value is out of its expected bounds.
    #[error("Value {val} is out of expected range [{min}, {max}]")]
    OutOfRange {
        /// The invalid value.
        val: f64,
        /// Minimum allowed bound.
        min: f64,
        /// Maximum allowed bound.
        max: f64,
    },
    /// Error returned when a metric value is NaN or infinite.
    #[error("Value {val} is not finite")]
    NotFinite {
        /// The invalid non-finite value.
        val: f64,
    },
    /// Error returned when a duration calculation yields an invalid span.
    #[error("Duration seconds {secs} is invalid")]
    InvalidDuration {
        /// The invalid duration duration in seconds.
        secs: u64,
    },
}

impl PartialEq for MetricConstructionError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::OutOfRange {
                    val: v1,
                    min: min1,
                    max: max1,
                },
                Self::OutOfRange {
                    val: v2,
                    min: min2,
                    max: max2,
                },
            ) => {
                crate::retrieval::models::eq_f64(*v1, *v2)
                    && crate::retrieval::models::eq_f64(*min1, *min2)
                    && crate::retrieval::models::eq_f64(*max1, *max2)
            }
            (Self::NotFinite { val: v1 }, Self::NotFinite { val: v2 }) => {
                crate::retrieval::models::eq_f64(*v1, *v2)
            }
            (Self::InvalidDuration { secs: s1 }, Self::InvalidDuration { secs: s2 }) => s1 == s2,
            _ => false,
        }
    }
}

impl Eq for MetricConstructionError {}

/// A strongly-typed, validated wrapper representing a normalized node label similarity score.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SimilarityScore(f64);

impl SimilarityScore {
    /// Constructs a new `SimilarityScore` and asserts the value falls within `[0.0, 1.0]`.
    pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
        if !val.is_finite() {
            return Err(MetricConstructionError::NotFinite { val });
        }
        if !(0.0..=1.0).contains(&val) {
            return Err(MetricConstructionError::OutOfRange {
                val,
                min: 0.0,
                max: 1.0,
            });
        }
        Ok(Self(val))
    }

    /// Access the underlying score value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// A strongly-typed, validated wrapper representing a normalized edge promotion score.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PromotionScore(f64);

impl PromotionScore {
    /// Constructs a new `PromotionScore` and asserts the value falls within `[0.0, 1.0]`.
    pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
        if !val.is_finite() {
            return Err(MetricConstructionError::NotFinite { val });
        }
        if !(0.0..=1.0).contains(&val) {
            return Err(MetricConstructionError::OutOfRange {
                val,
                min: 0.0,
                max: 1.0,
            });
        }
        Ok(Self(val))
    }

    /// Access the underlying score value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// A strongly-typed, validated wrapper representing a normalized consolidation confidence score.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ConfidenceScore(f64);

impl ConfidenceScore {
    /// Constructs a new `ConfidenceScore` and asserts the value falls within `[0.0, 1.0]`.
    pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
        if !val.is_finite() {
            return Err(MetricConstructionError::NotFinite { val });
        }
        if !(0.0..=1.0).contains(&val) {
            return Err(MetricConstructionError::OutOfRange {
                val,
                min: 0.0,
                max: 1.0,
            });
        }
        Ok(Self(val))
    }

    /// Access the underlying score value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// A strongly-typed wrapper representing the elapsed seconds of inactivity for relationship elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StalenessAge(u64);

impl StalenessAge {
    /// Constructs a new `StalenessAge` wrapper from the given elapsed duration.
    pub fn new(duration: Duration) -> Result<Self, MetricConstructionError> {
        Ok(Self(duration.as_secs()))
    }

    /// Access the underlying staleness age in seconds.
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A generic packaging struct linking a target identity with its corresponding metric evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceEntry<ID, M> {
    /// The target object or collection identifier.
    pub target: ID,
    /// The associated evidence metric.
    pub metric: M,
}

/// An intermediate, immutable snapshot containing objective structural metrics and candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationAnalysis {
    /// Groups of duplicate node identifiers with label similarity scores.
    pub duplicate_node_groups: Vec<EvidenceEntry<Vec<NodeId>, SimilarityScore>>,
    /// Candidates for episodic-to-semantic promotion with promotion weights.
    pub promotion_candidates: Vec<EvidenceEntry<EdgeId, PromotionScore>>,
    /// Low-weight candidates recommended for archival.
    pub archival_candidates: Vec<EvidenceEntry<EdgeId, StalenessAge>>,
    /// Stale, unreinforced episodic relationships.
    pub stale_episodic_edges: Vec<EvidenceEntry<EdgeId, StalenessAge>>,
}

/// Lightweight operations referencing targets by identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsolidationActionType {
    /// Promotes an episodic relationship to a permanent semantic category (infinite validity).
    PromoteToSemantic {
        /// Target edge identity.
        edge_id: EdgeId,
    },
    /// Merges multiple redundant duplicate nodes into a canonical node.
    MergeNodes {
        /// The canonical destination node.
        canonical_node_id: NodeId,
        /// The redundant source nodes to merge.
        redundant_node_ids: Vec<NodeId>,
        /// The consolidated label representation.
        merged_label: String,
    },
    /// Moves a low-activity relationship to archival storage.
    ArchiveEdge {
        /// Target edge identity.
        edge_id: EdgeId,
    },
    /// Deletes a stale, unreinforced episodic relationship.
    PruneEdge {
        /// Target edge identity.
        edge_id: EdgeId,
    },
}

/// A metadata-carrying action directive containing rationale and confidence metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationAction {
    /// The consolidation command type.
    pub action: ConsolidationActionType,
    /// Descriptive explanation of why this action was decided.
    pub rationale: String,
    /// The confidence score of this decision.
    pub confidence: ConfidenceScore,
}

/// Policy configurations parameterizing maintenance and lifecycle transitions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationPolicy {
    /// Weight threshold above which relationships qualify for promotion.
    pub promotion_weight_threshold: f64,
    /// Weight threshold below which relationships are pruned or archived.
    pub pruning_weight_threshold: f64,
    /// Elapsed seconds after which an episodic relationship is considered stale.
    pub staleness_age_threshold_secs: u64,
}

/// Domain engine evaluating lifecycle sweeps and consolidating knowledge.
#[derive(Debug, Clone, Copy)]
pub struct Consolidator {
    /// Active policy rules.
    pub policy: ConsolidationPolicy,
}

impl Consolidator {
    /// Creates a new `Consolidator` with the given policy rules.
    pub fn new(policy: ConsolidationPolicy) -> Self {
        Self { policy }
    }

    /// Evaluates structural candidate evidence inside the knowledge graph.
    pub fn analyze(&self, graph: &KnowledgeGraph) -> ConsolidationAnalysis {
        // Internal Stage 1: Candidate Discovery
        let mut duplicate_node_groups = Vec::new();
        let mut groups: std::collections::HashMap<String, Vec<NodeId>> =
            std::collections::HashMap::new();
        for node in graph.nodes.values() {
            groups
                .entry(node.label.to_lowercase().trim().to_string())
                .or_default()
                .push(node.id);
        }
        for (label, ids) in groups {
            if ids.len() > 1 && !label.is_empty() {
                let mut sorted_ids = ids.clone();
                sorted_ids.sort();
                if let Ok(sim) = SimilarityScore::new(1.0) {
                    duplicate_node_groups.push(EvidenceEntry {
                        target: sorted_ids,
                        metric: sim,
                    });
                }
            }
        }

        let mut promotion_candidates = Vec::new();
        let mut archival_candidates = Vec::new();
        let mut stale_episodic_edges = Vec::new();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Internal Stage 2: Policy Bounds Filtering
        for edge in graph.edges.values() {
            let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());

            if edge.weight >= self.policy.promotion_weight_threshold && edge.weight < 1.0 {
                if let Ok(score) = PromotionScore::new(edge.weight) {
                    promotion_candidates.push(EvidenceEntry {
                        target: edge_id.clone(),
                        metric: score,
                    });
                }
            }

            if edge.weight < self.policy.pruning_weight_threshold {
                let age = current_time.saturating_sub(edge.updated_at);
                if let Ok(staleness) = StalenessAge::new(Duration::from_secs(age)) {
                    archival_candidates.push(EvidenceEntry {
                        target: edge_id.clone(),
                        metric: staleness,
                    });
                }
            }

            let age = current_time.saturating_sub(edge.updated_at);
            if age >= self.policy.staleness_age_threshold_secs {
                if let Ok(staleness) = StalenessAge::new(Duration::from_secs(age)) {
                    stale_episodic_edges.push(EvidenceEntry {
                        target: edge_id,
                        metric: staleness,
                    });
                }
            }
        }

        // Sort lists deterministically to preserve order invariants
        duplicate_node_groups.sort_by(|a, b| a.target.cmp(&b.target));
        promotion_candidates.sort_by(|a, b| a.target.cmp(&b.target));
        archival_candidates.sort_by(|a, b| a.target.cmp(&b.target));
        stale_episodic_edges.sort_by(|a, b| a.target.cmp(&b.target));

        ConsolidationAnalysis {
            duplicate_node_groups,
            promotion_candidates,
            archival_candidates,
            stale_episodic_edges,
        }
    }

    /// Plans final actions based on structural analysis evidence.
    pub fn plan(&self, analysis: ConsolidationAnalysis) -> Vec<ConsolidationAction> {
        let mut actions = Vec::new();

        for entry in analysis.duplicate_node_groups {
            let ids = entry.target;
            if ids.len() < 2 {
                continue;
            }
            let canonical = ids[0];
            let redundant = ids[1..].to_vec();
            if let Ok(conf) = ConfidenceScore::new(entry.metric.value()) {
                actions.push(ConsolidationAction {
                    action: ConsolidationActionType::MergeNodes {
                        canonical_node_id: canonical,
                        redundant_node_ids: redundant,
                        merged_label: format!("Merged node {}", canonical),
                    },
                    rationale: format!(
                        "Consolidating duplicate nodes based on label similarity (score: {:.2})",
                        entry.metric.value()
                    ),
                    confidence: conf,
                });
            }
        }

        for entry in analysis.promotion_candidates {
            if let Ok(conf) = ConfidenceScore::new(entry.metric.value()) {
                actions.push(ConsolidationAction {
                    action: ConsolidationActionType::PromoteToSemantic { edge_id: entry.target.clone() },
                    rationale: format!(
                        "Promoting highly reinforced episodic relationship to semantic category (score: {:.2})",
                        entry.metric.value()
                    ),
                    confidence: conf,
                });
            }
        }

        for entry in analysis.archival_candidates {
            if let Ok(conf) = ConfidenceScore::new(1.0) {
                actions.push(ConsolidationAction {
                    action: ConsolidationActionType::ArchiveEdge { edge_id: entry.target.clone() },
                    rationale: format!(
                        "Archiving relationship due to low active weight (staleness age: {} seconds)",
                        entry.metric.value()
                    ),
                    confidence: conf,
                });
            }
        }

        let archived_set: std::collections::HashSet<_> = actions
            .iter()
            .filter_map(|act| {
                if let ConsolidationActionType::ArchiveEdge { edge_id } = &act.action {
                    Some(edge_id.clone())
                } else {
                    None
                }
            })
            .collect();

        for entry in analysis.stale_episodic_edges {
            if archived_set.contains(&entry.target) {
                continue;
            }
            if let Ok(conf) = ConfidenceScore::new(0.8) {
                actions.push(ConsolidationAction {
                    action: ConsolidationActionType::PruneEdge {
                        edge_id: entry.target.clone(),
                    },
                    rationale: format!(
                        "Pruning stale unreinforced relationship (staleness age: {} seconds)",
                        entry.metric.value()
                    ),
                    confidence: conf,
                });
            }
        }

        // Sort planned actions deterministically to ensure action ordering determinism
        actions.sort_by(|a, b| {
            let key_a = format!("{:?}", a.action);
            let key_b = format!("{:?}", b.action);
            key_a.cmp(&key_b)
        });

        actions
    }
}
