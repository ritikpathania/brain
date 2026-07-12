use crate::NodeId;
use crate::retrieval::{RetrievedCandidate, RetrievalExplanation, RetrievalResult};

/// Telemetry identifier representing each stage of the query compilation and execution pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RetrievalStage {
    /// Vector database search stage.
    VectorSearch,
    /// Keyword text search stage.
    KeywordSearch,
    /// Neighborhood/structural graph expansion stage.
    GraphExpansion,
    /// Reciprocal Rank Fusion / Candidate merging stage.
    Fusion,
    /// Scoring, sorting, and final relevance ranking stage.
    Ranking,
}

/// Typed code/reason for retrieval execution completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompletionReason {
    /// Query successfully processed to completion.
    Finished,
    /// Query execution cancelled early.
    Cancelled,
}

/// Transport-agnostic, owned domain events representing intermediate execution states.
#[derive(Debug, Clone)]
pub enum RetrievalEvent {
    /// Emitted when starting execution of a pipeline stage.
    StageStarted {
        /// The telemetry stage type.
        stage: RetrievalStage,
    },
    /// Emitted when a candidate node is discovered.
    CandidateFound(RetrievedCandidate),
    /// Emitted when new explanation/provenance evidence is added to a candidate.
    ExplanationUpdated {
        /// Target node identifier.
        node_id: NodeId,
        /// New accumulated explanation detail.
        explanation: RetrievalExplanation,
    },
    /// Emitted when a pipeline stage finishes.
    StageCompleted {
        /// The telemetry stage type.
        stage: RetrievalStage,
    },
    /// Final stream event carrying execution metrics and ranked results.
    Completed {
        /// Completion code.
        reason: CompletionReason,
        /// Final retrieval results.
        result: RetrievalResult,
    },
}

/// Event receiver trait supporting stateful collectors, tests, and renderers.
pub trait RetrievalSink: Send {
    /// Mutably processes a single query compiler/executor lifecycle event.
    fn on_event(&mut self, event: RetrievalEvent);
}

/// A standard in-memory collector for recording emitted events for replay, tests, or telemetry.
#[derive(Debug, Default, Clone)]
pub struct RecordingSink {
    events: Vec<RetrievalEvent>,
}

impl RecordingSink {
    /// Constructs a new empty RecordingSink.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Returns a slice of all recorded events.
    pub fn events(&self) -> &[RetrievalEvent] {
        &self.events
    }

    /// Consumes the sink and returns the owned list of events.
    pub fn into_events(self) -> Vec<RetrievalEvent> {
        self.events
    }

    /// Clears all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl RetrievalSink for RecordingSink {
    fn on_event(&mut self, event: RetrievalEvent) {
        self.events.push(event);
    }
}
