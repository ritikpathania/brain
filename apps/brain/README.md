# brain (CLI Application)

## Purpose
Primary user-facing command-line interface binary and interactive frontend launcher.

## Responsibilities
* Parse CLI command arguments and subcommands (daemon start/stop/status/run, ingest, query, ui).
* Resolve and launch the first-party `packages/brain-frontend` for `brain ui` from packaged product installation or development repository ancestry. `BRAIN_FRONTEND_ROOT` is only an explicit development override; `packages/brain-shell` is legacy/reference code and is not a fallback.
* Manage local background daemon lifecycle against canonical DaemonHost topology.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-core`, `brain-domain`, `brain-services`, `brain-config`.
* **Forbidden Dependencies:** `brain-storage`, `rusqlite`, `pyo3`.

## Public API & Facades
* CLI binary entry point `main()`.

## Invariants Protected
* Production composition root convergence (Invariant 4), zero direct SQLite/PyO3 dependencies.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo check -p brain`

## Maintainer
See `CODEOWNERS`.
