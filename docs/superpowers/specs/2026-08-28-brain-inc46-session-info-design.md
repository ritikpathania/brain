# Increment 46 — Active Session Composite Info Inspector (`/session info`) Design Specification

## Overview

Increment 46 introduces a runtime session diagnostic inspector command (`/session info`, `/session stats`, `/session status`) that provides a consolidated, truthful overview of the active session state maintained in `SessionController`.

```text
/session info (or /session stats, /session status)
      │
      ▼
AppShell.tsx (dispatches action 'session-info')
      │
      ▼
SessionController.reportSessionInfo()
      ├── Session ID & Title
      ├── Lifecycle status (pinned, archived, message count)
      ├── Active Model & Thinking Budget (configured vs effective)
      ├── Active System Instructions (custom text vs default)
      └── Latest Memory Context Telemetry (candidates count, tokens, latency)
              │
              ▼
SessionController.notice(formattedSummary)
```

---

## 1. Controller Method & Presentation

### `SessionController.reportSessionInfo(): void`

Assembles the current controller state into a clear multi-line notice:
- **Session Identity**: `Session: <sessionId> ("<title>")` (or `(ephemeral / unstarted)` if no sessionId).
- **Status & Counts**: `Status: active · <pinned/unpinned> · <archived/unarchived> · <N> messages`
- **Model & Reasoning**:
  - `Model: <modelId>`
  - If model does not support thinking: `Reasoning: n/a (model unsupported)` (or `n/a (configured: <budget>, but unsupported by model)`).
  - If model supports thinking and budget is set: `Reasoning: <budget> tokens (effective)`
  - If thinking is off: `Reasoning: off`
- **System Instructions**:
  - If custom prompt set: `Instructions: "<preview>"`
  - If default prompt: `Instructions: default engine prompt (no custom instructions)`
- **Context Telemetry**:
  - If latest telemetry present: `Context: <count> memories compiled (<tokenCount> tokens, <latencyMs>ms)`
  - If no turns executed yet: `Context: no context retrieval executed yet`

---

## 2. Command Surface & Aliases

### `packages/brain-shell/src/commands/builtin.ts`
- `/session info` — Show active session metadata, model steering, instructions, and context
- `/session stats` — Alias for `/session info`
- `/session status` — Alias for `/session info`

---

## 3. Verification Strategy

1. **Unit Tests (`sessionInfoLogic.test.ts`)**:
   - Tests default session state output.
   - Tests custom system instructions formatting.
   - Tests thinking budget formatting (supported vs unsupported model).
   - Tests context retrieval telemetry inclusion.
2. **Canonical Live PTY Smoke Test (`scripts/ptySmokeInc46.py`)**:
   - Boots live daemon & shell.
   - Sets custom system prompt (`/system You are a test agent`).
   - Runs `/session info`.
   - Asserts session identity, model, instructions, and reasoning appear in notice.
3. **Full 23-suite canonical PTY matrix**.
4. **Clean detached worktree `cargo check --workspace`**.
5. **WIP baseline preservation**: 3,938 paths byte-identical.
