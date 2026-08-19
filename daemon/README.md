# brain-daemon (Background Daemon Service)

## Purpose
Long-running Unix Domain Socket (UDS) background daemon hosting the relational memory engine.

## Responsibilities
* Manage background daemon process lifecycle, PID files, and signal handling (SIGTERM, SIGINT).
* Expose non-blocking Unix Domain Socket server (`daemon.sock`) for IPC.
* Drive the single composition root `DaemonHost::run_server` via ApplicationRuntime.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-core`, `brain-domain`, `brain-application`, `brain-services`, `brain-integrations`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* Daemon binary entry point `main()` and `DaemonHost` runner.

## Invariants Protected
* Canonical composition root (Invariant 3), zero direct SQLite persistence (Invariant 1).

## Canonical References
* Specification: `../docs/reference/protocol.md`

## Testing & Verification
* `./daemon/.venv/bin/pytest daemon/tests`

## Maintainer
See `CODEOWNERS`.
