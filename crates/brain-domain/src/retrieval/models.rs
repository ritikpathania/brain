use crate::identifiers::NodeId;
use crate::entities::KnowledgeGraph;
use crate::relations::RelationRegistry;
use crate::query::analytics::GraphAnalyticsContext;
use crate::validation::ValidationReport;
use std::collections::HashMap;

/// Helper function to reliably hash `f64` floats (handling NaN and ensuring -0.0 and +0.0 map identically).
pub fn hash_f64<H: std::hash::Hasher>(val: f64, state: &mut H) {
    let bits = if val.is_nan() {
        f64::NAN.to_bits()
    } else if val == 0.0 {
        0.0f64.to_bits()
    } else {
        val.to_bits()
    };
    std::hash::Hash::hash(&bits, state);
}

/// Helper function to reliably compare `f64` floats for equality (treating NaNs as equal).
pub fn eq_f64(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        true
    } else {
        a.to_bits() == b.to_bits()
    }
}

/// Typed identifier representing an immutable execution snapshot version.
/// Exposes sequence number accessors but restricts instantiation to crate authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SnapshotId(u64);

impl SnapshotId {
    /// Restricted constructor for snapshot management authorities.
    pub fn new(val: u64) -> Self {
        Self(val)
    }

    /// Access the underlying sequence number.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}



/// Immutable execution context containing all graph details, relationships, and validations.
///
/// **Immutable Infrastructure Invariant**:
/// This context acts strictly as an immutable query planner environment. It must never serve
/// as a mutable scratchpad. Any execution-local state (e.g. visited sets, timings, metrics)
/// must be managed within the executor/solvers rather than modifying the context.
#[derive(Clone)]
pub struct RetrievalExecutionContext<'a> {
    /// Monotonic snapshot identifier protecting caching from stale reads.
    pub snapshot_id: SnapshotId,
    /// Reference to the underlying knowledge graph.
    pub graph: &'a KnowledgeGraph,
    /// Registry containing relational semantic rules and inverse pairs.
    pub registry: &'a RelationRegistry,
    /// Analytical indexes (adjacency, degree cache, etc.).
    pub analytics: &'a GraphAnalyticsContext<'a>,
    /// Precomputed validation report warnings and errors.
    pub validation: Option<&'a ValidationReport>,
}

impl<'a> std::fmt::Debug for RetrievalExecutionContext<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalExecutionContext")
            .field("snapshot_id", &self.snapshot_id)
            .field("graph_nodes", &self.graph.nodes.len())
            .field("graph_edges", &self.graph.edges.len())
            .finish()
    }
}

impl<'a> RetrievalExecutionContext<'a> {
    /// Creates a new `RetrievalExecutionContext`.
    pub fn new(
        snapshot_id: SnapshotId,
        graph: &'a KnowledgeGraph,
        registry: &'a RelationRegistry,
        analytics: &'a GraphAnalyticsContext<'a>,
        validation: Option<&'a ValidationReport>,
    ) -> Self {
        Self {
            snapshot_id,
            graph,
            registry,
            analytics,
            validation,
        }
    }
}

use crate::entities::RelationKind;

/// Pluggable stopping bounds to terminate graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StoppingCriterion {
    /// Maximum search depth/hops from target seed.
    MaxDepth(usize),
    /// Maximum count of visited nodes.
    MaxVisitedNodes(usize),
    /// Stop if edge confidence drops below threshold.
    MinConfidence(f64),
}

impl Eq for StoppingCriterion {}

impl std::hash::Hash for StoppingCriterion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::MaxDepth(d) => {
                state.write_u8(0);
                d.hash(state);
            }
            Self::MaxVisitedNodes(v) => {
                state.write_u8(1);
                v.hash(state);
            }
            Self::MinConfidence(c) => {
                state.write_u8(2);
                hash_f64(*c, state);
            }
        }
    }
}

/// Defines bounds and termination strategies for candidate graph expansions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExpansionPolicy {
    /// List of criteria to terminate expansion early.
    pub criteria: Vec<StoppingCriterion>,
    /// Optional filter to limit traversal to specific relation types.
    pub relation_filter: Option<Vec<RelationKind>>,
}

impl Default for ExpansionPolicy {
    fn default() -> Self {
        Self {
            criteria: vec![StoppingCriterion::MaxDepth(2)],
            relation_filter: None,
        }
    }
}

/// User-facing query request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievalRequest {
    /// The natural language search query.
    pub query: String,
    /// Minimum threshold for candidate acceptances.
    pub min_confidence: f64,
}

impl PartialEq for RetrievalRequest {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query && eq_f64(self.min_confidence, other.min_confidence)
    }
}

impl Eq for RetrievalRequest {}

impl std::hash::Hash for RetrievalRequest {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.query.hash(state);
        hash_f64(self.min_confidence, state);
    }
}

