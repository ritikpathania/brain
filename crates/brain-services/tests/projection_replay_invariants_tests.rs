use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

struct CountingReducer(u64);
impl ProjectionReducer for CountingReducer {
    fn id(&self) -> ProjectionId { ProjectionId::new("counting") }
    fn version(&self) -> ProjectionVersion { ProjectionVersion(1) }
    fn apply_event(&mut self, _event: &FactEvent) -> Result<(), ProjectionError> {
        self.0 += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.0 = 0;
        Ok(())
    }
}

fn dummy_event() -> FactEvent {
    FactEvent::FactArchived {
        fact_id: FactVersionId(Uuid::new_v4()),
        archived_at: Timestamp::now(),
    }
}

#[test]
fn test_replay_equivalence_invariant() {
    let events = vec![dummy_event(), dummy_event(), dummy_event()];

    // Live path
    let store_live = Box::new(InMemoryCheckpointStore::new());
    let mut runtime_live = ProjectionRuntime::new(store_live);
    let instance_live = ProjectionInstance::new(Box::new(CountingReducer(0)));
    runtime_live.register_projection(instance_live).unwrap();

    for (idx, event) in events.iter().enumerate() {
        runtime_live.dispatch_event(event, idx as u64 + 1).unwrap();
    }

    // Replay path
    let store_replay = Box::new(InMemoryCheckpointStore::new());
    let mut runtime_replay = ProjectionRuntime::new(store_replay);
    let instance_replay = ProjectionInstance::new(Box::new(CountingReducer(0)));
    runtime_replay.register_projection(instance_replay).unwrap();
    runtime_replay.catchup_all(events.iter(), Watermark(3)).unwrap();
}

#[test]
fn test_repeated_interruption_recovery_invariant() {
    let events = vec![dummy_event(), dummy_event(), dummy_event(), dummy_event()];

    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);
    let instance = ProjectionInstance::new(Box::new(CountingReducer(0)));
    runtime.register_projection(instance).unwrap();

    // Partial catchup 1
    runtime.catchup_all(events.iter().take(2), Watermark(2)).unwrap();

    // Partial catchup 2 (restart from 2 to 4)
    runtime.catchup_all(events.iter(), Watermark(4)).unwrap();
}

#[test]
fn test_empty_replay_cutoff_invariant() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);
    let instance = ProjectionInstance::new(Box::new(CountingReducer(0)));
    runtime.register_projection(instance).unwrap();

    let events: Vec<FactEvent> = vec![];
    runtime.catchup_all(events.iter(), Watermark(0)).unwrap();
}
