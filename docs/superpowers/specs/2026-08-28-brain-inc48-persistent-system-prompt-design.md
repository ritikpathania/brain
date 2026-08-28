# Increment 48 — Persistent Per-Session System Prompt (SQLite Migration v17) Design Specification

## Overview

Increment 48 closes the persistence seam for custom session system prompt instructions (`/system` / `/instructions`), making them durable across session resumes and process restarts via SQLite migration v17.

```text
/system "You are an expert Rust engineer"
        │
        ▼
SessionController.setSystemPrompt()
        │
        ├── sets ephemeral controller state
        └── calls client.setSessionSystemPrompt(sessionId, prompt)
                │
                ▼
v1/session/update_instructions (and during v1/generation/stream)
        │
        ▼
SqliteStorage (Migration v17: sessions.system_prompt TEXT)
        │
        ▼
/resume <session_id>
        │
        ▼
v1/session/load (returns SessionDetailDto.systemPrompt)
        │
        ▼
SessionController.resumeSession() (restores this.systemPrompt)
```

---

## 1. Domain Model (`crates/brain-domain/src/entities.rs`)

### `Session` Aggregate
- Field: `pub system_prompt: Option<String>`
- `Session::new`: initializes with `system_prompt: None`.
- `Session::reconstruct`: accepts `system_prompt: Option<String>` (or provides builder method).
- `Session::set_system_prompt(&mut self, prompt: Option<String>, timestamp: SessionTimestamp)`: normalizes empty strings to `None` and records timestamp.
- `Session::fork`: copies `system_prompt: self.system_prompt.clone()`.

---

## 2. Storage Migration v17 (`crates/brain-storage/src/migrations.rs`)

### DDL & Schema Updates
- `MIGRATIONS[0]` (new databases): `CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, history TEXT NOT NULL, updated_at INTEGER NOT NULL, system_prompt TEXT);`
- `apply_v17_add_session_system_prompt_column(conn: &mut Connection) -> Result<(), BrainError>`:
  - Checks if version 17 is applied in `_schema_migrations`.
  - Checks `PRAGMA table_info(sessions)` for `system_prompt`.
  - If missing: executes `ALTER TABLE sessions ADD COLUMN system_prompt TEXT;`.
  - Records version 17 in `_schema_migrations`.

### Session Repository (`crates/brain-storage/src/store.rs`)
- `save_session_conn`:
  - `INSERT OR REPLACE INTO sessions (id, history, updated_at, system_prompt) VALUES (?, ?, ?, ?)`
  - Passes `session.system_prompt`.
- `load_session_conn`:
  - Deserializes `Session` from `history` JSON (with `#[serde(default)] system_prompt`).

---

## 3. Daemon Protocol & UDS Handlers

### `daemon/src/server/protocol.rs`
- `SessionDetailDto`: add `#[serde(rename = "systemPrompt", default, skip_serializing_if = "Option::is_none")] pub system_prompt: Option<String>`
- `SessionSummaryDto`: add `#[serde(rename = "systemPrompt", default, skip_serializing_if = "Option::is_none")] pub system_prompt: Option<String>`
- `StartSessionPayload`: add `#[serde(rename = "systemPrompt", alias = "system_prompt", default)] pub system_prompt: Option<String>`
- `SetSessionSystemPromptPayload`: `{ sessionId: String, systemPrompt: Option<String> }`

### `daemon/src/transport/uds/handlers.rs`
- `v1/session/load`: includes `system_prompt: session.system_prompt.clone()`.
- `v1/session/list`: includes `system_prompt: session.system_prompt.clone()`.
- `v1/session/start`: sets `session.system_prompt`.
- `v1/session/fork`: preserves `system_prompt` on forked aggregate.
- `v1/generation/stream`: updates `session_aggregate.system_prompt` from `stream_req.system_prompt`.
- `v1/session/update_instructions` (or `v1/session/set_system_prompt`): loads session, calls `set_system_prompt()`, saves aggregate.

---

## 4. Shell & Session Controller

### `packages/brain-shell/src/client/`
- `BrainBackendClient.ts`: `BrainSession` and `BrainSessionSummary` include `systemPrompt?: string | null`. Add `setSessionSystemPrompt?(sessionId: string, prompt: string | null): Promise<void>`.
- `UdsBrainBackendClient.ts`: maps `systemPrompt` in `loadSession()` and implements `setSessionSystemPrompt()`.

### `packages/brain-shell/src/state/sessionController.ts`
- `resumeSession(sessionId)`: sets `this.systemPrompt = session.systemPrompt ?? null;` before notifying or rendering.
- `setSystemPrompt(prompt)`: if `this.sessionId` exists, asynchronously persists via `this.client.setSessionSystemPrompt?.(this.sessionId, this.systemPrompt)`.

---

## 5. Verification Matrix

1. **Rust Tests (`crates/brain-storage/` & `crates/brain-domain/`)**:
   - V17 migration execution and restart idempotency.
   - Backward compatibility for pre-v17 database (loading sessions with NULL `system_prompt`).
   - Session serialization round-trip with Some/None prompt.
   - Fork preserves `system_prompt`.
2. **Bun Unit Tests (`persistentSystemPromptLogic.test.ts`)**:
   - `resumeSession` restores `systemPrompt`.
   - `setSystemPrompt` calls `setSessionSystemPrompt` when `sessionId` is set.
   - Resumed custom prompt flows into subsequent `submit()` generation request.
3. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc48.py`)**:
   - Sets custom system prompt `/system "You are a compiler engineer"`.
   - Runs generation turn to verify prompt is accepted and persisted.
   - Restarts shell/daemon or resumes session `/resume <id>`.
   - Inspects `/session info` -> confirms `System Instructions: “You are a compiler engineer”` is preserved.
4. **25-Suite Canonical Regression Matrix**.
5. **Clean worktree compilation & 3,938 WIP paths preserved**.
