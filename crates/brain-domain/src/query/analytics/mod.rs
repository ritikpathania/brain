use crate::identifiers::NodeId;

/// A* Search pathfinder.
pub mod astar;
/// Centrality degree score solver.
pub mod centrality;
/// Closeness Centrality score solver.
pub mod closeness;
/// Connected components algorithm solver.
pub mod components;
/// Connectivity diagnostics (bridges, articulation points).
pub mod connectivity;
/// Lazy GraphAnalyticsContext and index abstractions.
pub mod context;
/// Cycle detection solver.
pub mod cycles;
/// Relation distribution analyzer.
pub mod distribution;
/// Heuristic provider traits for A*.
pub mod heuristic;
/// Central stable canonical sorting utilities.
pub mod ordering;
/// Centrality power-iteration PageRank solver.
pub mod pagerank;
/// Provenance statistics analyzer.
pub mod provenance;
/// Centralized analytical output result structures.
pub mod results;
/// Tarjan strongly connected components solver.
pub mod scc;
/// Dijkstra shortest path routing solver.
pub mod shortest_path;
/// Lightweight traversal utility helpers.
pub mod traversal;
/// Edge weighting and routing distance cost providers.
pub mod weights;

pub use astar::{AStar, AStarConfig};
pub use centrality::{Centrality, CentralityConfig};
pub use closeness::{Closeness, ClosenessConfig, ClosenessVariant};
pub use components::{ConnectedComponents, ConnectedComponentsConfig};
pub use connectivity::{Connectivity, ConnectivityConfig};
pub use context::{AdjacencyIndex, DegreeIndex, GraphAnalyticsContext, ReverseAdjacencyIndex};
pub use cycles::{CycleDetectionConfig, CycleDetector};
pub use distribution::{Distribution, DistributionConfig};
pub use heuristic::{HeuristicProvider, ZeroHeuristic};
pub use pagerank::{PageRank, PageRankConfig};
pub use provenance::{ProvenanceConfig, ProvenanceStatistics};
pub use results::{
    ClosenessResult, ConnectivityReport, DegreeCentrality, PageRankResult, ProvenanceStats,
    RelationDistribution, StronglyConnectedComponent,
};
pub use scc::{SccConfig, StronglyConnectedComponents};
pub use shortest_path::{ShortestPath, ShortestPathConfig};
pub use traversal::GraphTraversal;
pub use weights::{ConfidenceDistanceProvider, EdgeWeightProvider, UniformWeightProvider};

/// Trait defining the contract for shortest-path routing algorithms.
pub trait RoutingAlgorithm<'a, 'b> {
    /// Configuration parameter value object.
    type Config;
    /// The computed path result.
    type Result;

    /// Computes the optimal path between the source and target nodes.
    fn compute(&self, source: NodeId, target: NodeId) -> Self::Result;
}

/// Complexity class designation of analytical algorithms.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Complexity {
    /// Constant time complexity O(1)
    Constant,
    /// Logarithmic time complexity O(log N)
    LogN,
    /// Linear time complexity O(N)
    Linear,
    /// Linear-logarithmic time complexity O(N log N)
    LinearLog,
    /// Quadratic time complexity O(N^2)
    Quadratic,
    /// Cubic time complexity O(N^3)
    Cubic,
    /// Polynomial time complexity
    Polynomial,
    /// Exponential time complexity
    Exponential,
    /// Other/unspecified time complexity
    Other,
    /// Unknown/unmeasured time complexity
    Unknown,
}

/// Trait defining the contract for graph analytics algorithms.
pub trait AnalyticsAlgorithm<'a, 'b> {
    /// The computed output structure of the algorithm.
    type Output;

    /// The unique identifier of the algorithm.
    fn algorithm_id(&self) -> &'static str;

    /// The time complexity class of the algorithm.
    fn complexity(&self) -> Complexity;

    /// Executes the analytical computation over the graph.
    fn compute(&self) -> Self::Output;
}

/// Façade orchestrating modular analytical solver runs.
pub struct AnalyticsFacade;

impl AnalyticsFacade {
    /// Finds connected components in the graph.
    pub fn find_connected_components<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: ConnectedComponentsConfig,
    ) -> Vec<Vec<NodeId>> {
        ConnectedComponents::new(context, config).compute()
    }

    /// Calculates degree centrality score for all nodes.
    pub fn calculate_degree_centrality<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: CentralityConfig,
    ) -> Vec<DegreeCentrality> {
        Centrality::new(context, config).compute()
    }

    /// Computes relation distributions.
    pub fn relation_distribution<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: DistributionConfig,
    ) -> Vec<RelationDistribution> {
        Distribution::new(context, config).compute()
    }

    /// Computes provenance statistics.
    pub fn provenance_statistics<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: ProvenanceConfig,
    ) -> ProvenanceStats {
        ProvenanceStatistics::new(context, config).compute()
    }

    /// Finds the shortest path between source and target nodes using a specific weight provider.
    pub fn shortest_path<'a, 'b, W: EdgeWeightProvider>(
        context: &'b GraphAnalyticsContext<'a>,
        source: NodeId,
        target: NodeId,
        config: ShortestPathConfig,
        weight_provider: W,
    ) -> Option<Vec<NodeId>> {
        ShortestPath::new(context, config, weight_provider).compute(source, target)
    }

    /// Finds all simple cycles in the directed graph.
    pub fn find_cycles<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: CycleDetectionConfig,
    ) -> Vec<Vec<NodeId>> {
        CycleDetector::new(context, config).compute()
    }

    /// Checks if the graph has any directed cycles.
    pub fn has_cycles<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: CycleDetectionConfig,
    ) -> bool {
        !CycleDetector::new(context, config).compute().is_empty()
    }

    /// Computes PageRank centrality ranking for all nodes in the graph.
    pub fn pagerank<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: PageRankConfig,
    ) -> Vec<PageRankResult> {
        PageRank::new(context, config).compute()
    }

    /// Group nodes into directed strongly connected components using Tarjan's algorithm.
    pub fn strongly_connected_components<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: SccConfig,
    ) -> Vec<StronglyConnectedComponent> {
        StronglyConnectedComponents::new(context, config).compute()
    }

    /// Finds the shortest path between source and target nodes using A* search with heuristic guidance.
    pub fn astar_shortest_path<'a, 'b, W: EdgeWeightProvider, H: HeuristicProvider>(
        context: &'b GraphAnalyticsContext<'a>,
        source: NodeId,
        target: NodeId,
        config: AStarConfig,
        weight_provider: W,
        heuristic_provider: H,
    ) -> Option<Vec<NodeId>> {
        AStar::new(context, config, weight_provider, heuristic_provider).compute(source, target)
    }

    /// Computes closeness centrality ranking for all nodes in the graph.
    pub fn closeness_centrality<'a, 'b, W: EdgeWeightProvider>(
        context: &'b GraphAnalyticsContext<'a>,
        config: ClosenessConfig,
        weight_provider: W,
    ) -> Vec<ClosenessResult> {
        Closeness::new(context, config, weight_provider).compute()
    }

    /// Locates all bridges and articulation points to diagnose network connectivity.
    pub fn connectivity_diagnostics<'a, 'b>(
        context: &'b GraphAnalyticsContext<'a>,
        config: ConnectivityConfig,
    ) -> ConnectivityReport {
        Connectivity::new(context, config).compute()
    }
}
