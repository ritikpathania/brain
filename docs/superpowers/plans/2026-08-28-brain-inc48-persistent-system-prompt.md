# Increment 48 — Persistent Per-Session System Prompt (SQLite Migration v17) Implementation Plan

## Proposed Changes

### 1. Domain Model (`crates/brain-domain/src/entities.rs`)
- Add `system_prompt: Option<String>` to `Session` struct.
- Update `Session::new_empty()`, `Session::new()`, `Session::reconstruct()`, `Session::fork()`.
- Add `Session::set_system_prompt(&mut self, prompt: Option<String>, timestamp: SessionTimestamp)`.
- Update tests in `crates/brain-domain/tests/`.

### 2. Storage Migration & Repository (`crates/brain-storage/`)
- In `crates/brain-storage/src/migrations.rs`:
  - Update `MIGRATIONS[0]` for new databases.
  - Implement `apply_v17_add_session_system_prompt_column(conn: &mut Connection) -> Result<(), BrainError>`.
  - Add migration test for V17.
- In `crates/brain-storage/src/store.rs`:
  - Update `save_session_conn` with `system_prompt` column.

### 3. Daemon Protocol & RPCs (`daemon/src/`)
- In `daemon/src/server/protocol.rs`:
  - Update `SessionDetailDto`, `SessionSummaryDto`, `StartSessionPayload`.
  - Add `SetSessionSystemPromptPayload`.
- In `daemon/src/transport/uds/handlers.rs`:
  - Update `session/load`, `session/list`, `session/start`, `session/fork`, `v1/generation/stream`.
  - Add `session/set_system_prompt` / `v1/session/update_instructions` handler.

### 4. Client SDK & Controller (`packages/brain-shell/`)
- In `packages/brain-shell/src/client/BrainBackendClient.ts` and `UdsBrainBackendClient.ts`:
  - Add `systemPrompt` to `BrainSession` and `BrainSessionSummary`.
  - Implement `setSessionSystemPrompt()`.
- In `packages/brain-shell/src/state/sessionController.ts`:
  - In `resumeSession()`: restore `this.systemPrompt = session.systemPrompt ?? null`.
  - In `setSystemPrompt()`: persist to daemon if session exists.

### 5. Tests & Verification
- Unit test: `packages/brain-shell/src/test/adapter/persistentSystemPromptLogic.test.ts`
- Live PTY test: `scripts/ptySmokeInc48.py`
- Canonical matrix: Incs 2 through 48 (25 suites).
- Rust tests & clean worktree check.
- WIP preservation.
