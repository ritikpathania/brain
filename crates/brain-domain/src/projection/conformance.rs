//! Automated conformance testing trait and harness for ProjectionReducers.

use crate::bkf::events::FactEvent;
use crate::projection::reducer::ProjectionReducer;
use std::fmt::Debug;

/// Trait exposing inspectable domain state for automated projection conformance testing.
pub trait ProjectionStateView: ProjectionReducer {
    /// The underlying state type, which must be default-initializable, cloneable, and comparable.
    type State: Clone + PartialEq + Debug + Default;

    /// Returns a reference to the reducer's current materialized state.
    fn state(&self) -> &Self::State;
}

/// Automated conformance suite testing fundamental ProjectionReducer invariants.
pub struct ProjectionConformanceSuite;

impl ProjectionConformanceSuite {
    /// Asserts that reset() restores the reducer state to bitwise Default state.
    pub fn assert_reset_clears_state<R>(mut reducer: R, sample_events: &[FactEvent])
    where
        R: ProjectionStateView,
    {
        for event in sample_events {
            reducer
                .apply_event(event)
                .expect("sample event should apply successfully");
        }
        reducer.reset().expect("reset should succeed");
        assert_eq!(
            reducer.state(),
            &R::State::default(),
            "State after reset() must equal Default::default()"
        );
    }

    /// Asserts that applying duplicate events produces identical materialized state (idempotency).
    pub fn assert_duplicate_event_idempotency<R>(mut reducer: R, event: &FactEvent)
    where
        R: ProjectionStateView,
    {
        reducer
            .apply_event(event)
            .expect("first event should apply successfully");
        let state_after_first = reducer.state().clone();
        reducer
            .apply_event(event)
            .expect("duplicate event should apply successfully");
        assert_eq!(
            reducer.state(),
            &state_after_first,
            "State after duplicate event must be identical to state after single event"
        );
    }

    /// Asserts that two independent reducers processing identical event streams arrive at identical state.
    pub fn assert_replay_equivalence<R>(mut reducer1: R, mut reducer2: R, events: &[FactEvent])
    where
        R: ProjectionStateView,
    {
        for event in events {
            reducer1
                .apply_event(event)
                .expect("event should apply successfully on reducer 1");
        }
        for event in events {
            reducer2
                .apply_event(event)
                .expect("event should apply successfully on reducer 2");
        }
        assert_eq!(
            reducer1.state(),
            reducer2.state(),
            "Replayed reducer states must be 100% identical"
        );
    }
}
