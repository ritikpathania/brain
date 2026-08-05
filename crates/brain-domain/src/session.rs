//! Immutable ReasoningSession aggregate and ReasoningSessionStage transition state machine.

use crate::errors::DomainError;
use crate::evolution::EvolutionPlan;
use crate::execution::ExecutionId;
use crate::reasoning_reflection::ReflectionReport;
use crate::synthesis::ReasoningResult;

use std::fmt;
use uuid::Uuid;

/// Strongly-typed identifier for a reasoning session.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ReasoningSessionId(pub Uuid);

impl ReasoningSessionId {
    /// Instantiates a new unique `ReasoningSessionId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ReasoningSessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ReasoningSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sess-{}", self.0.simple())
    }
}

/// Declarative lifecycle stages for a ReasoningSession.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReasoningSessionStage {
    /// Initial stage: reasoning plan generated.
    Planning,
    /// Execution stage: DAG step runner active.
    Executing,
    /// Synthesis stage: ReasoningResult derived.
    Synthesized,
    /// Reflection stage: ReflectionReport derived.
    Reflected,
    /// Evolution stage: EvolutionPlan derived.
    EvolutionPlanned,
    /// Terminal stage: session workflow complete.
    Completed,
}

impl fmt::Display for ReasoningSessionStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Planning => write!(f, "Planning"),
            Self::Executing => write!(f, "Executing"),
            Self::Synthesized => write!(f, "Synthesized"),
            Self::Reflected => write!(f, "Reflected"),
            Self::EvolutionPlanned => write!(f, "EvolutionPlanned"),
            Self::Completed => write!(f, "Completed"),
        }
    }
}

/// Declarative session transitions consumed by value to advance session stage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionTransition {
    /// Transition from Planning to Executing stage.
    StartExecution,
    /// Transition from Executing to Synthesized stage with ReasoningResult payload.
    AttachReasoningResult(ReasoningResult),
    /// Transition from Synthesized to Reflected stage with ReflectionReport payload.
    AttachReflectionReport(ReflectionReport),
    /// Transition from Reflected to EvolutionPlanned stage with EvolutionPlan payload.
    AttachEvolutionPlan(EvolutionPlan),
    /// Terminal transition to Completed stage.
    Complete,
}

/// Immutable top-level domain aggregate coordinating reasoning session lifecycle.
///
/// Invariants:
/// - `ReasoningSession` is a workflow coordinator, not a heavy data container.
/// - Contains at most one `ReasoningResult`, at most one `ReflectionReport`, and at most one `EvolutionPlan`.
/// - Transitions consume `self` by value and enforce strict stage progression without mutable setters.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningSession {
    /// Unique session identifier.
    pub id: ReasoningSessionId,
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// Current lifecycle stage.
    pub stage: ReasoningSessionStage,
    /// Attached reasoning result (populated at Synthesized stage).
    pub reasoning_result: Option<ReasoningResult>,
    /// Attached reflection report (populated at Reflected stage).
    pub reflection_report: Option<ReflectionReport>,
    /// Attached evolution plan (populated at EvolutionPlanned stage).
    pub evolution_plan: Option<EvolutionPlan>,
}

impl ReasoningSession {
    /// Instantiates a new `ReasoningSession` in the `Planning` stage.
    pub fn new(execution_id: ExecutionId) -> Self {
        Self {
            id: ReasoningSessionId::new(),
            execution_id,
            stage: ReasoningSessionStage::Planning,
            reasoning_result: None,
            reflection_report: None,
            evolution_plan: None,
        }
    }

    /// Advances the session lifecycle state via immutable consumption of `self`.
    pub fn transition(mut self, transition: SessionTransition) -> Result<Self, DomainError> {
        match (self.stage, transition) {
            (SessionTransitionStageMatch::Planning, SessionTransition::StartExecution) => {
                self.stage = ReasoningSessionStage::Executing;
                Ok(self)
            }
            (
                SessionTransitionStageMatch::Executing,
                SessionTransition::AttachReasoningResult(res),
            ) => {
                if res.execution_id != self.execution_id {
                    return Err(DomainError::ValidationError {
                        message: format!(
                            "ExecutionId mismatch: expected {}, got {}",
                            self.execution_id, res.execution_id
                        ),
                        rule_id: Some("VAL-SESS-001".to_string()),
                    });
                }
                self.reasoning_result = Some(res);
                self.stage = ReasoningSessionStage::Synthesized;
                Ok(self)
            }
            (
                SessionTransitionStageMatch::Synthesized,
                SessionTransition::AttachReflectionReport(rep),
            ) => {
                if rep.execution_id != self.execution_id {
                    return Err(DomainError::ValidationError {
                        message: format!(
                            "ExecutionId mismatch: expected {}, got {}",
                            self.execution_id, rep.execution_id
                        ),
                        rule_id: Some("VAL-SESS-002".to_string()),
                    });
                }
                self.reflection_report = Some(rep);
                self.stage = ReasoningSessionStage::Reflected;
                Ok(self)
            }
            (
                SessionTransitionStageMatch::Reflected,
                SessionTransition::AttachEvolutionPlan(plan),
            ) => {
                if plan.execution_id != self.execution_id {
                    return Err(DomainError::ValidationError {
                        message: format!(
                            "ExecutionId mismatch: expected {}, got {}",
                            self.execution_id, plan.execution_id
                        ),
                        rule_id: Some("VAL-SESS-003".to_string()),
                    });
                }
                self.evolution_plan = Some(plan);
                self.stage = ReasoningSessionStage::EvolutionPlanned;
                Ok(self)
            }
            (SessionTransitionStageMatch::EvolutionPlanned, SessionTransition::Complete) => {
                self.stage = ReasoningSessionStage::Completed;
                Ok(self)
            }
            (curr, trans) => Err(DomainError::ValidationError {
                message: format!("Invalid stage transition {:?} from stage {}", trans, curr),
                rule_id: Some("VAL-SESS-004".to_string()),
            }),
        }
    }
}

// Internal type alias helper for clean pattern matching
type SessionTransitionStageMatch = ReasoningSessionStage;
