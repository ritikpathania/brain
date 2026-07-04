# Design Invariants

This document establishes the strict frontend invariants for the Brain TUI client. These rules govern layout, rendering, input handling, and terminal states, serving as checklist criteria during PR reviews.

---

## 1. Input Focus Invariant
* **Statement**: Typing never loses focus.
* **Invariant**: The cursor and keyboard focus must remain within the `Input` editor prompt widget during active typing. State changes from the background daemon (e.g. session lists loading, background tool start/stop notifications) must never force the focus to shift away from the active input prompt. Focus only moves on explicit user action (e.g. `Tab` or `Esc`).

## 2. Scroll Determinism Invariant
* **Statement**: Viewport movement is deterministic.
* **Invariant**: The viewport container for the conversation log must maintain its position unless:
  1. The viewport is already at the very bottom of the document (`scroll_offset == max`), in which case incoming streaming chunks will automatically push the viewport to stick to the bottom.
  2. The user executes a manual scrolling shortcut (e.g. `PageUp`, `PageDown`, `Up`, `Down`).
* **Consequence**: Incoming streaming events must never cause the viewport to jump if the user is scrolling back to read history.

## 3. Append-Only / Immutability Invariant
* **Statement**: Streaming never rewrites completed output, and conversation ordering is immutable.
* **Invariant**: Once a stream token or chat message is committed to history, its content is immutable. The renderer cannot back-edit, truncate, or rewrite text segments.
* **Invariant**: Tool executions, bash shell outputs, and daemon notifications must append at the bottom of the timeline strictly in the order they occurred.

## 4. Input Preservation on Failure Invariant
* **Statement**: Errors never erase user input.
* **Invariant**: If a socket request, remote daemon communication, or local parsing execution fails, the input editor's buffer must NOT be cleared. The system must report the failure block but retain the text in the prompt editor to allow the user to modify and re-submit.

## 5. Non-Blocking Motion Invariant
* **Statement**: Animations never block input.
* **Invariant**: Spinners, typewriter queues, scroll transitions, and frame-rate updates are decoupled from the main input loop. Input polling and event processing must run immediately on the same tick, ensuring visual animations never increase input latency or drop keystrokes.

## 6. Keyboard Dominance Invariant
* **Statement**: Mouse is optional; keyboard is mandatory.
* **Invariant**: The TUI must be fully functional and all options selectable when running in mouse-disabled terminals. Every transition, selection, pane shift, scroll, and configuration switch must have an assigned keybinding.