/// Declarative query request containing constraints (the Query DSL).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryRequest {
    /// The search keywords or semantic description.
    pub semantic_query: String,
    /// Minimum threshold for candidate relationships.
    pub min_confidence: f64,
    /// Optional filter targeting specific entity (Node) types.
    pub entity_types: Option<Vec<crate::entities::NodeType>>,
    /// Optional filter targeting specific relationship types.
    pub relations: Option<Vec<RelationKind>>,
    /// Optional budget cap on the number of visited nodes.
    pub max_visited: Option<usize>,
    /// Optional traversal depth cap.
    pub max_depth: Option<usize>,
}

impl PartialEq for QueryRequest {
    fn eq(&self, other: &Self) -> bool {
        self.semantic_query == other.semantic_query
            && eq_f64(self.min_confidence, other.min_confidence)
            && self.entity_types == other.entity_types
            && self.relations == other.relations
            && self.max_visited == other.max_visited
            && self.max_depth == other.max_depth
    }
}

impl Eq for QueryRequest {}

impl std::hash::Hash for QueryRequest {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.semantic_query.hash(state);
        hash_f64(self.min_confidence, state);
        self.entity_types.hash(state);
        self.relations.hash(state);
        self.max_visited.hash(state);
        self.max_depth.hash(state);
    }
}

/// Normalized representation of the query (lexically cleaned).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NormalizedQuery {
    /// Lexically cleaned query text (lowercase, trimmed).
    pub semantic_query: String,
    /// Minimum confidence rating.
    pub min_confidence: f64,
    /// Entity type filters.
    pub entity_types: Option<Vec<crate::entities::NodeType>>,
    /// Relation type filters.
    pub relations: Option<Vec<RelationKind>>,
    /// Budget constraint: maximum visited nodes.
    pub max_visited: Option<usize>,
    /// Budget constraint: maximum depth.
    pub max_depth: Option<usize>,
}

impl PartialEq for NormalizedQuery {
    fn eq(&self, other: &Self) -> bool {
        self.semantic_query == other.semantic_query
            && eq_f64(self.min_confidence, other.min_confidence)
            && self.entity_types == other.entity_types
            && self.relations == other.relations
            && self.max_visited == other.max_visited
            && self.max_depth == other.max_depth
    }
}

impl Eq for NormalizedQuery {}

impl std::hash::Hash for NormalizedQuery {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.semantic_query.hash(state);
        hash_f64(self.min_confidence, state);
        self.entity_types.hash(state);
        self.relations.hash(state);
        self.max_visited.hash(state);
        self.max_depth.hash(state);
    }
}

/// Canonical representation of the query (semantically resolved and rewritten).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CanonicalQuery {
    /// Synonym resolved semantic search text.
    pub semantic_query: String,
    /// Minimum confidence rating.
    pub min_confidence: f64,
    /// Standardized entity type constraints.
    pub entity_types: Option<Vec<crate::entities::NodeType>>,
    /// Standardized relation type constraints.
    pub relations: Option<Vec<RelationKind>>,
    /// Budget constraint: maximum visited nodes.
    pub max_visited: Option<usize>,
    /// Budget constraint: maximum depth.
    pub max_depth: Option<usize>,
    /// Flag to indicate if graph expansion is disabled (constant folding).
    pub disable_expansion: bool,
}

impl PartialEq for CanonicalQuery {
    fn eq(&self, other: &Self) -> bool {
        self.semantic_query == other.semantic_query
            && eq_f64(self.min_confidence, other.min_confidence)
            && self.entity_types == other.entity_types
            && self.relations == other.relations
            && self.max_visited == other.max_visited
            && self.max_depth == other.max_depth
            && self.disable_expansion == other.disable_expansion
    }
}

impl Eq for CanonicalQuery {}

impl std::hash::Hash for CanonicalQuery {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.semantic_query.hash(state);
        hash_f64(self.min_confidence, state);
        self.entity_types.hash(state);
        self.relations.hash(state);
        self.max_visited.hash(state);
        self.max_depth.hash(state);
        self.disable_expansion.hash(state);
    }
}

/// Phases of the query compiler pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompilerPhase {
    /// Low-level lexical token/string cleaning phase.
    Lexical,
    /// Semantic rewriting (synonyms, aliases, macros).
    Semantic,
    /// Constant folding and rule-based planning optimization.
    Optimization,
    /// Semantic integrity and parameter validation rules.
    Validation,
}

impl CompilerPhase {
    /// Returns true if the compilation phase is required to form a valid pipeline.
    pub fn is_required(&self) -> bool {
        match self {
            Self::Lexical | Self::Semantic => true,
            Self::Optimization | Self::Validation => false,
        }
    }

    /// Returns a list of all compiler phase variants.
    pub fn all_phases() -> &'static [Self] {
        &[Self::Lexical, Self::Semantic, Self::Optimization, Self::Validation]
    }
}

