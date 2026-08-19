# brain-events

## Purpose
In-memory event bus and typed event envelope dispatching.

## Responsibilities
* Provide broadcast and mpsc async event channels for domain and stream events.
* Package domain events into versioned `EventEnvelope` wrappers with sequence IDs.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `tokio`, `serde`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* `EventBus`, `EventEnvelope`, `EventSubscriber`.

## Invariants Protected
* Strict separation between DomainEvent, EventEnvelope, and StreamEvent.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-events`

## Maintainer
See `CODEOWNERS`.
