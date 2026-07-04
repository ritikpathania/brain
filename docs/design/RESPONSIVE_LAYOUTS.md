# Responsive Layouts

This document defines the responsive layout specifications, terminal dimensions breakpoints, panel resizing rules, and component collapsing behavior for the Brain TUI.

---

## 1. Terminal Dimension Breakpoints

The UI automatically reorganizes and partitions the terminal cells based on the screen width (`cols` or `width` parameter of the window frame):

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Width < 80 cols (Compact Mode)                                            │
│  [Logo / Header]                                                           │
│  [       Chat Pane (100% width)                                          ] │
│  [Editor Input]                                                            │
│  [Footer (Truncated)]                                                      │
└────────────────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────────────────┐
│ Width 80 - 120 cols (Standard Mode)                                        │
│  [Logo / Header]                                                           │
│  [ Sidebar (25 cols) ] [ Chat Pane (Remainder)                           ] │
│  [Editor Input]                                                            │
│  [Footer (Standard)]                                                       │
└────────────────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────────────────┐
│ Width > 120 cols (Wide Mode)                                               │
│  [Logo / Header]                                                           │
│  [ Sidebar (25 cols) ] [ Chat Pane (60%) ] [ Split Diff Panel (40%)      ] │
│  [Editor Input]                                                            │
│  [Footer (Extended)]                                                       │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Breakpoint Behaviors

### 2.1. Compact Mode (< 80 columns)
* **Sidebar**: Automatically hidden. The client drops the session list from rendering completely, freeing all horizontal space for the dialogue.
* **Header / Status**: Connection text degrades to compact symbols:
  * `[Connected: Daemon]` -> `[D]`
  * `[Connected: In-Process]` -> `[I]`
  * `[Disconnected]` -> `[X]`
* **Footer**: Displays only critical shortcuts: `[Esc: Exit | Enter: Submit]`.

### 2.2. Standard Mode (80–120 columns)
* **Sidebar**: Rendered persistently, locked to a width of exactly `25 columns`.
* **Chat Pane**: Occupies all remaining width (`total_width - 25`).
* **Header / Status**: Full labels rendered (e.g. `[Connected: Daemon]`).
* **Footer**: Renders standard shortcuts: `[Tab: Focus | Esc: Exit | Ctrl+C: Cancel | Enter: Submit]`.

### 2.3. Wide Mode (120–160 columns)
* **Sidebar**: Locked to a width of `25 columns`.
* **Chat Pane**: Left-aligned, taking up the bulk of the remaining screen.
* **Contextual Panels**: When a plan is executing or a file edit is proposed, instead of rendering inline inside the chat, the diff preview or plan checklist is drawn on the right half of the screen in a vertical split layout.

### 2.4. Ultra-Wide Mode (> 160 columns)
* **Three-Column Layout**:
  1. **Left Sidebar** (25 columns): Session list.
  2. **Center Chat Pane** (Flex-grow, minimum 80 columns): Conversational log.
  3. **Right Sidebar** (Flex-grow, minimum 55 columns): Persistent diagnostic panel showing live daemon logs, system metrics, and file diff previews side-by-side.