/// Machine-readable stable diagnostic codes for query compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum DiagnosticCode {
    /// Query string normalized lexically (CMP-001 - CMP-099 range).
    QueryNormalized,
    /// Synonym or alias expanded (CMP-100 - CMP-199 range).
    AliasExpanded,
    /// Logical query constant folded/pruned (CMP-200 - CMP-299 range).
    ConstantFolded,
    /// Unsupported query DSL construct (CMP-300 - CMP-399 range).
    UnsupportedConstruct,
}

impl DiagnosticCode {
    /// String code notation (e.g. "CMP-001").
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::QueryNormalized => "CMP-001",
            Self::AliasExpanded => "CMP-100",
            Self::ConstantFolded => "CMP-200",
            Self::UnsupportedConstruct => "CMP-300",
        }
    }
}

/// Severity levels for query compiler diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub enum Severity {
    /// Informational notices (synonym resolutions, expansions).
    Info,
    /// Warnings highlighting potential execution inefficiencies or budget adjustments.
    Warning,
    /// Critical compiler errors preventing plan formulation.
    Error,
}

/// Structured query compiler diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub struct Diagnostic {
    /// Stable machine-readable diagnostic code.
    pub code: DiagnosticCode,
    /// Telemetry severity grade.
    pub severity: Severity,
    /// Diagnostic description.
    pub message: String,
    /// Pass that generated the diagnostic.
    pub origin_pass: Option<String>,
}

/// Errors that can occur when constructing the query compiler pipeline.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CompilerBuildError {
    /// Duplicate pass identifier detected.
    #[error("Duplicate pass identifier detected: {0}")]
    DuplicatePass(String),
    /// Invalid pass ordering detected.
    #[error("Invalid pass ordering: pass {pass_id} of phase {phase:?} ran out of order")]
    InvalidPassOrdering {
        /// The identifier of the pass that caused the ordering violation.
        pass_id: String,
        /// The phase of the violating pass.
        phase: CompilerPhase,
    },
    /// A required compiler phase is missing from the pipeline.
    #[error("Required compiler phase {0:?} is missing from the pipeline")]
    MissingRequiredPhase(CompilerPhase),
}

/// Metadata collected during query compilation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct CompilationMetadata {
    /// Ordered log of compiler passes executed.
    pub passes_executed: Vec<String>,
    /// Structured diagnostic messages.
    pub diagnostics: Vec<Diagnostic>,
    /// Compiler version.
    pub compiler_version: String,
}

/// Output of the compiler pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct CompilationResult {
    /// Canonicalized query ready for planner execution.
    pub canonical_query: CanonicalQuery,
    /// Telemetry details from query compilation.
    pub metadata: CompilationMetadata,
}

/// Logical description of retrieval strategy and steps.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct LogicalRetrievalPlan {
    /// Ordered logical operations to perform.
    pub steps: Vec<LogicalStep>,
}

/// Logical step types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum LogicalStep {
    /// Perform semantic vector search.
    VectorRetrieve {
        /// The query text for semantic lookup.
        query: String,
    },
    /// Perform inverted index keyword search.
    KeywordRetrieve {
        /// The keyword pattern/string.
        query: String,
    },
    /// Expand neighbors starting from candidate seeds.
    ExpandNeighbors {
        /// Optional subset of nodes to expand. If empty, uses previous step candidates.
        source_nodes: Vec<NodeId>,
        /// Expansion policy settings for this step.
        policy: ExpansionPolicy,
    },
}

/// Physical plan ready for execution (optimized step order, caching configurations).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct PhysicalRetrievalPlan {
    /// Ordered execution instructions.
    pub physical_steps: Vec<PhysicalStep>,
    /// Estimated cost of executing this physical plan.
    pub cost: EstimatedCost,
    /// Version of cost heuristics used.
    pub heuristics_version: u64,
}

/// Heuristic cost metrics for executing a physical retrieval plan.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EstimatedCost {
    /// Cost estimation of vector database search queries.
    pub vector_cost: f64,
    /// Cost estimation of keyword query indexing operations.
    pub keyword_cost: f64,
    /// Cost estimation of BFS neighbor graph expansions.
    pub expansion_cost: f64,
    /// Cost estimation of Candidate Fusion (e.g. RRF rank calculations).
    pub fusion_cost: f64,
    /// Cost estimation of final candidate ranking and sorting.
    pub ranking_cost: f64,
}

impl EstimatedCost {
    /// Computes the overall consolidated sum of all cost components.
    pub fn total_cost(&self) -> f64 {
        self.vector_cost + self.keyword_cost + self.expansion_cost + self.fusion_cost + self.ranking_cost
    }
}

impl Eq for EstimatedCost {}

impl std::hash::Hash for EstimatedCost {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        hash_f64(self.vector_cost, state);
        hash_f64(self.keyword_cost, state);
        hash_f64(self.expansion_cost, state);
        hash_f64(self.fusion_cost, state);
        hash_f64(self.ranking_cost, state);
    }
}

