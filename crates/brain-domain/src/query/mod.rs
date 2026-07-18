use crate::entities::{Edge, ExplanationChain, KnowledgeGraph};
use crate::identifiers::{EdgeId, NodeId};
use crate::relations::RelationRegistry;
use crate::validation::{AffectedElement, ValidationDiagnostic, ValidationReport};

/// Graph metrics and connected component algorithms.
pub mod analytics;
/// Explanation expansion and reasoning queries.
pub mod explanation;
/// Consolidated single-turn inspector queries.
pub mod inspector;
/// Path query traversal algorithms.
pub mod path;
/// Temporal explainability and recency context builders.
pub mod temporal_explanation;
/// Diagnostic and validation report filters.
pub mod validation;

pub use inspector::{
    ActivityLogEntry, InspectorModel, ProvenanceDTO, RelationshipDTO, RetrievalExplanationDTO,
};

pub use analytics::{
    AStar, AStarConfig, AnalyticsAlgorithm, AnalyticsFacade, Centrality, CentralityConfig,
    Closeness, ClosenessConfig, ClosenessResult, ClosenessVariant, Complexity,
    ConfidenceDistanceProvider, ConnectedComponents, ConnectedComponentsConfig, Connectivity,
    ConnectivityConfig, ConnectivityReport, CycleDetectionConfig, CycleDetector, DegreeCentrality,
    Distribution, DistributionConfig, EdgeWeightProvider, GraphAnalyticsContext, GraphTraversal,
    HeuristicProvider, PageRank, PageRankConfig, PageRankResult, ProvenanceConfig,
    ProvenanceStatistics, ProvenanceStats, RelationDistribution, RoutingAlgorithm, SccConfig,
    ShortestPath, ShortestPathConfig, StronglyConnectedComponent, StronglyConnectedComponents,
    UniformWeightProvider, ZeroHeuristic,
};
pub use explanation::ExplanationQueryService;
pub use path::PathQueryService;
pub use temporal_explanation::{HistoricalExplanationBuilder, RecencyEvaluation};
pub use validation::ValidationQueryService;

/// Traversal limit configuration.
#[derive(Debug, Clone, Default)]
pub struct PathLimits {
    /// Maximum search depth of traversal chains.
    pub max_depth: Option<usize>,
}

/// Traversal edge relationship filtering.
#[derive(Debug, Clone, Default)]
pub struct PathFilters {
    /// Allowed relation types.
    pub relation_filter: Option<std::collections::HashSet<crate::identifiers::RelationId>>,
}

/// Extensible query configuration for finding paths in the graph.
#[derive(Debug, Clone, Default)]
pub struct PathQuery {
    /// Limits applied to traversal path length/depth.
    pub limits: PathLimits,
    /// Filtering criteria on edge paths.
    pub filters: PathFilters,
}

impl PathQuery {
    /// Creates a new default `PathQuery`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to specify maximum search depth limit.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.limits.max_depth = Some(depth);
        self
    }

    /// Builder method to specify relation filters.
    pub fn with_relations(
        mut self,
        relations: std::collections::HashSet<crate::identifiers::RelationId>,
    ) -> Self {
        self.filters.relation_filter = Some(relations);
        self
    }
}

/// A dedicated service façade that unifies querying and analytics over domain models.
pub struct GraphQueryEngine<'a> {
    /// The underlying knowledge graph to query.
    pub graph: &'a KnowledgeGraph,
    /// The snapshot validation report to query.
    pub report: &'a ValidationReport,
    /// The relation schema registry.
    pub registry: &'a RelationRegistry,
}

impl<'a> GraphQueryEngine<'a> {
    /// Creates a new `GraphQueryEngine` for the given graph, validation report, and relation registry.
    pub fn new(
        graph: &'a KnowledgeGraph,
        report: &'a ValidationReport,
        registry: &'a RelationRegistry,
    ) -> Self {
        Self {
            graph,
            report,
            registry,
        }
    }

    /// Recursively explains the derivation reasoning chain for an edge.
    pub fn explain(&self, edge_id: &EdgeId) -> Option<ExplanationChain> {
        ExplanationQueryService::explain(self.graph, edge_id)
    }

