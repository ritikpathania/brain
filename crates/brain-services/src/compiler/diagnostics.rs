//! First-class compiler diagnostics emitted by Knowledge Compiler passes.

use serde::{Deserialize, Serialize};

/// Severity level of a compiler diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    /// Informational compilation insight.
    Info,
    /// Warning indicating potential ambiguity or low confidence.
    Warning,
    /// Critical compilation error requiring intervention.
    Error,
}

impl std::fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticLevel::Info => write!(f, "info"),
            DiagnosticLevel::Warning => write!(f, "warning"),
            DiagnosticLevel::Error => write!(f, "error"),
        }
    }
}

/// Structural category of compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DiagnosticKind {
    /// Incompatible or contradictory facts detected across observations.
    ConflictingFacts,
    /// Multiple canonical candidates for a single entity reference.
    AmbiguousIdentity,
    /// Fact or entity lacks supporting evidence provenance.
    MissingEvidence,
    /// Entity or fact confidence score below compilation threshold.
    LowConfidence,
    /// Entity node with no incoming or outgoing relations.
    OrphanConcept,
}

impl std::fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticKind::ConflictingFacts => write!(f, "conflicting_facts"),
            DiagnosticKind::AmbiguousIdentity => write!(f, "ambiguous_identity"),
            DiagnosticKind::MissingEvidence => write!(f, "missing_evidence"),
            DiagnosticKind::LowConfidence => write!(f, "low_confidence"),
            DiagnosticKind::OrphanConcept => write!(f, "orphan_concept"),
        }
    }
}

/// First-class compiler diagnostic emitted by a compiler pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Diagnostic structural classification.
    pub kind: DiagnosticKind,
    /// Target entity ID, fact ID, or scope identifier.
    pub target: String,
    /// Detailed diagnostic message.
    pub message: String,
    /// Actionable suggestion for resolution.
    pub suggestion: Option<String>,
}

impl Diagnostic {
    /// Instantiates a new compiler diagnostic.
    pub fn new(
        level: DiagnosticLevel,
        kind: DiagnosticKind,
        target: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            level,
            kind,
            target: target.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Attaches an actionable resolution suggestion.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}
