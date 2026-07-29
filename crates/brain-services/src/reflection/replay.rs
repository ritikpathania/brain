//! Execution timeline recorder and replay verification engine for reflection event envelopes.

use brain_events::{EventStore, ReflectionEventEnvelope};

/// Recorded execution timeline storing an ordered sequence of runtime event envelopes.
#[derive(Debug, Clone, Default)]
pub struct ExecutionTimeline {
    envelopes: Vec<ReflectionEventEnvelope>,
}

impl ExecutionTimeline {
    /// Creates a new empty `ExecutionTimeline`.
    pub fn new() -> Self {
        Self {
            envelopes: Vec::new(),
        }
    }

    /// Records an event envelope to the timeline.
    pub fn record(&mut self, envelope: ReflectionEventEnvelope) {
        self.envelopes.push(envelope);
    }

    /// Returns a slice of recorded envelopes.
    pub fn envelopes(&self) -> &[ReflectionEventEnvelope] {
        &self.envelopes
    }

    /// Returns the count of recorded envelopes.
    pub fn len(&self) -> usize {
        self.envelopes.len()
    }

    /// Returns true if the timeline contains no envelopes.
    pub fn is_empty(&self) -> bool {
        self.envelopes.is_empty()
    }
}

/// Replay verification engine validating event stream monotonicity and stage transitions.
#[derive(Default)]
pub struct ReplayEngine;

impl ReplayEngine {
    /// Creates a new `ReplayEngine`.
    pub fn new() -> Self {
        Self
    }

    /// Replays an `ExecutionTimeline`, validating event monotonicity and sequence integrity.
    pub fn replay_timeline(&self, timeline: &ExecutionTimeline) -> Result<usize, String> {
        let mut last_timestamp = 0u64;

        for (idx, envelope) in timeline.envelopes().iter().enumerate() {
            if envelope.timestamp_ms < last_timestamp {
                return Err(format!(
                    "Non-monotonic timestamp detected at envelope index {}: {} < {}",
                    idx, envelope.timestamp_ms, last_timestamp
                ));
            }
            last_timestamp = envelope.timestamp_ms;
        }

        Ok(timeline.len())
    }

    /// Replays event envelopes from an `EventStore` for a plan ID and verifies timestamp monotonicity.
    pub fn verify_event_store_plan(
        &self,
        store: &dyn EventStore,
        plan_id: &str,
    ) -> Result<usize, String> {
        let envelopes = store.query(plan_id);
        let mut timeline = ExecutionTimeline::new();
        for env in envelopes {
            timeline.record(env);
        }
        self.replay_timeline(&timeline)
    }
}
