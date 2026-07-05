/// Core retrieval domain models and explanations.
pub mod models;
/// Core models representing feature extraction and signal normalization.
pub mod features;
/// Planner and policies deciding Logical plans.
pub mod planner;
/// Optimizer generating Physical plans.
pub mod optimizer;
/// Search sources.
pub mod source;
/// Candidate fusion strategies (RRF).
pub mod fusion;
/// Candidate sorting and normalization strategies.
pub mod ranking;
/// Physical retrieval plan executor.
pub mod executor;
/// Event-driven progressive retrieval streaming.
pub mod stream;
/// Layered query execution caching.
pub mod cache;
/// Speculative query execution.
pub mod speculation;

pub use models::{
    RetrievalRequest, LogicalRetrievalPlan, LogicalStep, PhysicalRetrievalPlan, PhysicalStep,
    RetrievedCandidate, Evidence, RetrievalExplanation, ScoredCandidate, PlanningMetadata,
    RuntimeMetadata, RetrievalExecutionReport, RetrievalResult, RetrievalExecutionContext,
    EstimatedCost, QueryRequest, StoppingCriterion, ExpansionPolicy, NormalizedQuery,
    CanonicalQuery, CompilationMetadata, CompilationResult, Diagnostic, Severity,
    CompilerPhase, DiagnosticCode, CompilerBuildError, SnapshotId,
    HeuristicWeights, HeuristicMetadata, CostHeuristics, ObservedCost
};
pub use planner::{RetrievalPlanner, QueryCompiler};
pub use optimizer::PlanOptimizer;
pub use source::{RetrievalSource, VectorSource, KeywordSource, GraphExpansionSource};
pub use fusion::{CandidateFusionStrategy, ReciprocalRankFusion};
pub use ranking::{RankingStrategy, NormalizedTieBreakerRanking};
pub use executor::{RetrievalExecutor, CancellationChecker, NeverCancelled, ExecutionPolicy};
pub use stream::{RetrievalStage, CompletionReason, RetrievalEvent, RetrievalSink, RecordingSink};
pub use cache::{
    CompiledQueryCacheKey, LogicalPlanCacheKey, PhysicalPlanCacheKey,
    ResultCacheKey, LayerStats, ExecutionCacheStats, CacheStore, SnapshotCacheStore
};
pub use speculation::{SpeculationPlan, SpeculationStrategy, SubstringSpeculationStrategy};
