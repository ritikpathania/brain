//! Read models and execution context for Runtime Composition: RuntimeContext, ReasoningPhaseReport, StewardshipPhaseReport, and RuntimeExecutionReport.

use crate::consolidation_decision::ConsolidationReport;
use crate::execution::ExecutionId;
use crate::mutation::{StewardshipAuditLog, StewardshipExecutionSummary};
use crate::reasoning_reflection::ReflectionReport;
use crate::session::ReasoningSession;
use crate::synthesis::ReasoningResult;
use uuid::Uuid;

/// Immutable runtime execution context passing trace parameters across subsystem boundaries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeContext {
    /// Distributed trace identifier.
    pub trace_id: Uuid,
    /// Execution run identifier.
    pub execution_id: ExecutionId,
}

impl RuntimeContext {
    /// Instantiates a new `RuntimeContext`.
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            execution_id: ExecutionId::new(),
        }
    }

    /// Instantiates a `RuntimeContext` with explicit execution ID.
    pub fn with_execution_id(execution_id: ExecutionId) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            execution_id,
        }
    }
}

impl Default for RuntimeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Grouped read model containing outputs produced during the reasoning phase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningPhaseReport {
    /// Synthesized reasoning result.
    pub reasoning_result: Option<ReasoningResult>,
    /// Derived reflection critique report.
    pub reflection_report: Option<ReflectionReport>,
    /// Derived consolidation report.
    pub consolidation_report: Option<ConsolidationReport>,
}

impl ReasoningPhaseReport {
    /// Instantiates a new `ReasoningPhaseReport`.
    pub fn new(
        reasoning_result: Option<ReasoningResult>,
        reflection_report: Option<ReflectionReport>,
        consolidation_report: Option<ConsolidationReport>,
    ) -> Self {
        Self {
            reasoning_result,
            reflection_report,
            consolidation_report,
        }
    }
}

/// Grouped read model containing outputs produced during the stewardship phase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StewardshipPhaseReport {
    /// Public caller execution summary.
    pub summary: Option<StewardshipExecutionSummary>,
    /// Persistent audit log.
    pub audit_log: Option<StewardshipAuditLog>,
}

impl StewardshipPhaseReport {
    /// Instantiates a new `StewardshipPhaseReport`.
    pub fn new(
        summary: Option<StewardshipExecutionSummary>,
        audit_log: Option<StewardshipAuditLog>,
    ) -> Self {
        Self { summary, audit_log }
    }
}

/// Immutable top-level read model capturing complete execution provenance across a reasoning cycle.
///
/// Invariants:
/// - Read model for composition and telemetry observation; not a transactional aggregate.
/// - Every externally observable state transition must be reconstructable from immutable execution artifacts, reports, and audit logs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeExecutionReport {
    /// Execution run identifier.
    pub execution_id: ExecutionId,
    /// Reasoning session aggregate.
    pub session: ReasoningSession,
    /// Grouped reasoning phase report.
    pub reasoning: ReasoningPhaseReport,
    /// Grouped stewardship phase report.
    pub stewardship: StewardshipPhaseReport,
}

impl RuntimeExecutionReport {
    /// Instantiates a new immutable `RuntimeExecutionReport`.
    pub fn new(
        execution_id: ExecutionId,
        session: ReasoningSession,
        reasoning: ReasoningPhaseReport,
        stewardship: StewardshipPhaseReport,
    ) -> Self {
        Self {
            execution_id,
            session,
            reasoning,
            stewardship,
        }
    }
}
