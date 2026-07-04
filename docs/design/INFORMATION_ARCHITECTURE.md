# Information Architecture

This document specifies the Information Architecture (IA) of the Brain TUI client, detailing what information is shown, where it resides, and under what conditions it is disclosed.

---

## 1. Information Visibility Categories

The TUI separates information into three distinct visibility tiers based on frequency of use and cognitive load:

```
┌─────────────────────────────────────────────────────────┐
│                     Always Visible                      │
│  [Logo/Header]           [Sidebar]        [Chat Pane]   │
│  [Editor Input]                                         │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                       Contextual                        │
│  [Diff Panels]      [Tool Cards]     [Dialogs] [Toasts] │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                         Hidden                          │
│  [Analytics]   [Raw Socket Data]  [System Metrics]      │
└─────────────────────────────────────────────────────────┘
```

---

## 2. Structural Breakdown

### 2.1. Always Visible Elements
These elements establish the TUI chrome and core workspace interface. They are persistently rendered on screen under normal terminal layout conditions (width >= 80).

1. **Header / Logo Bar**
   * *Contents*: System brand label (`BRAIN v2 Engine`), client/daemon connection status, and active session identity.
2. **Sidebar (Navigation)**
   * *Contents*: Scrollable list of active session titles, creation timestamps, and selection indicators. (Automatically hidden when terminal width falls below 80 columns).
3. **Chat Pane (Conversation Log)**
   * *Contents*: Message history stream showing alternating role labels (`User`, `Assistant`, `System`), text segments, and plan checklists.
4. **Editor Prompt (Input)**
   * *Contents*: Text buffer editing region, vertical prompt markers, and cursor position.
5. **Footer Bar**
   * *Contents*: Contextual shortcuts based on active focus and state.

---

### 2.2. Contextual Elements
These panels and elements are overlayed, embedded, or expanded inline only in response to specific system events or user selections.

1. **Tool Execution Cards**
   * *Placement*: Embedded inline within the Chat Pane at the position of occurrence.
   * *Contents*: Tool name, parameters, execution status (success, error, running), and abbreviated stderr/stdout logs.
2. **Diff Blocks**
   * *Placement*: Expands below a file-edit proposal.
   * *Contents*: Line-by-line diff comparing proposed vs. current code with semantic green/red background highlights.
3. **Confirmation & Permission Dialogs**
   * *Placement*: Centered modal overlays covering a portion of the screen.
   * *Contents*: Security/trust warning message, clear action prompt, and explicit `[y/N]` indicators.
4. **Toast Notifications**
   * *Placement*: Rendered in the top-right corner of the terminal screen.
   * *Contents*: Non-blocking asynchronous alerts (e.g. "Background task complete", "File synced"). Automatically disappear after a brief timeout.

---

### 2.3. Hidden Elements
These elements are isolated from the standard layout to maintain visual focus. They are never rendered in the main interface unless requested via dedicated diagnostic subcommands or developer toggles.

1. **Analytical Metrics**
   * *Examples*: Cache hit rate, IPC round-trip latency, average query processing microseconds, queue depth.
   * *Access*: Accessible only by querying the daemon HTTP analytical endpoints (e.g., `http://127.0.0.1:8080/metrics`).
2. **Raw Protocol JSON Streams**
   * *Examples*: Raw websocket/UDS bytes, serialized event envelopes, database transactions.
   * *Access*: Captured strictly within tracing log files (controlled by `LOG_LEVEL` and `LOG_FORMAT` env variables).
3. **Telemetry Logs**
   * *Examples*: DuckDB analytical sync events, lock-free trace queues.
   * *Access*: Processed asynchronously and exported directly to DuckDB files without showing up on screen.
