//! Persistent SQLite Event Log Storage Implementation (Phase 12 Milestone 12.1).

use crate::planning::durable_event_store::{EventEnvelope, EventLog, SequenceNumber};
use crate::planning::event_codec::EventCodec;
use crate::planning::event_publisher::EventPublishError;
use brain_storage::PlanningSqliteEventLog;

struct StorageCodecAdapter<E, C>(C, std::marker::PhantomData<E>);

impl<E: Send + Sync, C: EventCodec<E>> brain_storage::EventCodec<E> for StorageCodecAdapter<E, C> {
    fn encode(&self, event: &E) -> Result<Vec<u8>, brain_storage::EventPublishError> {
        self.0
            .encode(event)
            .map_err(|e| brain_storage::EventPublishError::StorageError(e.to_string()))
    }

    fn decode(&self, bytes: &[u8]) -> Result<E, brain_storage::EventPublishError> {
        self.0
            .decode(bytes)
            .map_err(|e| brain_storage::EventPublishError::StorageError(e.to_string()))
    }
}

/// Persistent event log backend backed by SQLite storage.
pub struct SqliteEventLog<E, C> {
    inner: PlanningSqliteEventLog<E, StorageCodecAdapter<E, C>>,
}

impl<E: Send + Sync, C: EventCodec<E>> SqliteEventLog<E, C> {
    /// Opens or instantiates an `SqliteEventLog` at the given path (or `:memory:`).
    pub fn new(path: &str, codec: C) -> Result<Self, EventPublishError> {
        let adapter = StorageCodecAdapter(codec, std::marker::PhantomData);
        let inner = PlanningSqliteEventLog::new(path, adapter).map_err(|e| match e {
            brain_storage::EventPublishError::StorageError(s) => EventPublishError::StorageError(s),
            brain_storage::EventPublishError::SerializationError(s) => {
                EventPublishError::StorageError(s)
            }
            brain_storage::EventPublishError::DeserializationError(s) => {
                EventPublishError::StorageError(s)
            }
        })?;
        Ok(Self { inner })
    }
}

impl<E: Send + Sync, C: EventCodec<E>> EventLog<E> for SqliteEventLog<E, C> {
    fn append(
        &self,
        event: E,
        timestamp_ms: u64,
        schema_version: u16,
    ) -> Result<SequenceNumber, EventPublishError> {
        use brain_storage::PlanningEventLog;
        self.inner
            .append(event, timestamp_ms, schema_version)
            .map(|seq| SequenceNumber(seq.0))
            .map_err(|e| match e {
                brain_storage::EventPublishError::StorageError(s) => {
                    EventPublishError::StorageError(s)
                }
                brain_storage::EventPublishError::SerializationError(s) => {
                    EventPublishError::StorageError(s)
                }
                brain_storage::EventPublishError::DeserializationError(s) => {
                    EventPublishError::StorageError(s)
                }
            })
    }

    fn read_range(
        &self,
        start: SequenceNumber,
        limit: usize,
    ) -> Result<Vec<EventEnvelope<E>>, EventPublishError> {
        use brain_storage::PlanningEventLog;
        let items = self
            .inner
            .read_range(brain_storage::PlanningSequenceNumber(start.0), limit)
            .map_err(|e| match e {
                brain_storage::EventPublishError::StorageError(s) => {
                    EventPublishError::StorageError(s)
                }
                brain_storage::EventPublishError::SerializationError(s) => {
                    EventPublishError::StorageError(s)
                }
                brain_storage::EventPublishError::DeserializationError(s) => {
                    EventPublishError::StorageError(s)
                }
            })?;

        Ok(items
            .into_iter()
            .map(|env| EventEnvelope {
                sequence: SequenceNumber(env.sequence.0),
                timestamp_ms: env.timestamp_ms,
                schema_version: env.schema_version,
                payload: env.payload,
            })
            .collect())
    }

    fn last_sequence_number(&self) -> SequenceNumber {
        use brain_storage::PlanningEventLog;
        SequenceNumber(self.inner.last_sequence_number().0)
    }
}
