//! Observability subscriber \u2014 `std::thread`-based blocking event consumer.
//!
//! `ObservabilitySubscriber` owns a `std::sync::mpsc::Receiver` and spawns one
//! background thread that blocks on `recv()`. On each `TaskProgress` event it appends
//! to the shared `CorrelationIndex`. The thread terminates automatically when the
//! sender side of the channel is dropped (graceful shutdown requires no explicit signal).
//!
//! The subscriber does NOT use `tokio`. It deliberately relies on `std::sync::mpsc`
//! to stay sync-first, deterministic, and easy to test.

use crate::correlation::CorrelationIndex;
use brain_core::events::{RuntimeEvent, TaskProgress};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;

/// Background subscriber that feeds `TaskProgress` events into a `CorrelationIndex`.
///
/// Construction creates and immediately spawns the background thread. The thread
/// runs until the `Receiver` is closed (i.e. the sending side is dropped).
pub struct ObservabilitySubscriber {
    index: Arc<Mutex<CorrelationIndex>>,
    /// Join handle for the background thread. Held so the thread is joined on drop.
    handle: Option<thread::JoinHandle<()>>,
}

impl ObservabilitySubscriber {
    /// Creates a new `ObservabilitySubscriber` and spawns its background thread.
    ///
    /// `rx` must be a `std::sync::mpsc::Receiver` over `Arc<dyn RuntimeEvent>`.
    /// The subscriber will run until `rx` is closed.
    pub fn new(rx: Receiver<Arc<dyn RuntimeEvent>>, index: Arc<Mutex<CorrelationIndex>>) -> Self {
        let index_clone = Arc::clone(&index);
        let handle = thread::Builder::new()
            .name("brain-observability-subscriber".to_string())
            .spawn(move || {
                loop {
                    match rx.recv() {
                        Ok(event) => {
                            // Downcast to TaskProgress; silently ignore other event types
                            if let Some(progress) = event.as_any().downcast_ref::<TaskProgress>() {
                                if let Ok(mut idx) = index_clone.lock() {
                                    idx.ingest(progress);
                                }
                            }
                        }
                        Err(_) => {
                            // Channel closed — sender dropped. Exit cleanly.
                            break;
                        }
                    }
                }
            })
            .expect("Failed to spawn brain-observability-subscriber thread");

        Self {
            index,
            handle: Some(handle),
        }
    }

    /// Returns a reference to the shared `CorrelationIndex` for querying.
    pub fn index(&self) -> Arc<Mutex<CorrelationIndex>> {
        Arc::clone(&self.index)
    }
}

impl Drop for ObservabilitySubscriber {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
