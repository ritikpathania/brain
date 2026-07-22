use crate::dto::v1::{Event, StreamMessage};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Manages active subscription channels and handles broadcasting.
pub struct SubscriptionManager {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<StreamMessage>>>>,
    sequence_counter: AtomicU64,
}

impl SubscriptionManager {
    /// Creates a new `SubscriptionManager` initialized with a starting sequence counter.
    pub fn new(start_sequence: u64) -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
            sequence_counter: AtomicU64::new(start_sequence),
        }
    }

    /// Broadcasts a live event to all registered subscribers.
    /// Employs a strict backpressure policy: if a subscriber's buffer is full,
    /// it is immediately dropped from the active subscribers list.
    pub fn broadcast(&self, event: Event) {
        let seq = self.sequence_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| {
            tx.try_send(StreamMessage::Event {
                sequence: seq,
                event: event.clone(),
            })
            .is_ok()
        });
    }

    /// Registers a new active subscriber sender.
    pub fn register(&self, tx: mpsc::Sender<StreamMessage>) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(tx);
    }

    /// Returns the count of active subscribers.
    pub fn active_count(&self) -> usize {
        let subs = self.subscribers.lock().unwrap();
        subs.len()
    }
}

/// Opaque wrapper for the client subscription stream.
pub struct EventStream {
    rx: mpsc::Receiver<StreamMessage>,
}

impl EventStream {
    /// Creates a new `EventStream`.
    pub fn new(rx: mpsc::Receiver<StreamMessage>) -> Self {
        Self { rx }
    }

    /// Asynchronously awaits and returns the next message in the stream.
    pub async fn next(&mut self) -> Option<StreamMessage> {
        self.rx.recv().await
    }
}
