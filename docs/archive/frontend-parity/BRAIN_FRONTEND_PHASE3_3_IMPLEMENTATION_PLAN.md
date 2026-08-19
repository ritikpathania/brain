# Architecture & Investigation Plan — Phase 3.3: Multi-turn Session & Workspace Persistence

> **Document Status**: Investigation & Implementation Plan  
> **Target Subsystems**: `packages/brain-frontend/src/adapter/BrainFrontendAdapter.ts`, `BrainFrontendController.ts`, `BrainUdsClient.ts`  
> **Presentation Shell Status**: `🔒 FROZEN FRONTEND INFRASTRUCTURE` (Zero changes to `components/**` or `types/**`)  
> **Backend Code Status**: `🔒 ZERO BACKEND RUST CHANGES`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
==================================================
PHASE 3.3 ARCHITECTURE & INVESTIGATION PLAN
==================================================
GOAL: Multi-turn Session & Workspace Persistence behind the Adapter/Controller Boundary
FROZEN INFRASTRUCTURE: components/** & types/presentation.ts (100% Locked)
RUST BACKEND: Zero modifications (reusing existing UDS protocol)
BOUNDARY: React/Ink (Presentation) -> Controller (Action) -> Adapter (State) -> UDS Client (Transport) -> Daemon
```

---

## 1. Existing Protocol & Runtime Analysis

### A. Session Enumeration & Management Protocol (from `crates/brain-tui/src/client.rs`)
- **Action**: `list_sessions`
- **Request Framing**:
  ```json
  {"action": "list_sessions", "payload": ""}
  ```
- **Daemon Wire Response**:
  ```json
  {
    "status": "ok",
    "message": "[{\"id\":\"sess_abc123\",\"title\":\"Relational Memory Session\",\"updated_at\":1723580000,\"pinned\":false,\"archived\":false}]"
  }
  ```

### B. Session Switching & Execution Context
- **Execution Request**:
  ```json
  {
    "action": "execute",
    "sessionId": "<session_id>",
    "prompt": "<query>",
    "workspaceContext": {
      "workingDir": "/Users/ritikpathania/Developer/PyCharm/brain"
    }
  }
  ```

---

## 2. Component Boundary & Data Flow

```text
┌────────────────────────────────────────────────────────┐
│     🔒 Frozen React + Ink + Yoga Shell                 │
│     (packages/brain-frontend/src/components/*)         │
└───────────────────────────┬────────────────────────────┘
                            │ Pure PresentationState Consumption
                            ▼
┌────────────────────────────────────────────────────────┐
│               BrainFrontendController                  │
│   - switchSession(sessionId, title, history)           │
│   - loadActiveWorkspace(workingDir, memoryStatus)      │
│   - restoreSessionHistory(messages)                    │
└───────────────────────────┬────────────────────────────┘
                            │ Method Invocations
                            ▼
┌────────────────────────────────────────────────────────┐
│               BrainFrontendAdapter                     │
│   - setSessionInfo(id, title, workingDir)              │
│   - restoreTimeline(messages: PresentationMessage[])   │
│   - setMemoryStatus(status: string)                    │
└───────────────────────────┬────────────────────────────┘
                            │ Send / Stream
                            ▼
┌────────────────────────────────────────────────────────┐
│               BrainUdsClient                           │
│   - fetchSessions(): Promise<SessionSummary[]>         │
└───────────────────────────┬────────────────────────────┘
                            │ ~/.brain/daemon.sock
                            ▼
┌────────────────────────────────────────────────────────┐
│               Brain Runtime Daemon                     │
└────────────────────────────────────────────────────────┘
```

---

## 3. Implementation Design

### A. `BrainFrontendAdapter` Enhancements (No `components/**` or `types/**` changes)
1. **`restoreTimeline(messages: PresentationMessage[])`**:
   - Replaces `this.state.timeline` with the provided historical message list.
   - Clears active streaming state, resets thinking drawer state, and recalculates unread/sticky indicators.
   - Idempotent: repeated restorations with identical content do not duplicate messages.
2. **`setMemoryStatus(memoryStatus: string)`**:
   - Updates `this.state.footer.memoryStatus`.

### B. `BrainFrontendController` Enhancements
1. **`switchSession(sessionId: string, title?: string, history?: PresentationMessage[])`**:
   - Sets the active session metadata in `PresentationState`.
   - Restores the session history in `state.timeline`.
   - Clears prompt buffer and scroll drift.
2. **`restoreWorkspace(workingDir: string, memoryStatus?: string)`**:
   - Updates `state.session.workingDir` and header title.

### C. `BrainUdsClient` Enhancements
1. **`listSessions(): Promise<Array<{ id: string; title: string; updatedAt: number }>>`**:
   - Dispatches `{"action":"list_sessions","payload":""}` over UDS and parses the response.
   - Gracefully handles connection drops or daemon offline states without crashing.

---

## 4. Test Strategy

We will create `packages/brain-frontend/src/test/sessionPersistence.test.ts` covering:
1. **Startup restores an existing session**: Restores messages into `timeline`.
2. **Empty session handling**: Initializes clean timeline without errors.
3. **Message ordering preservation**: Preserves chronological order of user and assistant messages.
4. **Session switching**: Switching session ID replaces timeline with target session history.
5. **Switching to empty session**: Clears existing timeline cleanly.
6. **Workspace restoration**: Updates `workingDir` and header title.
7. **Daemon unavailable handling**: Gracefully handles offline socket; maintains local state.
8. **Malformed/partial response handling**: Does not crash; logs error.
9. **Idempotency**: Repeated restoration calls do not duplicate messages.
10. **Zero Regression**: All existing 60 tests (fixtures, adapter, UDS client, controller) continue passing.

---

## 5. Verification & Freeze Guardrails

- `bun test`: All 60+ unit and integration tests must pass.
- `cargo check`: Rust library crates must compile with code 0.
- `git diff`: Confirm zero modifications in `packages/brain-frontend/src/components/**` and `packages/brain-frontend/src/types/presentation.ts`.
