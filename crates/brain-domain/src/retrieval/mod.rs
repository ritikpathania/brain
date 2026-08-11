//! Retrieval domain model.
//!
//! This module defines the canonical retrieval representation used by the daemon,
//! IPC layer, CLI, TUI, and future APIs.
//!
//! **Domain Invariants**:
//! - The domain is presentation-agnostic. No ratatui or UI dependencies allowed.
//! - Rendering concerns belong strictly to `brain-tui`.
//! - Transport concerns belong strictly to the IPC layer.

/// Canonical transport-independent retrieval result aggregates.
pub mod canonical;
/// Categorical and numeric confidence assessment.
pub mod confidence;
/// EvidenceItem domain aggregate representations.
pub mod evidence;
/// Extensible match explanation reasons.
pub mod explanation;
/// Opaque newtype identifiers.
pub mod ids;
/// Detailed retrieval stage timing breakdowns.
pub mod timing;

pub use canonical::{CanonicalRetrievalResult, RelationshipReference, RetrievalMetadata};
pub use confidence::{ConfidenceAssessment, ConfidenceLevel};
pub use evidence::EvidenceItem;
pub use explanation::{EvidenceReason, StructuredRetrievalExplanation};
pub use ids::{EvidenceId, QueryId};
pub use timing::RetrievalTiming;

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
