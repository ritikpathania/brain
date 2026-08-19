# Phase 4.2 Forensic Protocol Audit: Interactive Session Switcher

> **Document Status**: Forensic Protocol Audit (Audit Only — Pre-Implementation)  
> **Audited Capability**: Interactive Session Enumeration & Switching (`/sessions`, `/session <id>`)  
> **Audited Routes**: `list_sessions` / `v1/sessions/list`, `v1/sessions/get`  
> **Presentation Shell Status**: `🔒 FROZEN FRONTEND INFRASTRUCTURE` (Zero changes to `components/**` or `types/**`)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 4.2 FORENSIC PROTOCOL AUDIT
================================================================================
TARGET CAPABILITIES: /sessions (list), /session <id> (switch active context)
PROTOCOL STATUS: VERIFIED — All wire endpoints exist in daemon & SQLite storage
RUST BACKEND IMPACT: NONE (Zero modifications required)
FROZEN SHELL IMPACT: NONE (Zero modifications to components/** or types/**)
PROPOSED IMPLEMENTATION: BrainFrontendController + BrainUdsClient
FINAL VERDICT: PROCEED TO IMPLEMENTATION
================================================================================
```

---

## 1. Existing Session Wire Protocol & Endpoints

| Operation | Wire Action | Request Frame | Response Frame | Backend Delegate |
|---|---|---|---|---|
| **Enumerate Sessions** | `"list_sessions"` / `"v1/sessions/list"` | `{"action":"list_sessions","payload":""}` | `{"status":"ok","message":"[{\"id\":\"...\",\"title\":\"...\",\"updated_at\":...}]"}` | `SqliteSessionReadModelRepository::list_all()` |
| **Load Session History** | `"v1/sessions/get"` | `{"version":"1.0","type":"Request","id":1,"action":"v1/sessions/get","body":"{\"session_id\":\"...\"}"}` | `{"version":"1.0","type":"Response","id":1,"status":"success","body":"[{\"id\":\"...\",\"role\":\"...\",\"content\":\"...\",\"timestamp\":...}]"}` | `SessionRepository::load_session()` $\rightarrow$ SQLite `sessions` table |

Both endpoints were added, tested, and certified during Phase 3.3 and require **zero modifications** in the Rust backend.

---

## 2. Answers to Audit Questions

### 1. Does the daemon already expose everything required?
**Yes.** The daemon exposes `list_sessions` (for session summaries: `id`, `title`, `updated_at`, `is_pinned`, `is_archived`) and `v1/sessions/get` (for full message arrays with `id`, `role`, `content`, `timestamp`).

### 2. Does `list_sessions` return enough information for a useful session picker?
**Yes.** Each entry contains `id` (SessionId UUID), `title` (human-readable title), `updated_at` (epoch timestamp), `pinned` (boolean), and `archived` (boolean).

### 3. Is there already a presentation-state representation capable of displaying a session list?
**Yes.** System messages in `PresentationState.timeline` (`role: 'system'`) render formatted multiline text, tables, and lists. The `/sessions` command can format the active session list cleanly as a system message.

### 4. Can `/sessions` be implemented as a timeline/system-message interaction?
**Yes.** Formatting the enumerated sessions as a system message allows users to see available sessions and their IDs without modifying the frozen presentation shell.

### 5. Can `/session <id>` reuse the existing `switchSession()` path exactly?
**Yes.** `BrainFrontendController.switchSession(sessionId, title, messages)` already updates `state.session.id`, `state.session.title`, and invokes `adapter.restoreTimeline(messages)`.

### 6. Does switching sessions correctly reset all volatile states?
**Yes.** In `BrainFrontendAdapter.restoreTimeline()`:
- `state.streaming.isStreaming = false`
- `state.streaming.activeText = ''`
- `state.scroll.unseenCount = 0`
- `state.scroll.stickyPromptText = null`
- `this.currentAssistantMessageId = null`
- `this.currentToolCalls.clear()` and re-indexes only the restored messages' tools.

### 7. Does session switching preserve workspace context correctly?
**Yes.** `state.session.workingDir` and `state.footer.memoryStatus` are preserved across `switchSession()`.

### 8. Failure & Edge-Case Behavior:
- **Nonexistent Session ID**: `loadSession(id)` returns `[]`. The controller can notify the user: `Switched to new/empty session <id>`.
- **Zero Messages**: Restores an empty timeline cleanly (`state.timeline = []`).
- **Malformed History**: JSON parsing error in `loadSession()` is trapped and returns `[]` safely.
- **Daemon Disconnect During Switching**: Returns current state gracefully without crashing.
- **Switching While Streaming**: `restoreTimeline()` immediately sets `isStreaming = false` and nulls `currentAssistantMessageId`, cleanly halting token appending from the previous session.
- **Rapid A $\rightarrow$ B $\rightarrow$ A Switching**: Each switch is atomic and replaces `state.timeline`.

### 9. Protocol Gap?
**No.** Zero protocol gaps exist.

### 10. Presentation Model Gap?
**No.** `PresentationState.session` and `PresentationState.timeline` fully represent the required state.

### 11. Controller / State Management Gap?
**No.** Only minor slash command routing additions in `BrainFrontendController.handleSlashCommand()` (`/sessions`, `/session <id>`) and `listSessionsFormatted()` helper are needed.

---

## 3. Layer Impact & Invariant Protection

| Layer | Impact / Changes Required | Invariant Preserved |
|---|---|---|
| **`packages/brain-frontend/src/components/**`** | **ZERO (0 lines)** | `🔒 100% Frozen Shell` |
| **`packages/brain-frontend/src/types/**`** | **ZERO (0 lines)** | `🔒 100% Frozen Types` |
| **Rust Backend (`crates/*`, `daemon/*`)** | **ZERO (0 lines)** | `🔒 Existing UDS Routes Reused` |
| **`BrainUdsClient`** | Already implements `listSessions()` and `loadSession()` | Reused directly |
| **`BrainFrontendController`** | Add `/sessions` and `/session <id>` routing | Interaction layer only |
| **`BrainFrontendAdapter`** | Already implements `restoreTimeline()` | State translation layer only |

---

## 4. Proposed Implementation Scope (For Phase 4.2 Implementation Step)

When approved to implement:
1. Extend `BrainFrontendController.ts`:
   - Add `public async listSessions(): Promise<void>` $\rightarrow$ queries `udsClient.listSessions()`, formats active session list with timestamps and indicator for active session, and injects into timeline.
   - Add `public async switchActiveSession(sessionId: string): Promise<void>` $\rightarrow$ queries `udsClient.loadSession(sessionId)` and calls `switchSession()`.
   - Wire `/sessions` and `/session <id>` in `handleSlashCommand()`.
   - Update `/help` command text.
2. Add Unit & Integration Tests:
   - Test `/sessions` online & offline.
   - Test `/session <id>` switching, invalid IDs, empty sessions, and subsequent prompt execution.
3. Verify `bun test` and `cargo check`.

---

## 5. Final Audit Verdict

```text
================================================================================
FINAL VERDICT:
PROCEED TO IMPLEMENTATION
================================================================================
```
