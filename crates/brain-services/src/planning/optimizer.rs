//! Plan Optimization Engine (`PlanOptimizer`) operating over `PlanningIR` (Phase 7 Milestone 7.2).
//!
//! ### Architectural Invariants:
//! 1. `PlanOptimizer` operates **ONLY** on `PlanningIR` (the single mutable planning representation).
//! 2. `TaskPlan` compiled artifacts remain strictly **immutable**.
//! 3. Policy separation: Optimization passes take `&OptimizationPolicy` as input without hardcoding thresholds.
//! 4. Transformations report: `PlanOptimizer` emits `OptimizationReport` containing `applied_transformations`.
//! 5. Semantic equivalence & Evidence preservation: Evidence pointers are preserved/merged; provenance is never fabricated or lost for surviving tasks.
//! 6. Idempotency property: `Optimize(Policy, Optimize(Policy, IR)) == Optimize(Policy, IR)`.

use crate::planning::models::{PlanningIR, TaskId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configurable policy parameters for plan optimization passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationPolicy {
    /// Minimum confidence threshold for candidate retention.
    pub minimum_confidence: f32,
    /// Flag enabling redundant task elimination.
    pub enable_redundant_task_elimination: bool,
    /// Flag enabling branch consolidation.
    pub enable_branch_consolidation: bool,
}

impl Default for OptimizationPolicy {
    fn default() -> Self {
        Self {
            minimum_confidence: 0.20,
            enable_redundant_task_elimination: true,
            enable_branch_consolidation: true,
        }
    }
}

/// Description of a specific transformation applied by an optimization pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizationTransformation {
    /// Candidate task pruned due to low confidence score.
    CandidatePruned {
        /// Pruned task ID.
        task_id: TaskId,
        /// Confidence score of pruned task.
        confidence: f32,
    },
    /// Duplicate task merged into canonical task node.
    RedundantTaskMerged {
        /// Target merged task ID.
        task_id: TaskId,
        /// Master task ID retaining merged task's capabilities and evidence.
        merged_into: TaskId,
    },
    /// Duplicate alternative decomposition branch consolidated.
    BranchConsolidated {
        /// Removed duplicate branch index.
        branch_index: usize,
    },
}

/// Structured optimization report returned by `PlanOptimizer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationReport {
    /// Transformed intermediate representation.
    pub transformed_ir: PlanningIR,
    /// Applied transformations log.
    pub transformations: Vec<OptimizationTransformation>,
}

/// Trait implemented by individual, deterministic optimization passes.
pub trait OptimizationPass {
    /// Returns the pass identifier name.
    fn name(&self) -> &'static str;
    /// Executes the pass over `PlanningIR`, appending applied transformations.
    fn run(
        &self,
        ir: &mut PlanningIR,
        policy: &OptimizationPolicy,
    ) -> Vec<OptimizationTransformation>;
}

/// Pass 1: Prunes candidate task steps falling below `policy.minimum_confidence`.
#[derive(Debug, Clone, Default)]
pub struct ConfidenceMaximizationPass;

impl OptimizationPass for ConfidenceMaximizationPass {
    fn name(&self) -> &'static str {
        "ConfidenceMaximizationPass"
    }

    fn run(
        &self,
        ir: &mut PlanningIR,
        policy: &OptimizationPolicy,
    ) -> Vec<OptimizationTransformation> {
        let mut transformations = Vec::new();
        let mut retained_candidates = Vec::new();
        let original_candidates = std::mem::take(&mut ir.candidates);
        let total_count = original_candidates.len();

        for candidate in original_candidates {
            if candidate.confidence >= policy.minimum_confidence
                || (total_count == 1 && retained_candidates.is_empty())
            {
                retained_candidates.push(candidate);
            } else {
                transformations.push(OptimizationTransformation::CandidatePruned {
                    task_id: candidate.task_id,
                    confidence: candidate.confidence,
                });
            }
        }

        ir.candidates = retained_candidates;
        transformations
    }
}

