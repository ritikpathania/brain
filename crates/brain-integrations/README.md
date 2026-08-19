# brain-integrations

## Purpose
External protocol integrations and transport adapters (UDS, HTTP metrics).

## Responsibilities
* Handle Unix Domain Socket connection frames and codec encoding/decoding.
* Expose Prometheus HTTP metrics and health check probes.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `tokio`, `serde_json`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* UDS codec handlers, metrics endpoint server.

## Invariants Protected
* Transport fidelity and zero synthetic test filtering (Invariant 7, 8).

## Canonical References
* Specification: `../../docs/reference/protocol.md`

## Testing & Verification
* `cargo test -p brain-integrations`

## Maintainer
See `CODEOWNERS`.
