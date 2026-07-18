use brain_core::events::{RuntimeEvent, RuntimeEventDispatcher};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// Concrete in-memory event dispatcher that coordinates subscriptions using bounded mpsc channels.
///
/// NOTE: This is the current event bus implementation for Sprint 2 / Sprint 3, conforming to:
/// `RuntimeEventDispatcher (trait)` ──► `InMemoryEventDispatcher (reference implementation)`
///
/// Subscription management via `subscribe()` is intentionally *not* on `RuntimeEventDispatcher`.
/// Callers that only dispatch events use `Arc<dyn RuntimeEventDispatcher>`. Callers that also
/// need to `subscribe()` keep a concrete `Arc<InMemoryEventDispatcher>` reference alongside.
///
/// Two subscription flavours are supported:
/// - `subscribe()` — returns a `tokio::sync::mpsc::Receiver` (async-compatible).
/// - `subscribe_sync()` — returns a `std::sync::mpsc::Receiver` (blocking, for `std::thread`).
pub struct InMemoryEventDispatcher {
    subscribers: Mutex<Vec<Sender<Arc<dyn RuntimeEvent>>>>,
    sync_subscribers: Mutex<Vec<std::sync::mpsc::SyncSender<Arc<dyn RuntimeEvent>>>>,
    buffer_capacity: usize,
}

impl InMemoryEventDispatcher {
    /// Creates a new `InMemoryEventDispatcher` with bounded capacity constraints.
    pub fn new(buffer_capacity: usize) -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            sync_subscribers: Mutex::new(Vec::new()),
            buffer_capacity,
        }
    }

    /// Subscribes to the event dispatcher, returning an async `tokio::sync::mpsc::Receiver`.
    pub fn subscribe(&self) -> Receiver<Arc<dyn RuntimeEvent>> {
        let (tx, rx) = channel(self.buffer_capacity);
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(tx);
        rx
    }

    /// Subscribes to the event dispatcher, returning a blocking `std::sync::mpsc::Receiver`.
    ///
    /// Use this when the consumer runs on a `std::thread` (e.g. `ObservabilitySubscriber`).
    pub fn subscribe_sync(&self) -> std::sync::mpsc::Receiver<Arc<dyn RuntimeEvent>> {
        let (tx, rx) = std::sync::mpsc::sync_channel(self.buffer_capacity);
        let mut subs = self.sync_subscribers.lock().unwrap();
        subs.push(tx);
        rx
    }

    /// Drops all held channel senders, closing both async and sync subscription channels.
    ///
    /// After this call, any downstream `recv()` or `try_recv()` loop will receive a
    /// disconnected error and should exit cleanly.
    pub fn shutdown(&self) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.clear();
        let mut sync_subs = self.sync_subscribers.lock().unwrap();
        sync_subs.clear();
    }

    /// Returns the count of currently active event subscribers.
    pub fn active_subscribers_count(&self) -> usize {
        let subs = self.subscribers.lock().unwrap();
        let sync_subs = self.sync_subscribers.lock().unwrap();
        subs.len() + sync_subs.len()
    }
}

impl RuntimeEventDispatcher for InMemoryEventDispatcher {
    fn dispatch(&self, event: Arc<dyn RuntimeEvent>) {
        // Async (tokio) subscribers
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| match tx.try_send(Arc::clone(&event)) {
            Ok(_) => true,
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => true,
        });
        drop(subs);

        // Sync (std::thread) subscribers
        let mut sync_subs = self.sync_subscribers.lock().unwrap();
        sync_subs.retain(|tx| match tx.try_send(Arc::clone(&event)) {
            Ok(_) => true,
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => false,
            Err(std::sync::mpsc::TrySendError::Full(_)) => true,
        });
    }
}
