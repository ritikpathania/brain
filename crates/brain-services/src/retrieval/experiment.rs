use std::sync::Arc;
use brain_core::errors::BrainError;
use brain_core::retrieval::RetrievalRequest;
use brain_domain::retrieval::experiment::{
    ExperimentConfiguration, RoutingDecision
};
use crate::retrieval::active_weights::ActiveWeightProvider;

/// Computes FNV-1a 64-bit hash.
pub fn fnv1a_hash(data: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Interface for routing retrieval requests to appropriate weight snapshots.
pub trait ExperimentRouter: Send + Sync {
    /// Dynamically routes the request and yields a detailed `RoutingDecision`.
    fn route_decision(&self, request: &RetrievalRequest) -> Result<RoutingDecision, BrainError>;
}

/// Default fallback router routing all traffic to the active baseline snapshot.
pub struct DefaultExperimentRouter {
    provider: Arc<dyn ActiveWeightProvider>,
}

impl DefaultExperimentRouter {
    /// Creates a new `DefaultExperimentRouter`.
    pub fn new(provider: Arc<dyn ActiveWeightProvider>) -> Self {
        Self { provider }
    }
}

impl ExperimentRouter for DefaultExperimentRouter {
    fn route_decision(&self, _request: &RetrievalRequest) -> Result<RoutingDecision, BrainError> {
        let snapshot = self.provider.active_snapshot()?;
        Ok(RoutingDecision {
            snapshot: (*snapshot).clone(),
            variant_id: "baseline".to_string(),
            experiment_id: "default".to_string(),
            experiment_version: 0,
            reason: "Routed to default active snapshot".to_string(),
        })
    }
}

/// Canary router implementing multi-variant sticky FNV-1a hash routing.
pub struct CanaryExperimentRouter {
    baseline_provider: Arc<dyn ActiveWeightProvider>,
    config: ExperimentConfiguration,
}

impl CanaryExperimentRouter {
    /// Creates a new `CanaryExperimentRouter`.
    pub fn new(baseline_provider: Arc<dyn ActiveWeightProvider>, config: ExperimentConfiguration) -> Self {
        Self { baseline_provider, config }
    }
}

impl ExperimentRouter for CanaryExperimentRouter {
    fn route_decision(&self, request: &RetrievalRequest) -> Result<RoutingDecision, BrainError> {
        let session_str = request.session_id.to_string();
        // Fallback to baseline if no stable routing key is present (Deterministic default for nil UUID/ULID)
        if session_str.is_empty() || session_str == "00000000000000000000000000" {
            let snapshot = self.baseline_provider.active_snapshot()?;
            return Ok(RoutingDecision {
                snapshot: (*snapshot).clone(),
                variant_id: "baseline".to_string(),
                experiment_id: self.config.id.clone(),
                experiment_version: self.config.version,
                reason: "No stable session key; routed to baseline".to_string(),
            });
        }

        // Compute FNV-1a hash
        let hash_val = fnv1a_hash(&session_str);
        let fraction = (hash_val % 10000) as f64 / 10000.0;

        // Route according to cumulative traffic allocations
        let mut cumulative = 0.0;
        for (var_id, allocation) in &self.config.allocations {
            cumulative += allocation.value();
            if fraction < cumulative {
                if let Some(variant) = self.config.variants.iter().find(|v| &v.id == var_id) {
                    return Ok(RoutingDecision {
                        snapshot: variant.snapshot.clone(),
                        variant_id: var_id.clone(),
                        experiment_id: self.config.id.clone(),
                        experiment_version: self.config.version,
                        reason: format!("Routed to variant {} via sticky session hash ({:.4})", var_id, fraction),
                    });
                }
            }
        }

        // Fallback to active baseline if something overflows or falls outside allocation range
        let snapshot = self.baseline_provider.active_snapshot()?;
        Ok(RoutingDecision {
            snapshot: (*snapshot).clone(),
            variant_id: "baseline".to_string(),
            experiment_id: self.config.id.clone(),
            experiment_version: self.config.version,
            reason: "Exceeded allocation range; routed to baseline".to_string(),
        })
    }
}
