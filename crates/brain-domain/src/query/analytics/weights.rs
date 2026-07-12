use crate::entities::Edge;

/// Trait defining cost weight lookup for graph edges during traversal/routing.
pub trait EdgeWeightProvider {
    /// Returns the positive traversal weight/distance cost for an edge.
    fn weight(&self, edge: &Edge) -> f64;
}

/// Uniform weighting provider returning constant cost (1.0) for every edge.
#[derive(Debug, Clone, Default)]
pub struct UniformWeightProvider;

impl EdgeWeightProvider for UniformWeightProvider {
    fn weight(&self, _edge: &Edge) -> f64 {
        1.0
    }
}

/// Confidence-based distance provider mapping high edge weights (confidence) to low distance costs.
/// Distance = 1.0 / weight
#[derive(Debug, Clone, Default)]
pub struct ConfidenceDistanceProvider;

impl EdgeWeightProvider for ConfidenceDistanceProvider {
    fn weight(&self, edge: &Edge) -> f64 {
        if edge.weight <= 0.0 {
            f64::INFINITY
        } else {
            1.0 / edge.weight
        }
    }
}
