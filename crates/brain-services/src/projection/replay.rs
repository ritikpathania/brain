//! Deterministic catch-up replay engine driven by abstract event iterators.

use crate::projection::instance::*;
use brain_domain::bkf::events::FactEvent;
use brain_domain::projection::*;

/// Catch-up replay engine.
pub struct ReplayEngine;

impl ReplayEngine {
    /// Replays events from an abstract iterator to catch up a projection to target watermark.
    pub fn replay_catchup<'a, I>(
        instance: &mut ProjectionInstance,
        event_iter: I,
        target_watermark: Watermark,
    ) -> Result<(), ProjectionError>
    where
        I: Iterator<Item = &'a FactEvent>,
    {
        let current_wm = instance.checkpoint().watermark;
        if current_wm >= target_watermark {
            instance.set_lifecycle(ProjectionLifecycle::Live);
            return Ok(());
        }

        instance.set_lifecycle(ProjectionLifecycle::Replaying);
        for (idx, event) in event_iter.enumerate() {
            let seq = current_wm.0 + idx as u64 + 1;
            if seq > target_watermark.0 {
                break;
            }
            instance.apply_event(event, seq)?;
        }

        instance.set_lifecycle(ProjectionLifecycle::Live);
        Ok(())
    }
}
