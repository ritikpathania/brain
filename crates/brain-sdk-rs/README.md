# brain-sdk-rs

## Purpose
Official Rust client SDK for communicating with brain-daemon over Unix Domain Sockets.

## Responsibilities
* Provide async client interface (`BrainClient`) for daemon IPC.
* Handle socket connection management, reconnects, and streaming response frames.

## Boundaries & Constraints
* **Allowed Dependencies:** `tokio`, `serde`, `serde_json`, `thiserror`.
* **Forbidden Dependencies:** `brain-storage`, `brain-services`, `brain-tui`, `pyo3`.

## Public API & Facades
* `BrainClient`, `ClientConfig`, `StreamReceiver`.

## Invariants Protected
* Stateless SDK client decoupling.

## Canonical References
* Specification: `../../docs/reference/protocol.md`

## Testing & Verification
* `cargo test -p brain-sdk-rs`

## Maintainer
See `CODEOWNERS`.