/// Physical step types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum PhysicalStep {
    /// Execute semantic query.
    VectorRetrieve {
        /// Optimized semantic query text.
        query: String,
    },
    /// Execute keyword query.
    KeywordRetrieve {
        /// Optimized keyword query.
        query: String,
    },
    /// Traverse and collect neighbor nodes.
    ExpandNeighbors {
        /// Hydrated seed nodes to expand from.
        source_nodes: Vec<NodeId>,
        /// Expansion policy settings for this step.
        policy: ExpansionPolicy,
    },
}

/// Core candidate generated by a retrieval source before candidate fusion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetrievedCandidate {
    /// The retrieved node identifier.
    pub node_id: NodeId,
    /// Identifies which source produced this candidate.
    pub source_id: &'static str,
    /// Local raw relevance score produced by the source.
    pub local_score: f64,
    /// Traversal or search details justifying the retrieval.
    pub explanation_fragments: Vec<Evidence>,
}

impl PartialEq for RetrievedCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
            && self.source_id == other.source_id
            && eq_f64(self.local_score, other.local_score)
            && self.explanation_fragments == other.explanation_fragments
    }
}

impl Eq for RetrievedCandidate {}

impl std::hash::Hash for RetrievedCandidate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
        self.source_id.hash(state);
        hash_f64(self.local_score, state);
        self.explanation_fragments.hash(state);
    }
}

/// Structured audit-trail evidence describing how and why a node was retrieved.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Evidence {
    /// Semantic vector similarity score.
    SemanticMatch {
        /// Computed cosine/relevance similarity.
        similarity: f64,
    },
    /// Traversal step path trace.
    GraphTraversal {
        /// Graph distance from seed.
        depth: usize,
        /// The root seed node ID where traversal originated.
        from: NodeId,
    },
    /// Keyword text match metrics.
    KeywordHit {
        /// Number of keyword occurrences found in node text.
        occurrences: usize,
    },
    /// RRF score contribution.
    FusionContribution {
        /// The source ranking index.
        rrf_rank: usize,
    },
    /// Boosting adjustments.
    RankingAdjustment {
        /// Score multiplication/addition boost weight.
        boost: f64,
    },
    /// Explains temporal visibility of an edge at query reference time.
    TemporalVisibility {
        /// When the edge was first observed.
        observed_at: crate::temporal::TimePoint,
        /// Active validity intervals during projection.
        validity_intervals: Vec<crate::temporal::TimeInterval>,
        /// Reference time of the temporal query.
        query_time: crate::temporal::TimePoint,
        /// Visibility policy/mode matched.
        visibility_mode: crate::temporal::TemporalVisibility,
    },
    /// Explains score adjustments due to temporal decay.
    RecencyDecay {
        /// Recency decay policy type.
        policy: crate::temporal::RecencyPolicy,
        /// When the edge was first observed.
        observed_at: crate::temporal::TimePoint,
        /// Reference time of the temporal query.
        reference_time: crate::temporal::TimePoint,
        /// Computed elapsed time in seconds.
        elapsed_seconds: f64,
        /// Computed recency decay factor applied to the match score.
        decay_factor: f64,
    },
}

impl PartialEq for Evidence {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::SemanticMatch { similarity: a }, Self::SemanticMatch { similarity: b }) => eq_f64(*a, *b),
            (Self::GraphTraversal { depth: d1, from: f1 }, Self::GraphTraversal { depth: d2, from: f2 }) => d1 == d2 && f1 == f2,
            (Self::KeywordHit { occurrences: o1 }, Self::KeywordHit { occurrences: o2 }) => o1 == o2,
            (Self::FusionContribution { rrf_rank: r1 }, Self::FusionContribution { rrf_rank: r2 }) => r1 == r2,
            (Self::RankingAdjustment { boost: b1 }, Self::RankingAdjustment { boost: b2 }) => eq_f64(*b1, *b2),
            (
                Self::TemporalVisibility {
                    observed_at: o1,
                    validity_intervals: v1,
                    query_time: q1,
                    visibility_mode: m1,
                },
                Self::TemporalVisibility {
                    observed_at: o2,
                    validity_intervals: v2,
                    query_time: q2,
                    visibility_mode: m2,
                },
            ) => o1 == o2 && v1 == v2 && q1 == q2 && m1 == m2,
            (
                Self::RecencyDecay {
                    policy: p1,
                    observed_at: o1,
                    reference_time: r1,
                    elapsed_seconds: el1,
                    decay_factor: d1,
                },
                Self::RecencyDecay {
                    policy: p2,
                    observed_at: o2,
                    reference_time: r2,
                    elapsed_seconds: el2,
                    decay_factor: d2,
                },
            ) => p1 == p2 && o1 == o2 && r1 == r2 && eq_f64(*el1, *el2) && eq_f64(*d1, *d2),
            _ => false,
        }
    }
}