/// Pass 2: Eliminates redundant candidate tasks with identical descriptions, merging evidence and capabilities.
#[derive(Debug, Clone, Default)]
pub struct RedundantTaskEliminationPass;

impl OptimizationPass for RedundantTaskEliminationPass {
    fn name(&self) -> &'static str {
        "RedundantTaskEliminationPass"
    }

    fn run(
        &self,
        ir: &mut PlanningIR,
        policy: &OptimizationPolicy,
    ) -> Vec<OptimizationTransformation> {
        if !policy.enable_redundant_task_elimination || ir.candidates.len() < 2 {
            return Vec::new();
        }

        let mut transformations = Vec::new();
        let mut unique_candidates = Vec::new();
        let mut seen_descriptions = std::collections::HashMap::new();

        for candidate in ir.candidates.drain(..) {
            if let Some(&existing_idx) = seen_descriptions.get(&candidate.description) {
                let master: &mut crate::planning::models::TaskCandidate =
                    &mut unique_candidates[existing_idx];

                // Merge required capabilities without duplicates
                for cap in candidate.required_capabilities {
                    if !master.required_capabilities.contains(&cap) {
                        master.required_capabilities.push(cap);
                    }
                }

                // Merge EvidenceRef pointers without losing provenance
                for ev in candidate.evidence {
                    if !master.evidence.contains(&ev) {
                        master.evidence.push(ev);
                    }
                }

                // Master retains maximum confidence score
                if candidate.confidence > master.confidence {
                    master.confidence = candidate.confidence;
                }

                transformations.push(OptimizationTransformation::RedundantTaskMerged {
                    task_id: candidate.task_id,
                    merged_into: master.task_id,
                });
            } else {
                seen_descriptions.insert(candidate.description.clone(), unique_candidates.len());
                unique_candidates.push(candidate);
            }
        }

        ir.candidates = unique_candidates;
        transformations
    }
}

/// Pass 3: Consolidates duplicate alternative decomposition branches.
#[derive(Debug, Clone, Default)]
pub struct BranchConsolidationPass;

impl OptimizationPass for BranchConsolidationPass {
    fn name(&self) -> &'static str {
        "BranchConsolidationPass"
    }

    fn run(
        &self,
        ir: &mut PlanningIR,
        policy: &OptimizationPolicy,
    ) -> Vec<OptimizationTransformation> {
        if !policy.enable_branch_consolidation || ir.alternative_decompositions.len() < 2 {
            return Vec::new();
        }

        let mut transformations = Vec::new();
        let mut unique_branches = Vec::new();
        let mut seen_branches = HashSet::new();

        for (idx, branch) in ir.alternative_decompositions.drain(..).enumerate() {
            if !seen_branches.insert(branch.clone()) {
                transformations
                    .push(OptimizationTransformation::BranchConsolidated { branch_index: idx });
            } else {
                unique_branches.push(branch);
            }
        }

        ir.alternative_decompositions = unique_branches;
        transformations
    }
}

/// Optimizer running deterministic passes in fixed sequence over `PlanningIR`.
#[derive(Default)]
pub struct PlanOptimizer {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl PlanOptimizer {
    /// Instantiates a new `PlanOptimizer` with default pass pipeline.
    pub fn new() -> Self {
        Self {
            passes: vec![
                Box::new(ConfidenceMaximizationPass),
                Box::new(RedundantTaskEliminationPass),
                Box::new(BranchConsolidationPass),
            ],
        }
    }

    /// Executes optimization passes deterministically over `PlanningIR`, returning `OptimizationReport`.
    pub fn optimize(&self, mut ir: PlanningIR, policy: &OptimizationPolicy) -> OptimizationReport {
        let mut all_transformations = Vec::new();

        for pass in &self.passes {
            let transformations = pass.run(&mut ir, policy);
            all_transformations.extend(transformations);
        }

        OptimizationReport {
            transformed_ir: ir,
            transformations: all_transformations,
        }
    }
}
