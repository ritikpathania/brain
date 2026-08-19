# Keybindings

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`, strictly subordinate to and governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).


This document defines the canonical keyboard shortcuts, key combination behaviors, scoping rules, and conflict resolution policies for the Brain TUI client.

---

## 1. Global Keybindings Matrix

| Shortcut | Scope / Focus | Action / Behavior | Conflict Resolution |
| :--- | :--- | :--- | :--- |
| **Tab** | Global | Cycle focus forward (`Editor` -> `Sidebar` -> `Chat`) | Always active, cannot be consumed by editor buffer. |
| **Shift+Tab** | Global | Cycle focus backward | Always active. |
| **Ctrl+C** | Global | **Idle**: Hard exit of client process.<br>**Busy**: Cancels active stream/tool execution; returns to Idle. | Always active. Intercepted by terminal signal handler first. |
| **Esc** | Global | Dismisses any active dialog/modal. If no modal is open: cancels active stream or clears input focus. | Resets focus state before performing cancellations. |
| **Enter** | Contextual | **Editor**: Submits prompt buffer.<br>**Sidebar**: Loads selected session history.<br>**Dialog**: Confirms default selection. | Editor consumes Enter when editing, unless submitting. |
| **Shift+Enter**| Editor | Inserts a literal newline character `\n` into the editor buffer. | Falls back to prompt submission if multi-line is unsupported. |
| **Up Arrow** | Contextual | **Editor**: Recalls previous prompt in history.<br>**Sidebar**: Navigates selection up.<br>**Chat**: Scrolls viewport up. | Overridden by active component focus. |
| **Down Arrow** | Contextual | **Editor**: Recalls next prompt in history.<br>**Sidebar**: Navigates selection down.<br>**Chat**: Scrolls viewport down. | Overridden by active component focus. |
| **PageUp** | Chat | Scrolls chat viewport up by a half-page increment. | Inactive when modals are focused. |
| **PageDown** | Chat | Scrolls chat viewport down by a half-page increment. | Inactive when modals are focused. |
| **Ctrl+R** | Global | Force-reconnects Unix socket and refreshes active session data. | Non-blocking background reconnect trigger. |
| **Ctrl+P** | Global | Displays command palette overlay containing slash commands. | Suspends input prompt typing while palette is visible. |
| **Ctrl+L** | Global | Clears terminal screen buffer and triggers a full UI redraw. | Refreshes crossterm alternate screen buffer. |
| **Ctrl+K** | Global | Detaches active session and schedules the process as a background task. | Valid only if connection mode is `Daemon`. |
| **F1** / **?** | Global | Opens the interactive keyboard shortcuts help overlay modal. | Dismissed via `Esc` or clicking/pressing `F1` again. |

---

## 2. Event Multiplexing & Dispatch Rules

1. **Global Intercepts**: Key events are checked against Global shortcuts (`Ctrl+C`, `Ctrl+R`, `Ctrl+L`, `F1`, `Tab`) before being dispatched to the focused widget.
2. **Focus Isolation**: When a widget has focus (e.g. the prompt editor), standard typing characters (`a-z`, `0-9`, symbols) must flow strictly into that widget's internal state reducer.
3. **Modal Dominance**: When a modal dialog (e.g. permission request, keybindings help) is active, all keyboard inputs except `Esc` and dialog navigation keys (e.g. `Tab`, `Arrow keys`, `Enter`) must be blocked.
