//! Storage-agnostic `QueryContextProvider` trait and `InMemoryQueryContext` implementation.

use crate::compiler::KnowledgeIR;
use crate::query::executor::Candidate;
use crate::query::plan::{GraphStep, SemanticStep, TemporalStep, TextStep};

/// Abstract view provider trait insulating query execution from underlying storage models.
pub trait QueryContextProvider: Send + Sync {
    /// Evaluates a lexical text search pattern.
    fn evaluate_text(&self, step: &TextStep) -> Vec<Candidate>;
    /// Evaluates a semantic embedding similarity pattern.
    fn evaluate_semantic(&self, step: &SemanticStep) -> Vec<Candidate>;
    /// Evaluates a relationship hop graph traversal pattern.
    fn evaluate_graph(&self, step: &GraphStep) -> Vec<Candidate>;
    /// Evaluates a temporal timestamp constraint pattern.
    fn evaluate_temporal(&self, step: &TemporalStep) -> Vec<Candidate>;
}

/// Default in-memory `QueryContextProvider` evaluating steps against `KnowledgeIR`.
pub struct InMemoryQueryContext<'a> {
    /// Reference to in-memory `KnowledgeIR`.
    pub ir: &'a KnowledgeIR,
}

impl<'a> InMemoryQueryContext<'a> {
    /// Instantiates a new `InMemoryQueryContext`.
    pub fn new(ir: &'a KnowledgeIR) -> Self {
        Self { ir }
    }
}

impl<'a> QueryContextProvider for InMemoryQueryContext<'a> {
    fn evaluate_text(&self, step: &TextStep) -> Vec<Candidate> {
        let pattern_lower = step.pattern.to_lowercase();
        let mut candidates = Vec::new();

        for (id, entity) in &self.ir.entities {
            if entity
                .canonical_name
                .to_lowercase()
                .contains(&pattern_lower)
                || entity
                    .aliases
                    .iter()
                    .any(|a| a.to_lowercase().contains(&pattern_lower))
            {
                candidates.push(Candidate {
                    entity_id: id.clone(),
                    score: 0.90,
                });
            }
        }

        candidates
    }

    fn evaluate_semantic(&self, step: &SemanticStep) -> Vec<Candidate> {
        let prompt_lower = step.prompt.to_lowercase();
        let mut candidates = Vec::new();

        for (id, entity) in &self.ir.entities {
            if entity.canonical_name.to_lowercase().contains(&prompt_lower) {
                candidates.push(Candidate {
                    entity_id: id.clone(),
                    score: 0.95,
                });
            }
        }

        candidates
    }

    fn evaluate_graph(&self, step: &GraphStep) -> Vec<Candidate> {
        let mut candidates = Vec::new();

        for rel in &self.ir.relations {
            if rel.relation_kind == step.relation_kind.as_ref() && rel.target_id == step.target_id {
                candidates.push(Candidate {
                    entity_id: rel.source_id.clone(),
                    score: rel.weight as f32,
                });
            }
        }

        candidates
    }

    fn evaluate_temporal(&self, step: &TemporalStep) -> Vec<Candidate> {
        let mut candidates = Vec::new();

        for (id, entity) in &self.ir.entities {
            let ts = entity.provenance.timestamp_ms;
            let start_ok = step.range.start_ms.is_none_or(|start| ts >= start);
            let end_ok = step.range.end_ms.is_none_or(|end| ts <= end);

            if start_ok && end_ok {
                candidates.push(Candidate {
                    entity_id: id.clone(),
                    score: 1.0,
                });
            }
        }

        candidates
    }
}

/// Execution budget limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionBudget {
    /// Maximum rows processed limit.
    pub max_rows: usize,
    /// Maximum batches processed limit.
    pub max_batches: usize,
    /// Maximum memory bytes limit.
    pub max_memory: usize,
}

impl Default for ExecutionBudget {
    fn default() -> Self {
        Self {
            max_rows: 10_000,
            max_batches: 1_000,
            max_memory: 64 * 1024 * 1024,
        }
    }
}

/// Immutable execution configuration settings.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Unique query execution ID.
    pub query_id: uuid::Uuid,
    /// Vector batch capacity size.
    pub batch_size: usize,
    /// Execution budget limits.
    pub budget: ExecutionBudget,
}

impl ExecutionConfig {
    /// Creates a new ExecutionConfig with defaults.
    pub fn new() -> Self {
        Self {
            query_id: uuid::Uuid::new_v4(),
            batch_size: 100,
            budget: ExecutionBudget::default(),
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable execution runtime state and telemetry collectors.
#[derive(Debug, Clone)]
pub struct ExecutionState {
    /// Cancellation token.
    pub cancellation_token: tokio_util::sync::CancellationToken,
    /// Total rows scanned across all operators.
    pub total_rows_scanned: usize,
}

impl ExecutionState {
    /// Creates a new ExecutionState with defaults.
    pub fn new() -> Self {
        Self {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            total_rows_scanned: 0,
        }
    }
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}

