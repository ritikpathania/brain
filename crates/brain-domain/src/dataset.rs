//! Domain-neutral regression dataset models: BenchmarkScenario and EvaluationDataset.

use crate::runtime_report::RuntimeContext;
use std::fmt;
use uuid::Uuid;

/// Strongly-typed scenario identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ScenarioId(pub Uuid);

impl ScenarioId {
    /// Instantiates a new unique `ScenarioId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ScenarioId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ScenarioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scen-{}", self.0.simple())
    }
}

/// Individual benchmark evaluation scenario.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkScenario {
    /// Unique scenario identifier.
    pub id: ScenarioId,
    /// Human-readable scenario name.
    pub name: String,
    /// Input query string.
    pub query: String,
    /// Contextual execution context.
    pub context: RuntimeContext,
}

impl BenchmarkScenario {
    /// Instantiates a new `BenchmarkScenario`.
    pub fn new(name: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            id: ScenarioId::new(),
            name: name.into(),
            query: query.into(),
            context: RuntimeContext::new(),
        }
    }
}

/// Extensible collection of benchmark scenarios for regression and policy evaluation.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvaluationDataset {
    /// Dataset name identifier.
    pub name: String,
    /// List of benchmark scenarios.
    pub scenarios: Vec<BenchmarkScenario>,
}

impl EvaluationDataset {
    /// Instantiates a new `EvaluationDataset`.
    pub fn new(name: impl Into<String>, scenarios: Vec<BenchmarkScenario>) -> Self {
        Self {
            name: name.into(),
            scenarios,
        }
    }
}