impl Eq for Evidence {}

impl std::hash::Hash for Evidence {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::SemanticMatch { similarity } => {
                state.write_u8(0);
                hash_f64(*similarity, state);
            }
            Self::GraphTraversal { depth, from } => {
                state.write_u8(1);
                depth.hash(state);
                from.hash(state);
            }
            Self::KeywordHit { occurrences } => {
                state.write_u8(2);
                occurrences.hash(state);
            }
            Self::FusionContribution { rrf_rank } => {
                state.write_u8(3);
                rrf_rank.hash(state);
            }
            Self::RankingAdjustment { boost } => {
                state.write_u8(4);
                hash_f64(*boost, state);
            }
            Self::TemporalVisibility {
                observed_at,
                validity_intervals,
                query_time,
                visibility_mode,
            } => {
                state.write_u8(5);
                observed_at.hash(state);
                validity_intervals.hash(state);
                query_time.hash(state);
                visibility_mode.hash(state);
            }
            Self::RecencyDecay {
                policy,
                observed_at,
                reference_time,
                elapsed_seconds,
                decay_factor,
            } => {
                state.write_u8(6);
                policy.hash(state);
                observed_at.hash(state);
                reference_time.hash(state);
                hash_f64(*elapsed_seconds, state);
                hash_f64(*decay_factor, state);
            }
        }
    }
}

/// Structured explanation containing ordered evidence trails.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct RetrievalExplanation {
    /// Sequence of audit trail entries.
    pub evidence_list: Vec<Evidence>,
}

/// Consolidated final candidate result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScoredCandidate {
    /// The final node identifier.
    pub node_id: NodeId,
    /// Consolidated fused/reranked score.
    pub score: f64,
}

impl PartialEq for ScoredCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id && eq_f64(self.score, other.score)
    }
}

impl Eq for ScoredCandidate {}

impl std::hash::Hash for ScoredCandidate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
        hash_f64(self.score, state);
    }
}

/// Planner cost estimates and optimizations compiled prior to execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct PlanningMetadata {
    /// Planner estimates computed prior to execution.
    pub estimated_cost: EstimatedCost,
    /// Strategic planner decisions.
    pub planner_decisions: Vec<String>,
    /// Logical-to-physical optimization decisions.
    pub optimizer_decisions: Vec<String>,
    /// Version of the CostHeuristics snapshot used for optimization.
    pub heuristics_version: u64,
}

/// Real-world execution metrics and timings observed during execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct RuntimeMetadata {
    /// Latency elapsed in microseconds.
    pub elapsed_microseconds: u64,
    /// Number of candidates produced by sources.
    pub candidates_produced: usize,
    /// Number of candidates fused.
    pub candidates_fused: usize,
    /// Number of neighbor nodes expanded.
    pub expansions_performed: usize,
    /// Number of candidate ranking operations.
    pub ranking_operations: usize,
}

/// Comprehensive plan execution audit report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct RetrievalExecutionReport {
    /// Strategic compilation and planning metadata.
    pub planning: PlanningMetadata,
    /// Real-world execution metrics and runtime statistics.
    pub runtime: RuntimeMetadata,
}

/// Final composed retrieval result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RetrievalResult {
    /// Canonical, sorted list of resulting candidates.
    pub candidates: Vec<ScoredCandidate>,
    /// Maps candidates to their explainability trails.
    pub explanations: HashMap<NodeId, RetrievalExplanation>,
    /// Execution telemetry execution report.
    pub report: RetrievalExecutionReport,
}

/// Active execution weights for planner/optimizer components.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HeuristicWeights {
    /// Estimated relative cost weight for vector database lookups.
    pub vector_weight: f64,
    /// Estimated relative cost weight for keyword queries.
    pub keyword_weight: f64,
    /// Estimated relative cost weight for neighbors BFS expansions.
    pub expansion_weight: f64,
    /// Estimated relative cost weight for candidate rank fusion.
    pub fusion_weight: f64,
    /// Estimated relative cost weight for final candidates ranking.
    pub ranking_weight: f64,
}

impl Default for HeuristicWeights {
    fn default() -> Self {
        Self {
            vector_weight: 10.0,
            keyword_weight: 2.0,
            expansion_weight: 5.0,
            fusion_weight: 1.0,
            ranking_weight: 0.5,
        }
    }
}

/// Versioning details for cost heuristic snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct HeuristicMetadata {
    /// Monotonically incrementing sequence version.
    pub version: u64,
}

/// Consolidated snapshot containing planning metadata and weights.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CostHeuristics {
    /// Lifecycle and tracking metadata.
    pub metadata: HeuristicMetadata,
    /// Relative weights mapping step costs.
    pub weights: HeuristicWeights,
}

impl Default for CostHeuristics {
    fn default() -> Self {
        Self {
            metadata: HeuristicMetadata { version: 1 },
            weights: HeuristicWeights::default(),
        }
    }
}

