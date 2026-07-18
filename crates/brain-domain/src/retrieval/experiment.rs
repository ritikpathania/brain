use crate::consolidation::MetricConstructionError;
use crate::retrieval::models::WeightSnapshot;

/// Validation errors for experiment configuration.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExperimentValidationError {
    /// Wrapping metric construction error.
    #[error("Metric error: {0}")]
    MetricError(#[from] MetricConstructionError),
    /// Allocation sum doesn't equal exactly 1.0.
    #[error("Allocation sum is {sum}, but must sum to exactly 1.0 (epsilon: 1e-9)")]
    InvalidAllocationSum {
        /// Computed sum.
        sum: f64,
    },
    /// Duplicate variant ID found.
    #[error("Duplicate variant ID: {variant_id}")]
    DuplicateVariantId {
        /// Duplicate ID string.
        variant_id: String,
    },
    /// No variants specified.
    #[error("Configuration has no variants")]
    EmptyVariants,
}

/// Holds a validated traffic allocation percentage in range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrafficAllocation(f64);

impl TrafficAllocation {
    /// Creates a new validated `TrafficAllocation`.
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

    /// Accesses the allocation value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// Explicit variant binding an identifier to a snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Variant {
    /// Unique variant identifier.
    pub id: String,
    /// Multitask weights configuration snapshot.
    pub snapshot: WeightSnapshot,
}

/// Identifiers for routing traffic.
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum RoutingKey {
    /// Stable session ID key.
    SessionId(String),
    /// Stable user ID key.
    UserId(String),
    /// Request-level key.
    RequestId(String),
    /// Fallback/static default routing.
    Constant,
}

/// Target algorithms defining how allocations are distributed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RoutingStrategy {
    /// Sticky FNV-1a hash allocation.
    StickyHashRouting,
}

/// Domain configurations governing experiment routing tables.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExperimentConfiguration {
    /// Unique experiment identifier.
    pub id: String,
    /// Version of the experiment configuration.
    pub version: u64,
    /// Registered variants.
    pub variants: Vec<Variant>,
    /// Allocation ratios mapping variant IDs to target sizes.
    pub allocations: Vec<(String, TrafficAllocation)>,
    /// Configured routing strategy.
    pub routing_strategy: RoutingStrategy,
}

impl ExperimentConfiguration {
    /// Creates and validates a new `ExperimentConfiguration`.
    pub fn new(
        id: String,
        version: u64,
        variants: Vec<Variant>,
        allocations: Vec<(String, TrafficAllocation)>,
        routing_strategy: RoutingStrategy,
    ) -> Result<Self, ExperimentValidationError> {
        if variants.is_empty() {
            return Err(ExperimentValidationError::EmptyVariants);
        }

        // Validate Allocation Conservation: Sum of allocations must be exactly 1.0 (epsilon: 1e-9)
        let sum: f64 = allocations.iter().map(|(_, a)| a.value()).sum();
        if (sum - 1.0).abs() > 1e-9 {
            return Err(ExperimentValidationError::InvalidAllocationSum { sum });
        }

        // Validate duplicates and check that variant IDs match allocations
        let mut seen = std::collections::HashSet::new();
        for variant in &variants {
            if !seen.insert(&variant.id) {
                return Err(ExperimentValidationError::DuplicateVariantId {
                    variant_id: variant.id.clone(),
                });
            }
        }

        Ok(Self {
            id,
            version,
            variants,
            allocations,
            routing_strategy,
        })
    }
}

/// Telemetry report capturing evaluation metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingDecision {
    /// Selected weight snapshot.
    pub snapshot: WeightSnapshot,
    /// Selected variant identifier.
    pub variant_id: String,
    /// Experiment context identifier.
    pub experiment_id: String,
    /// Experiment configuration version.
    pub experiment_version: u64,
    /// Descriptive justification.
    pub reason: String,
}
