//! Read-only active `ReflectionEngineV2` and pass implementations (Phase 6 Milestone 6.1).

use crate::reflection::input::ReflectionInput;
use crate::reflection::models::{
    ConfidenceDecayDetails, ContradictionDetails, DuplicateEntityDetails, FindingId,
    ReflectionFindingKind, ReflectionFindingV2, ReflectionReportV2,
};
use uuid::Uuid;

/// Classification kind for reflection passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReflectionPassKind {
    /// Deterministic pass (contradictions, orphans, structural checks).
    Deterministic,
    /// Heuristic pass (similarity threshold duplicate merging, semantic clustering).
    Heuristic,
}

/// Trait defining a read-only monotonic reflection pass operating over `ReflectionInput`.
pub trait ReflectionPassV2: Send + Sync {
    /// Human-readable pass name.
    fn name(&self) -> &'static str;
    /// Classification kind (Deterministic or Heuristic).
    fn pass_kind(&self) -> ReflectionPassKind;
    /// Analyzes an immutable `ReflectionInput` snapshot and returns domain findings.
    fn analyze(&self, input: &ReflectionInput) -> Vec<ReflectionFindingV2>;
}

/// Deterministic pass detecting attribute contradictions across entities.
#[derive(Debug, Clone, Default)]
pub struct ContradictionDetectionPassV2;

impl ContradictionDetectionPassV2 {
    /// Instantiates a new `ContradictionDetectionPassV2`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionPassV2 for ContradictionDetectionPassV2 {
    fn name(&self) -> &'static str {
        "ContradictionDetectionPassV2"
    }

    fn pass_kind(&self) -> ReflectionPassKind {
        ReflectionPassKind::Deterministic
    }

    fn analyze(&self, input: &ReflectionInput) -> Vec<ReflectionFindingV2> {
        let mut findings = Vec::new();

        for entity in &input.entities {
            if entity.properties.contains_key("conflict") {
                findings.push(ReflectionFindingV2 {
                    id: FindingId(Uuid::new_v4()),
                    kind: ReflectionFindingKind::AttributeContradiction(ContradictionDetails {
                        entity_id: entity.id.clone(),
                        conflicting_fact_ids: Vec::new(),
                        description: format!(
                            "Conflicting property detected on entity '{}'",
                            entity.canonical_name
                        ),
                    }),
                    confidence: 0.95,
                });
            }
        }

        findings
    }
}

/// Heuristic pass detecting duplicate entity candidates via name similarity.
#[derive(Debug, Clone, Default)]
pub struct DuplicateMergePassV2;

impl DuplicateMergePassV2 {
    /// Instantiates a new `DuplicateMergePassV2`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionPassV2 for DuplicateMergePassV2 {
    fn name(&self) -> &'static str {
        "DuplicateMergePassV2"
    }

    fn pass_kind(&self) -> ReflectionPassKind {
        ReflectionPassKind::Heuristic
    }

    fn analyze(&self, input: &ReflectionInput) -> Vec<ReflectionFindingV2> {
        let mut findings = Vec::new();

        for i in 0..input.entities.len() {
            for j in (i + 1)..input.entities.len() {
                let e1 = &input.entities[i];
                let e2 = &input.entities[j];

                if e1.canonical_name.to_lowercase() == e2.canonical_name.to_lowercase() {
                    findings.push(ReflectionFindingV2 {
                        id: FindingId(Uuid::new_v4()),
                        kind: ReflectionFindingKind::DuplicateEntity(DuplicateEntityDetails {
                            entity_ids: vec![e1.id.clone(), e2.id.clone()],
                            similarity_score: 1.0,
                        }),
                        confidence: 0.99,
                    });
                }
            }
        }

        findings
    }
}

/// Deterministic pass evaluating confidence decay over historical entities.
#[derive(Debug, Clone, Default)]
pub struct ConfidenceDecayPassV2;

impl ConfidenceDecayPassV2 {
    /// Instantiates a new `ConfidenceDecayPassV2`.
    pub fn new() -> Self {
        Self
    }
}

impl ReflectionPassV2 for ConfidenceDecayPassV2 {
    fn name(&self) -> &'static str {
        "ConfidenceDecayPassV2"
    }

    fn pass_kind(&self) -> ReflectionPassKind {
        ReflectionPassKind::Deterministic
    }

    fn analyze(&self, input: &ReflectionInput) -> Vec<ReflectionFindingV2> {
        let mut findings = Vec::new();

        for entity in &input.entities {
            if entity.confidence < 0.5 {
                findings.push(ReflectionFindingV2 {
                    id: FindingId(Uuid::new_v4()),
                    kind: ReflectionFindingKind::ConfidenceDecay(ConfidenceDecayDetails {
                        entity_id: entity.id.clone(),
                        old_confidence: entity.confidence as f32,
                        new_confidence: (entity.confidence * 0.9) as f32,
                    }),
                    confidence: 0.80,
                });
            }
        }

        findings
    }
}

/// Active read-only `ReflectionEngineV2` orchestrating analysis passes over a `ReflectionInput` snapshot.
pub struct ReflectionEngineV2 {
    passes: Vec<Box<dyn ReflectionPassV2>>,
}

impl Default for ReflectionEngineV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl ReflectionEngineV2 {
    /// Instantiates a standard `ReflectionEngineV2` with default deterministic and heuristic passes.
    pub fn new() -> Self {
        Self {
            passes: vec![
                Box::new(ContradictionDetectionPassV2::new()),
                Box::new(DuplicateMergePassV2::new()),
                Box::new(ConfidenceDecayPassV2::new()),
            ],
        }
    }

    /// Customizes passes configured for reflection analysis.
    pub fn add_pass(&mut self, pass: Box<dyn ReflectionPassV2>) {
        self.passes.push(pass);
    }

    /// Executes all read-only reflection passes over `ReflectionInput` snapshot to produce a `ReflectionReportV2`.
    pub fn run(&self, input: &ReflectionInput) -> ReflectionReportV2 {
        let mut findings = Vec::new();

        for pass in &self.passes {
            let pass_findings = pass.analyze(input);
            findings.extend(pass_findings);
        }

        ReflectionReportV2 {
            report_id: Uuid::new_v4(),
            snapshot_id: input.snapshot_id,
            findings,
            evaluated_entities_count: input.entities.len(),
            timestamp_ms: input.timestamp_ms,
        }
    }
}