/// Extracted observed real-world cost metrics from runtime reports.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedCost {
    /// Actual cost calculated for vector search.
    pub vector_cost: f64,
    /// Actual cost calculated for keyword search.
    pub keyword_cost: f64,
    /// Actual cost calculated for node neighborhood expansion traversal.
    pub expansion_cost: f64,
    /// Actual cost calculated for rank fusion aggregation.
    pub fusion_cost: f64,
    /// Actual cost calculated for list sorting/ranking.
    pub ranking_cost: f64,
}

/// Non-negative, finite ranking multiplier value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct RankingWeight(f64);

impl RankingWeight {
    /// Create a new validated `RankingWeight`.
    pub fn new(val: f64) -> Result<Self, crate::consolidation::MetricConstructionError> {
        if !val.is_finite() {
            return Err(crate::consolidation::MetricConstructionError::NotFinite { val });
        }
        if val < 0.0 {
            return Err(crate::consolidation::MetricConstructionError::OutOfRange { val, min: 0.0, max: f64::MAX });
        }
        Ok(Self(val))
    }

    /// Access the underlying value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

impl std::hash::Hash for RankingWeight {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        hash_f64(self.0, state);
    }
}

impl Eq for RankingWeight {}

/// Normalized ranking signal in range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct NormalizedSignal(f64);

impl NormalizedSignal {
    /// Create a new validated `NormalizedSignal` between 0.0 and 1.0.
    pub fn new(val: f64) -> Result<Self, crate::consolidation::MetricConstructionError> {
        if !val.is_finite() {
            return Err(crate::consolidation::MetricConstructionError::NotFinite { val });
        }
        if val < 0.0 || val > 1.0 {
            return Err(crate::consolidation::MetricConstructionError::OutOfRange { val, min: 0.0, max: 1.0 });
        }
        Ok(Self(val))
    }

    /// Access the underlying value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

impl std::hash::Hash for NormalizedSignal {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        hash_f64(self.0, state);
    }
}

impl Eq for NormalizedSignal {}

/// Monotonically incrementing snapshot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct SnapshotVersion(u64);

impl SnapshotVersion {
    /// Create a new `SnapshotVersion`.
    pub fn new(val: u64) -> Self {
        Self(val)
    }

    /// Access the underlying version number.
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Extensible structured calibration details.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalibrationMetadata {
    /// The algorithm used for calibration.
    algorithm_used: String,
    /// The calculated validation loss.
    validation_loss: Option<f64>,
    /// Model version identifier.
    model_version: Option<RankingModelVersion>,
    /// Serialized parameters (e.g. JSON representation of decision tree splits)
    parameters: Option<String>,
}

impl CalibrationMetadata {
    /// Create a new `CalibrationMetadata`.
    pub fn new(algorithm_used: String, validation_loss: Option<f64>) -> Self {
        Self {
            algorithm_used,
            validation_loss,
            model_version: None,
            parameters: None,
        }
    }

    /// Builder method to append model details.
    pub fn with_model_details(
        mut self,
        model_version: Option<RankingModelVersion>,
        parameters: Option<String>,
    ) -> Self {
        self.model_version = model_version;
        self.parameters = parameters;
        self
    }

    /// Get the algorithm used.
    pub fn algorithm_used(&self) -> &str {
        &self.algorithm_used
    }

    /// Get the validation loss.
    pub fn validation_loss(&self) -> Option<f64> {
        self.validation_loss
    }

    /// Get the model version.
    pub fn model_version(&self) -> Option<RankingModelVersion> {
        self.model_version
    }

    /// Get the serialized parameters.
    pub fn parameters(&self) -> Option<&str> {
        self.parameters.as_deref()
    }
}

/// Metadata tracking weight lineage.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotMetadata {
    /// The snapshot version number.
    pub version: SnapshotVersion,
    /// The snapshot creation time.
    pub created_at: crate::temporal::TimePoint,
    /// Calibration metadata associated with the snapshot.
    pub calibration_metadata: CalibrationMetadata,
}

/// Scoring multipliers, immutable by construction.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RankingWeights {
    semantic: RankingWeight,
    graph: RankingWeight,
    recency: RankingWeight,
    temporal: RankingWeight,
}

impl RankingWeights {
    /// Create a new `RankingWeights` collection.
    pub fn new(
        semantic: RankingWeight,
        graph: RankingWeight,
        recency: RankingWeight,
        temporal: RankingWeight,
    ) -> Self {
        Self {
            semantic,
            graph,
            recency,
            temporal,
        }
    }

    /// Get the semantic weight multiplier.
    pub fn semantic(&self) -> RankingWeight {
        self.semantic
    }

    /// Get the graph weight multiplier.
    pub fn graph(&self) -> RankingWeight {
        self.graph
    }

    /// Get the recency weight multiplier.
    pub fn recency(&self) -> RankingWeight {
        self.recency
    }

