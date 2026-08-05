//! ReflectionService orchestrating critique report generation over ReasoningResults using pure ReflectionPolicies.

use crate::reasoning::reflection_policy::ReflectionPolicy;
use brain_domain::{DomainError, ReasoningResult, ReflectionReport};

/// Pure orchestration service for generating ReflectionReport aggregates.
/// Invariant: ReflectionService performs orchestration only; all critique logic lives inside ReflectionPolicy.
#[derive(Debug, Clone, Default)]
pub struct ReflectionService;

impl ReflectionService {
    /// Instantiates a new `ReflectionService`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates a `ReasoningResult` using a `ReflectionPolicy` to produce an immutable `ReflectionReport`.
    pub fn reflect(
        &self,
        result: &ReasoningResult,
        policy: &dyn ReflectionPolicy,
    ) -> Result<ReflectionReport, DomainError> {
        let findings = policy.evaluate(result);
        let report = ReflectionReport::new(result.execution_id, findings);
        report.validate()?;
        Ok(report)
    }
}
