# brain-session

## Purpose
Interactive conversation session tracking, history management, and context pinning.

## Responsibilities
* Manage active conversation sessions, message history buffers, and context attachments.
* Coordinate session lifecycle events (creation, active, archived).

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* `SessionManager`, `ConversationState`, `SessionContext`.

## Invariants Protected
* Session domain encapsulation.

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-session`

## Maintainer
See `CODEOWNERS`.