    /// Get the temporal weight multiplier.
    pub fn temporal(&self) -> RankingWeight {
        self.temporal
    }
}

/// A versioned configuration snapshot of ranking weights.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WeightSnapshot {
    /// Metadata tracking snapshot lineage and creation parameters.
    pub metadata: SnapshotMetadata,
    /// Scoring weight values.
    pub weights: RankingWeights,
}

/// Represents a user interaction event recorded for rank calibration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackEvent {
    /// Unique event identifier.
    pub id: String,
    /// Schema version of the event format.
    pub schema_version: u32,
    /// The query executed during interaction.
    pub query: String,
    /// Target node selected or evaluated.
    pub node_id: NodeId,
    /// True if the user selected/interacted with the candidate.
    pub selected: bool,
    /// Timestamp of interaction.
    pub timestamp: u64,
    /// The position at which the node was ranked when presented.
    pub ranking_position: usize,
    /// Extensible metadata context payload.
    pub context: String,
}

/// Feature scores representing candidate properties.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RankingSignals {
    /// The semantic similarity signal.
    pub semantic: NormalizedSignal,
    /// The graph structural centrality signal.
    pub graph: NormalizedSignal,
    /// The temporal recency signal.
    pub recency: NormalizedSignal,
    /// The temporal interaction/observation density signal.
    pub temporal: NormalizedSignal,
}

impl RankingSignals {
    /// Create a new collection of `RankingSignals`.
    pub fn new(
        semantic: NormalizedSignal,
        graph: NormalizedSignal,
        recency: NormalizedSignal,
        temporal: NormalizedSignal,
    ) -> Self {
        Self {
            semantic,
            graph,
            recency,
            temporal,
        }
    }
}

/// Version tag for calibration policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct CalibrationPolicyVersion(u32);

impl CalibrationPolicyVersion {
    /// Create a new `CalibrationPolicyVersion`.
    pub fn new(val: u32) -> Self {
        Self(val)
    }

    /// Get the underlying version value.
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Enumeration of calibration algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CalibrationAlgorithmType {
    /// Linear moving average heuristic updates.
    LinearAdjustment,
}

/// Parameter knobs configuring calibration loops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationPolicy {
    /// Calibration policy identifier.
    pub version: CalibrationPolicyVersion,
    /// The algorithm strategy target.
    pub algorithm: CalibrationAlgorithmType,
    /// Gradient or multiplier step size.
    pub learning_rate: f64,
    /// Weights decay or smoothing regularization.
    pub regularization: f64,
    /// Minimum feedback events required before adjusting parameters.
    pub min_feedback_events: usize,
}

/// Immutable calibration result parameters.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalibrationReport {
    /// Monotonic version generated.
    pub candidate_version: SnapshotVersion,
    /// Version active before calibration run.
    pub previous_version: SnapshotVersion,
    /// Policy configuration version executed.
    pub policy_version: CalibrationPolicyVersion,
    /// Number of feedback records processed.
    pub feedback_processed: usize,
    /// Evaluated validation loss metric.
    pub validation_loss: f64,
    /// Text details or statistics on convergence.
    pub convergence_information: String,
    /// Publication accepted or rejected decision indicator.
    pub publication_decision: bool,
}

/// Version identifier tracking the model implementation structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RankingModelVersion {
    /// Linear combination model version 1.
    V1Linear,
    /// Decision tree model version 2.
    V2DecisionTree,
}

/// Polymorphic interface for scoring candidate nodes.
pub trait RankingModel: Send + Sync {
    /// Access model version tag.
    fn version(&self) -> RankingModelVersion;
    /// Calculate the ranking score for a candidate's signals.
    fn score(&self, signals: &RankingSignals) -> f64;
}

/// A standard linear model weighting ranking signals.
#[derive(Debug, Clone)]
pub struct LinearRankingModel {
    weights: RankingWeights,
}

impl LinearRankingModel {
    /// Create a new `LinearRankingModel`.
    pub fn new(weights: RankingWeights) -> Self {
        Self { weights }
    }
}

impl RankingModel for LinearRankingModel {
    fn version(&self) -> RankingModelVersion {
        RankingModelVersion::V1Linear
    }

    fn score(&self, signals: &RankingSignals) -> f64 {
        self.weights.semantic().value() * signals.semantic.value()
            + self.weights.graph().value() * signals.graph.value()
            + self.weights.recency().value() * signals.recency.value()
            + self.weights.temporal().value() * signals.temporal.value()
    }
}

/// Identifier for ranking signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FeatureId {
    /// Semantic similarity signal.
    Semantic,
    /// Graph-based connectivity signal.
    Graph,
    /// Recency/decay signal.
    Recency,
    /// Projected temporal edge score.
    Temporal,
}

/// Holds a validated split threshold value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SplitThreshold(f64);

