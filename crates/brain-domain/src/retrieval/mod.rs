/// Layered query execution caching.
pub mod cache;
/// Metric value objects, datasets, and report structures.
pub mod evaluation;
/// Physical retrieval plan executor.
pub mod executor;
/// Experiment configurations and routing decision models.
pub mod experiment;
/// Core models representing feature extraction and signal normalization.
pub mod features;
/// Candidate fusion strategies (RRF).
pub mod fusion;
/// Core retrieval domain models and explanations.
pub mod models;
/// Optimizer generating Physical plans.
pub mod optimizer;
/// Planner and policies deciding Logical plans.
pub mod planner;
/// Candidate sorting and normalization strategies.
pub mod ranking;
/// Search sources.
pub mod source;
/// Speculative query execution.
pub mod speculation;
/// Event-driven progressive retrieval streaming.
pub mod stream;

pub use cache::{
    CacheStore, CompiledQueryCacheKey, ExecutionCacheStats, LayerStats, LogicalPlanCacheKey,
    PhysicalPlanCacheKey, ResultCacheKey, SnapshotCacheStore,
};
pub use executor::{CancellationChecker, ExecutionPolicy, NeverCancelled, RetrievalExecutor};
pub use fusion::{CandidateFusionStrategy, ReciprocalRankFusion};
pub use models::{
    CanonicalQuery, CompilationMetadata, CompilationResult, CompilerBuildError, CompilerPhase,
    CostHeuristics, Diagnostic, DiagnosticCode, EstimatedCost, Evidence, ExpansionPolicy,
    HeuristicMetadata, HeuristicWeights, LogicalRetrievalPlan, LogicalStep, NormalizedQuery,
    ObservedCost, PhysicalRetrievalPlan, PhysicalStep, PlanningMetadata, QueryRequest,
    RetrievalExecutionContext, RetrievalExecutionReport, RetrievalExplanation, RetrievalRequest,
    RetrievalResult, RetrievedCandidate, RuntimeMetadata, ScoredCandidate, Severity, SnapshotId,
    StoppingCriterion,
};
pub use optimizer::PlanOptimizer;
pub use planner::{QueryCompiler, RetrievalPlanner};
pub use ranking::{NormalizedTieBreakerRanking, RankingStrategy};
pub use source::{GraphExpansionSource, KeywordSource, RetrievalSource, VectorSource};
pub use speculation::{SpeculationPlan, SpeculationStrategy, SubstringSpeculationStrategy};
pub use stream::{CompletionReason, RecordingSink, RetrievalEvent, RetrievalSink, RetrievalStage};
