# Specification — Brain Real Product Integration Plan

> **Document Status**: Approved Architecture & Implementation Plan  
> **Target Subsystems**: `packages/brain-frontend` & `sdks/typescript`  
> **Presentation Shell Status**: `🔒 FROZEN FRONTEND INFRASTRUCTURE`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
==================================================
BRAIN PRODUCT INTEGRATION ARCHITECTURE
==================================================
1. Presentation Shell: 100% Frozen (React 18 + Ink + Yoga)
2. Translation Layer: BrainFrontendAdapter (UDS -> PresentationState)
3. Product Semantics: Brain Daemon + Relational Memory Engine + Tool Orchestrator
```

---

## 1. Current State & Runtime Contracts

### A. Brain Runtime APIs & UDS Wire Contracts
- **UDS Socket Path**: `~/.brain/daemon.sock` (or configurable via `BRAIN_SOCKET_PATH`).
- **Wire Framing**: JSON Lines (`\n` terminated JSON objects).
- **Inbound Event Types**:
  - `stream_start`: Execution initiation.
  - `stream_progress`: Reasoning & retrieval stage progress updates.
  - `stream_chunk`: Incremental token chunks for assistant markdown responses.
  - `stream_end`: Completion payload with metadata and token metrics.
  - `stream_cancelled`: Interruption confirmation.
  - `error`: Diagnostics error payload.
  - `tool_request`: Authorization request for tool execution.
  - `tool_result`: Tool execution output/error.

### B. Current Presentation Boundary
- `PresentationState`: Canonical, UI-agnostic snapshot consumed by the frozen React component tree.
- `BrainFrontendAdapter`: Ingests UDS events and updates `PresentationState` synchronously.

---

## 2. Discovered Integration Gaps

| Area | Current Frontend State | Brain Production Requirement | Integration Work Item |
| :--- | :--- | :--- | :--- |
| **UDS Transport Client** | Mock/Adapter Ingestion API | Live Unix Domain Socket event loop | Wire Node.js / Bun `net.createConnection` to `BrainFrontendAdapter`. |
| **Tool Approval Flow** | `pending` badge display | Interactive approval decision (`y` / `n`) | Send approval decision wire message to daemon socket. |
| **Slash Commands** | Command palette modal list | Execution of `/help`, `/status`, `/config`, `/clear`, `/exit` | Implement client-side command router in adapter. |
| **Session Switching** | Static session ID | Dynamic session creation and continuation | Implement session selection & restore via daemon protocol. |
| **Workspace Context** | Header working directory string | Active workspace path & graph index status | Fetch workspace status from daemon on startup. |

---

## 3. Proposed Implementation Phases

### Phase 3.1: Live UDS Stream Client & Connection Lifecycle
- Implement `BrainUdsClient` wrapper connecting to `~/.brain/daemon.sock`.
- Handle auto-reconnect on socket drop (`connecting` $\rightarrow$ `connected` $\rightarrow$ `disconnected`).
- Wire incoming UDS JSON lines directly into `BrainFrontendAdapter.handleUdsMessage()`.

### Phase 3.2: Interactive Tool Approvals & Command Router
- Support interactive keyboard decision (`Enter` / `y` to approve, `Esc` / `n` to deny) for `pending` tool calls.
- Route slash commands (`/help`, `/status`, `/clear`, `/exit`) through adapter without requiring network roundtrips for local actions.

### Phase 3.3: Multi-turn Session & Workspace Persistence
- Implement session creation, restore, and history playback into `PresentationState.timeline`.
- Display active workspace root and graph indexing state in header and status bar.

### Phase 3.4: Production E2E Verification
- Run complete live session against active Brain daemon.
- Verify real query, live token streaming, tool execution, session continuation, and exit.

---

## 4. Architectural Boundary Invariants

> **RULE**: The React + Ink presentation components MUST NOT import:
> - SQLite
> - `brain-storage`
> - `brain-domain`
> - `ApplicationRuntime`
> - `net` / UDS socket classes directly.
>
> All translation logic is strictly confined to `packages/brain-frontend/src/adapter/`.

---

## 5. Risks, Mitigation & Rollback Strategy

- **Risk**: Socket disconnection during active streaming.
  - **Mitigation**: Adapter catches socket error, preserves timeline, sets `connection.status = 'disconnected'`, and initiates backoff reconnection.
- **Risk**: Presentation component regressions.
  - **Mitigation**: Frontend components are locked as frozen infrastructure. All changes occur exclusively in the adapter and UDS transport modules.
- **Rollback Strategy**: If transport integration encounters defects, fallback to fixture mode or previous known-good adapter commit without touching presentation code.

---

## 6. Exact Next Step

Awaiting user review and approval of this implementation plan before initiating **Phase 3.1: Live UDS Stream Client & Connection Lifecycle**.
