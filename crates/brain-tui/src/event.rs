use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use brain_core::events::StreamEvent;

/// Categorized terminal level operating system events.
pub enum TerminalEvent {
    /// Keyboard input event.
    Key(crossterm::event::KeyEvent),
    /// Mouse interaction event.
    Mouse(crossterm::event::MouseEvent),
    /// Terminal window resize width and height constraints.
    Resize(u16, u16),
}

/// Inbound application control and stream events.
pub enum AppEvent {
    /// Canonical execution stream event.
    Stream(StreamEvent),
    /// Diagnostic or transport level failure error.
    Error(String),
    /// Graceful UI shutdown request.
    Shutdown,
}

/// Combined event stream container.
pub enum Event {
    /// Terminal level hardware/OS input events.
    Terminal(TerminalEvent),
    /// Application runtime and stream notifications.
    App(AppEvent),
    /// Regular clock tick driving interface updates (e.g. animation frames).
    Tick,
}

/// Multiplexes operating system inputs and clock ticks onto an async receiver queue.
pub struct EventHandler {
    rx: UnboundedReceiver<Event>,
    _tx: UnboundedSender<Event>,
}

impl EventHandler {
    /// Spawns background tasks polling crossterm events and clock ticks.
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = unbounded_channel();
        let tx_clone = tx.clone();

        // Spawn Crossterm polling task
        tokio::spawn(async move {
            loop {
                if tx_clone.is_closed() {
                    break;
                }
                match crossterm::event::poll(Duration::from_millis(10)) {
                    Ok(true) => {
                        match crossterm::event::read() {
                            Ok(crossterm::event::Event::Key(key)) => {
                                let sent = tx_clone.send(Event::Terminal(TerminalEvent::Key(key)));
                                if sent.is_err() {
                                    break;
                                }
                            }
                            Ok(crossterm::event::Event::Mouse(mouse)) => {
                                let sent = tx_clone.send(Event::Terminal(TerminalEvent::Mouse(mouse)));
                                if sent.is_err() {
                                    break;
                                }
                            }
                            Ok(crossterm::event::Event::Resize(w, h)) => {
                                let sent = tx_clone.send(Event::Terminal(TerminalEvent::Resize(w, h)));
                                if sent.is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(false) => {}
                    Err(_) => break,
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        // Spawn clock tick generator task
        let tx_clone2 = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                if tx_clone2.is_closed() {
                    break;
                }
                interval.tick().await;
                if tx_clone2.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx, _tx: tx }
    }

    /// Receives the next multiplexed event from the input or tick queues.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_multiplexer_pacing() {
        let mut handler = EventHandler::new(Duration::from_millis(20));
        let first = handler.next().await;
        assert!(matches!(first, Some(Event::Tick)));
    }
}
