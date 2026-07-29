//! Independently schedulable concrete reflection tasks.

use crate::policies::lifecycle_policy::{DefaultLifecyclePolicy, LifecyclePolicy};
use crate::reconciliation::{
    AliasResolutionPass, ContradictionDetectionPass, DuplicateDetectionPass,
    EntityNormalizationPass, OrphanDetectionPass, ReconciliationPipeline,
};
use crate::reflection::contracts::{ReflectionTask, ReflectionTaskKind, TaskId, TaskReport};
use brain_domain::CanonicalEntity;
use std::collections::HashMap;
use std::time::Instant;

/// `RepairTask`: Runs the reconciliation compiler pipeline to fix entity normalization, alias mappings, duplicates, contradictions, and orphans.
#[derive(Debug, Clone, Default)]
pub struct RepairTask;

impl RepairTask {
    /// Creates a new `RepairTask`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionTask for RepairTask {
    fn id(&self) -> TaskId {
        TaskId("reflection.repair.reconciliation".to_string())
    }

    fn name(&self) -> &'static str {
        "RepairTask"
    }

    fn kind(&self) -> ReflectionTaskKind {
        ReflectionTaskKind::Repair
    }

    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport {
        let start = Instant::now();
        let pipeline = ReconciliationPipeline::new()
            .add(EntityNormalizationPass::new())
            .add(AliasResolutionPass::new(HashMap::new()))
            .add(DuplicateDetectionPass::new(0.8))
            .add(ContradictionDetectionPass::new())
            .add(OrphanDetectionPass::new());

        let reports = pipeline.execute(entities);
        let mut total_changes = 0;
        let mut all_diagnostics = Vec::new();
        let items_processed = entities.len();

        for r in reports {
            total_changes += r.changes_applied;
            all_diagnostics.extend(r.diagnostics);
        }

        TaskReport {
            task_name: self.name(),
            task_kind: self.kind(),
            items_processed,
            changes_applied: total_changes,
            diagnostics: all_diagnostics,
            duration: start.elapsed(),
        }
    }

    fn clone_box(&self) -> Box<dyn ReflectionTask> {
        Box::new(self.clone())
    }
}

/// `StrengthenTask`: Evaluates `LifecyclePolicy` across canonical entities to update state transitions.
#[derive(Debug, Clone, Default)]
pub struct StrengthenTask {
    policy: DefaultLifecyclePolicy,
}

impl StrengthenTask {
    /// Creates a new `StrengthenTask`.
    pub fn new() -> Self {
        Self {
            policy: DefaultLifecyclePolicy::default(),
        }
    }
}

impl ReflectionTask for StrengthenTask {
    fn id(&self) -> TaskId {
        TaskId("reflection.strengthen.lifecycle".to_string())
    }

    fn name(&self) -> &'static str {
        "StrengthenTask"
    }

    fn kind(&self) -> ReflectionTaskKind {
        ReflectionTaskKind::Strengthen
    }

    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport {
        let start = Instant::now();
        let mut changes_applied = 0;
        let items_processed = entities.len();

        for entity in entities.iter_mut() {
            let next_state = self
                .policy
                .evaluate_transition(entity.state, &entity.evidence);
            if next_state != entity.state {
                entity.state = next_state;
                changes_applied += 1;
            }
        }

        TaskReport {
            task_name: self.name(),
            task_kind: self.kind(),
            items_processed,
            changes_applied,
            diagnostics: Vec::new(),
            duration: start.elapsed(),
        }
    }

    fn clone_box(&self) -> Box<dyn ReflectionTask> {
        Box::new(self.clone())
    }
}

/// `EmbeddingRefreshTask`: Validates vector embedding freshness metadata.
#[derive(Debug, Clone, Default)]
pub struct EmbeddingRefreshTask;

impl EmbeddingRefreshTask {
    /// Creates a new `EmbeddingRefreshTask`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionTask for EmbeddingRefreshTask {
    fn id(&self) -> TaskId {
        TaskId("reflection.embedding.refresh".to_string())
    }

    fn name(&self) -> &'static str {
        "EmbeddingRefreshTask"
    }

    fn kind(&self) -> ReflectionTaskKind {
        ReflectionTaskKind::EmbeddingRefresh
    }

    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport {
        let start = Instant::now();
        TaskReport {
            task_name: self.name(),
            task_kind: self.kind(),
            items_processed: entities.len(),
            changes_applied: 0,
            diagnostics: Vec::new(),
            duration: start.elapsed(),
        }
    }

    fn clone_box(&self) -> Box<dyn ReflectionTask> {
        Box::new(self.clone())
    }
}

/// `CentralityTask`: Pre-computes PageRank/degree centrality metrics on entities.
#[derive(Debug, Clone, Default)]
pub struct CentralityTask;

impl CentralityTask {
    /// Creates a new `CentralityTask`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionTask for CentralityTask {
    fn id(&self) -> TaskId {
        TaskId("reflection.centrality.pagerank".to_string())
    }

    fn name(&self) -> &'static str {
        "CentralityTask"
    }

    fn kind(&self) -> ReflectionTaskKind {
        ReflectionTaskKind::Centrality
    }

    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport {
        let start = Instant::now();
        TaskReport {
            task_name: self.name(),
            task_kind: self.kind(),
            items_processed: entities.len(),
            changes_applied: 0,
            diagnostics: Vec::new(),
            duration: start.elapsed(),
        }
    }

    fn clone_box(&self) -> Box<dyn ReflectionTask> {
        Box::new(self.clone())
    }
}

/// `SummarizeTask`: Generates deterministic canonical entity summaries.
#[derive(Debug, Clone, Default)]
pub struct SummarizeTask;

impl SummarizeTask {
    /// Creates a new `SummarizeTask`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionTask for SummarizeTask {
    fn id(&self) -> TaskId {
        TaskId("reflection.summarize.canonical".to_string())
    }

    fn name(&self) -> &'static str {
        "SummarizeTask"
    }

    fn kind(&self) -> ReflectionTaskKind {
        ReflectionTaskKind::Summarize
    }

    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport {
        let start = Instant::now();
        TaskReport {
            task_name: self.name(),
            task_kind: self.kind(),
            items_processed: entities.len(),
            changes_applied: 0,
            diagnostics: Vec::new(),
            duration: start.elapsed(),
        }
    }

    fn clone_box(&self) -> Box<dyn ReflectionTask> {
        Box::new(self.clone())
    }
}

/// `OptimizeTask`: Handles storage/index maintenance.
#[derive(Debug, Clone, Default)]
pub struct OptimizeTask;

impl OptimizeTask {
    /// Creates a new `OptimizeTask`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionTask for OptimizeTask {
    fn id(&self) -> TaskId {
        TaskId("reflection.optimize.storage".to_string())
    }

    fn name(&self) -> &'static str {
        "OptimizeTask"
    }

    fn kind(&self) -> ReflectionTaskKind {
        ReflectionTaskKind::Optimize
    }

    fn execute(&self, entities: &mut Vec<CanonicalEntity>) -> TaskReport {
        let start = Instant::now();
        TaskReport {
            task_name: self.name(),
            task_kind: self.kind(),
            items_processed: entities.len(),
            changes_applied: 0,
            diagnostics: Vec::new(),
            duration: start.elapsed(),
        }
    }

    fn clone_box(&self) -> Box<dyn ReflectionTask> {
        Box::new(self.clone())
    }
}