    /// Finds all paths connecting the source node to the target node matching the specified query limits and filters.
    pub fn find_paths(
        &self,
        source: &NodeId,
        target: &NodeId,
        query: &PathQuery,
    ) -> Vec<Vec<EdgeId>> {
        PathQueryService::find_paths(self.graph, source, target, query)
    }

    /// Finds references to all diagnostics in the validation report that affect the target element.
    pub fn find_diagnostics_for_element(
        &self,
        element: &AffectedElement,
    ) -> Vec<&'a ValidationDiagnostic> {
        ValidationQueryService::find_diagnostics_for_element(element, self.report)
    }

    /// Returns an iterator over references to all inferred edges in the graph.
    pub fn find_derived_edges(&self) -> impl Iterator<Item = &'a Edge> {
        self.graph
            .edges
            .values()
            .filter(|e| e.provenance.source == crate::entities::ProvenanceSource::Inferred)
    }

    /// Finds connected components (undirected reachability).
    pub fn find_connected_components(&self, config: ConnectedComponentsConfig) -> Vec<Vec<NodeId>> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::find_connected_components(&ctx, config)
    }

    /// Calculates degree centrality score for all nodes.
    pub fn calculate_degree_centrality(&self, config: CentralityConfig) -> Vec<DegreeCentrality> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::calculate_degree_centrality(&ctx, config)
    }

    /// Computes relation distributions.
    pub fn relation_distribution(&self, config: DistributionConfig) -> Vec<RelationDistribution> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::relation_distribution(&ctx, config)
    }

    /// Computes provenance statistics.
    pub fn provenance_statistics(&self, config: ProvenanceConfig) -> ProvenanceStats {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::provenance_statistics(&ctx, config)
    }

    /// Finds the shortest path between source and target nodes using a specific weight provider.
    pub fn shortest_path<W: EdgeWeightProvider>(
        &self,
        source: NodeId,
        target: NodeId,
        config: ShortestPathConfig,
        weight_provider: W,
    ) -> Option<Vec<NodeId>> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::shortest_path(&ctx, source, target, config, weight_provider)
    }

    /// Finds all simple cycles in the directed graph.
    pub fn find_cycles(&self, config: CycleDetectionConfig) -> Vec<Vec<NodeId>> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::find_cycles(&ctx, config)
    }

    /// Checks if the graph has any directed cycles.
    pub fn has_cycles(&self, config: CycleDetectionConfig) -> bool {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::has_cycles(&ctx, config)
    }

    /// Computes PageRank centrality ranking for all nodes in the graph.
    pub fn pagerank(&self, config: PageRankConfig) -> Vec<PageRankResult> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::pagerank(&ctx, config)
    }

    /// Group nodes into directed strongly connected components using Tarjan's algorithm.
    pub fn strongly_connected_components(
        &self,
        config: SccConfig,
    ) -> Vec<StronglyConnectedComponent> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::strongly_connected_components(&ctx, config)
    }

    /// Finds the shortest path between source and target nodes using A* search with heuristic guidance.
    pub fn astar_shortest_path<W: EdgeWeightProvider, H: HeuristicProvider>(
        &self,
        source: NodeId,
        target: NodeId,
        config: AStarConfig,
        weight_provider: W,
        heuristic_provider: H,
    ) -> Option<Vec<NodeId>> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::astar_shortest_path(
            &ctx,
            source,
            target,
            config,
            weight_provider,
            heuristic_provider,
        )
    }

    /// Computes closeness centrality ranking for all nodes in the graph.
    pub fn closeness_centrality<W: EdgeWeightProvider>(
        &self,
        config: ClosenessConfig,
        weight_provider: W,
    ) -> Vec<ClosenessResult> {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::closeness_centrality(&ctx, config, weight_provider)
    }

    /// Locates all bridges and articulation points to diagnose network connectivity.
    pub fn connectivity_diagnostics(&self, config: ConnectivityConfig) -> ConnectivityReport {
        let ctx = GraphAnalyticsContext::new(self.graph);
        AnalyticsFacade::connectivity_diagnostics(&ctx, config)
    }
}
