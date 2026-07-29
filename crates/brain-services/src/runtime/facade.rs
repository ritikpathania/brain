//! Knowledge Runtime Orchestration Façade (`KnowledgeRuntime`) for Phase 5 Milestone 5.3.
//!
//! ### Orchestration Invariants:
//! 1. `KnowledgeRuntime` is strictly **stateless** with respect to query execution requests.
//! 2. All request-scoped state lives inside `RuntimeRequest<'a>`.
//! 3. Underlying pipelines (`QueryPipeline`, `InferencePipeline`, `ResponseSynthesizer`) remain independently testable.
//! 4. `KnowledgeRuntime` performs coordination only and contains ZERO business logic.
//! 5. Determinism: Given identical `QueryContextProvider` state, identical requests produce identical outputs.

use crate::query::ast::KnowledgeQuery;
use crate::query::context::QueryContextProvider;
use crate::query::fusion::QueryResult;
use crate::query::pipeline::QueryPipeline;
use crate::reasoning::engine::KnowledgeReasoningEngine;
use crate::reasoning::models::KnowledgeResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequestId(pub Uuid);

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "req_{}", self.0)
    }
}

/// Execution options grouping request-scoped telemetry and execution flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionOptions {
    /// Strongly-typed request identifier.
    pub request_id: RequestId,
    /// Optional tracing span identifier.
    pub tracing_id: Option<String>,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            request_id: RequestId(Uuid::new_v4()),
            tracing_id: None,
        }
    }
}

/// Request-scoped execution wrapper containing query intent, storage context, and options.
pub struct RuntimeRequest<'a> {
    /// Declarative query intent AST.
    pub query: &'a KnowledgeQuery,
    /// Abstract storage context provider.
    pub context: &'a dyn QueryContextProvider,
    /// Request-scoped execution options.
    pub options: ExecutionOptions,
}

impl<'a> RuntimeRequest<'a> {
    /// Instantiates a new `RuntimeRequest`.
    pub fn new(query: &'a KnowledgeQuery, context: &'a dyn QueryContextProvider) -> Self {
        Self {
            query,
            context,
            options: ExecutionOptions::default(),
        }
    }

    /// Attaches custom execution options.
    pub fn with_options(mut self, options: ExecutionOptions) -> Self {
        self.options = options;
        self
    }
}

/// Immutable configuration for `KnowledgeRuntime`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRuntimeConfig {
    /// Default candidate limit cap.
    pub max_candidates_limit: usize,
    /// Flag enabling telemetry instrumentation.
    pub enable_telemetry: bool,
}

impl Default for KnowledgeRuntimeConfig {
    fn default() -> Self {
        Self {
            max_candidates_limit: 100,
            enable_telemetry: true,
        }
    }
}

/// Builder for constructing immutable `KnowledgeRuntime`.
#[derive(Debug, Clone, Default)]
pub struct KnowledgeRuntimeBuilder {
    config: KnowledgeRuntimeConfig,
}

impl KnowledgeRuntimeBuilder {
    /// Instantiates a new `KnowledgeRuntimeBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies custom runtime configuration.
    pub fn with_config(mut self, config: KnowledgeRuntimeConfig) -> Self {
        self.config = config;
        self
    }

    /// Builds an immutable, stateless `KnowledgeRuntime`.
    pub fn build(self) -> KnowledgeRuntime {
        KnowledgeRuntime {
            query_pipeline: QueryPipeline::new(),
            reasoning_engine: KnowledgeReasoningEngine::new(),
            _config: self.config,
        }
    }
}

/// Stateless orchestration façade coordinating query retrieval, reasoning inference, and response synthesis.
pub struct KnowledgeRuntime {
    query_pipeline: QueryPipeline,
    reasoning_engine: KnowledgeReasoningEngine,
    _config: KnowledgeRuntimeConfig,
}

impl Default for KnowledgeRuntime {
    fn default() -> Self {
        KnowledgeRuntimeBuilder::new().build()
    }
}

impl KnowledgeRuntime {
    /// Executes deterministic retrieval query across the `QueryPipeline`.
    pub fn query(&self, req: RuntimeRequest<'_>) -> QueryResult {
        self.query_pipeline.execute(req.query, req.context)
    }

    /// Executes end-to-end knowledge inference and reasoning synthesis across `KnowledgeReasoningEngine`.
    pub fn reason(&self, req: RuntimeRequest<'_>) -> KnowledgeResponse {
        self.reasoning_engine.execute(req.query, req.context)
    }
}
