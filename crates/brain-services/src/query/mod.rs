//! Query services and Knowledge Query Engine (Phase 5 Milestone 5.1).
//!
//! Provides read-model query services (`JobQueryService`, `SearchQueryService`, `SessionQueryService`),
//! as well as the Knowledge Query Engine featuring declarative query AST (`KnowledgeQuery`),
//! first-class inspectable execution plans (`ExecutionPlan`), query planner, optimizer extension points,
//! abstract view context providers (`QueryContextProvider`), step execution (`QueryExecutor`), and reciprocal rank candidate fusion (`ReciprocalRankFusion`).

/// Strongly-typed Query Errors for Phase 5 Query Facade.
pub mod errors;
/// Unified Query Models for Phase 5 Query Facade.
pub mod models;
/// Atomic ProjectionSnapshot container for Phase 5 Query Facade.
pub mod snapshot;

/// Modular stateless evaluators for Phase 5 Query Facade.
pub mod evaluators;
/// Single thread-safe read query composition facade for Phase 5 Query Facade.
pub mod facade;

pub use errors::*;
pub use evaluators::*;
pub use facade::*;
pub use models::*;
pub use snapshot::*;

/// Data transfer objects for read-model views.
pub mod dto;
/// Query filters and pagination specifications.
pub mod filters;
/// Background jobs query service implementation.
pub mod jobs;
/// Shared subscription invalidation registry.
pub mod registry;
/// Search query service implementation.
pub mod search;
/// Sessions query service implementation.
pub mod sessions;
/// Live query DTO streams.
pub mod subscription;
/// Query service interfaces.
pub mod traits;

pub use dto::{JobDetails, JobSummary, MessageDTO, SearchSummary, SessionDetails, SessionSummary};
pub use filters::{JobQuery, PaginationSpec, SearchQuery, SessionQuery};
pub use jobs::SqliteJobQueryService;
pub use registry::{QueryResponse, QuerySubscriptionRegistry, SubscriptionKey};
pub use search::SqliteSearchQueryService;
pub use sessions::SqliteSessionQueryService;
pub use subscription::{
    JobSubscriptionService, LiveQuery, QuerySnapshot, SearchSubscriptionService,
    SessionSubscriptionService, SqliteJobSubscriptionService, SqliteSearchSubscriptionService,
    SqliteSessionSubscriptionService, WatchLiveQuery,
};
pub use traits::{JobQueryService, SearchQueryService, SessionQueryService};

/// Declarative intent AST for knowledge query engine.
pub mod ast;
/// Abstract view context provider trait and implementations.
pub mod context;
/// Query candidate structures and step executor.
pub mod executor;
/// Pluggable candidate fusion strategies and RRF.
pub mod fusion;
/// Query plan optimization passes.
pub mod optimizer;
/// End-to-end pipeline composition root.
pub mod pipeline;
/// Inspectable declarative execution plans and step types.
pub mod plan;
/// Declarative query planner.
pub mod planner;

pub use ast::{KnowledgeQuery, RelationFilter, TemporalRange};
pub use context::{InMemoryQueryContext, QueryContextProvider};
pub use executor::{Candidate, QueryExecutor, RawCandidateSet};
pub use fusion::{FusionStrategy, QueryResult, ReciprocalRankFusion};
pub use optimizer::{NoOpOptimizer, PlanOptimizer};
pub use pipeline::QueryPipeline;
pub use plan::{
    ExecutionPlan, ExecutionStep, ExecutionStepId, GraphStep, SemanticStep, TemporalStep, TextStep,
};
pub use planner::QueryPlanner;
/// Semantic binder for Phase 2 Knowledge Query Engine.
pub mod semantic_binder;
pub use semantic_binder::*;
/// Logical planner for Phase 2 Knowledge Query Engine.
pub mod logical_planner;
pub use logical_planner::*;
/// Logical optimizer for Phase 2 Knowledge Query Engine.
pub mod logical_optimizer;
pub use logical_optimizer::*;
/// Physical plan tree models for Phase 2 Knowledge Query Engine.
pub mod physical_plan;
pub use physical_plan::*;
/// Physical planner for Phase 2 Knowledge Query Engine.
pub mod physical_planner;
pub use physical_planner::*;
/// Vectorized batch container for Phase 2 Knowledge Query Engine.
pub mod batch;
pub use batch::*;
/// Physical batch operators for Phase 2 Knowledge Query Engine.
pub mod operators;
pub use operators::*;
/// Physical execution engine for Phase 2 Knowledge Query Engine.
pub mod execution_engine;
pub use execution_engine::*;
/// Explain plan formatter for Phase 2 Knowledge Query Engine.
pub mod explain_formatter;
pub use explain_formatter::*;