impl SplitThreshold {
    /// Creates a new validated `SplitThreshold`.
    pub fn new(val: f64) -> Result<Self, crate::consolidation::MetricConstructionError> {
        if !val.is_finite() {
            return Err(crate::consolidation::MetricConstructionError::NotFinite { val });
        }
        Ok(Self(val))
    }

    /// Accesses the underlying threshold value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// Holds a validated leaf score value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct LeafScore(f64);

impl LeafScore {
    /// Creates a new validated `LeafScore`.
    pub fn new(val: f64) -> Result<Self, crate::consolidation::MetricConstructionError> {
        if !val.is_finite() {
            return Err(crate::consolidation::MetricConstructionError::NotFinite { val });
        }
        Ok(Self(val))
    }

    /// Accesses the underlying score value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// Serializable decision tree node definition.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DecisionTreeNode {
    /// Internal split node.
    Split {
        /// Feature dimension to split on.
        feature: FeatureId,
        /// Split threshold.
        threshold: SplitThreshold,
        /// Left branch (val < threshold).
        left: Box<DecisionTreeNode>,
        /// Right branch (val >= threshold).
        right: Box<DecisionTreeNode>,
    },
    /// Terminal leaf node.
    Leaf {
        /// Return score.
        score: LeafScore,
    },
}

/// Serializable package for decision tree configs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecisionTreeDefinition {
    /// Root node of the tree.
    pub root: DecisionTreeNode,
}

/// A compiler that compiles tree definitions into executable trees.
pub struct DecisionTreeCompiler;

impl DecisionTreeCompiler {
    /// Compiles a `DecisionTreeDefinition` into a `CompiledDecisionTree`.
    pub fn compile(definition: &DecisionTreeDefinition) -> CompiledDecisionTree {
        CompiledDecisionTree {
            root: definition.root.clone(),
        }
    }
}

/// Compiled representation of a decision tree.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledDecisionTree {
    root: DecisionTreeNode,
}

impl CompiledDecisionTree {
    /// Evaluates the compiled tree recursively.
    pub fn evaluate(&self, signals: &RankingSignals) -> f64 {
        Self::evaluate_node(&self.root, signals)
    }

    /// Evaluates the compiled tree recursively while tracking the decision path.
    pub fn evaluate_with_path(&self, signals: &RankingSignals) -> (f64, Vec<FeatureId>) {
        let mut path = Vec::new();
        let score = Self::evaluate_node_with_path(&self.root, signals, &mut path);
        (score, path)
    }

    fn evaluate_node(node: &DecisionTreeNode, signals: &RankingSignals) -> f64 {
        match node {
            DecisionTreeNode::Leaf { score } => score.value(),
            DecisionTreeNode::Split { feature, threshold, left, right } => {
                let val = match feature {
                    FeatureId::Semantic => signals.semantic.value(),
                    FeatureId::Graph => signals.graph.value(),
                    FeatureId::Recency => signals.recency.value(),
                    FeatureId::Temporal => signals.temporal.value(),
                };
                if val < threshold.value() {
                    Self::evaluate_node(left, signals)
                } else {
                    Self::evaluate_node(right, signals)
                }
            }
        }
    }

    fn evaluate_node_with_path(node: &DecisionTreeNode, signals: &RankingSignals, path: &mut Vec<FeatureId>) -> f64 {
        match node {
            DecisionTreeNode::Leaf { score } => score.value(),
            DecisionTreeNode::Split { feature, threshold, left, right } => {
                path.push(*feature);
                let val = match feature {
                    FeatureId::Semantic => signals.semantic.value(),
                    FeatureId::Graph => signals.graph.value(),
                    FeatureId::Recency => signals.recency.value(),
                    FeatureId::Temporal => signals.temporal.value(),
                };
                if val < threshold.value() {
                    Self::evaluate_node_with_path(left, signals, path)
                } else {
                    Self::evaluate_node_with_path(right, signals, path)
                }
            }
        }
    }
}

/// Executable model evaluating decision tree definitions over ranking signals.
#[derive(Debug, Clone)]
pub struct DecisionTreeRankingModel {
    definition: DecisionTreeDefinition,
    compiled: CompiledDecisionTree,
}

impl DecisionTreeRankingModel {
    /// Creates a new `DecisionTreeRankingModel`.
    pub fn new(definition: DecisionTreeDefinition) -> Self {
        let compiled = DecisionTreeCompiler::compile(&definition);
        Self { definition, compiled }
    }

    /// Access the underlying immutable definition.
    pub fn definition(&self) -> &DecisionTreeDefinition {
        &self.definition
    }

    /// Access the compiled representation.
    pub fn compiled(&self) -> &CompiledDecisionTree {
        &self.compiled
    }
}

impl RankingModel for DecisionTreeRankingModel {
    fn version(&self) -> RankingModelVersion {
        RankingModelVersion::V2DecisionTree
    }

    fn score(&self, signals: &RankingSignals) -> f64 {
        self.compiled.evaluate(signals)
    }
}



