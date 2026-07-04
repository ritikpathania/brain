# Component Library Specification

This document defines the interface, layout constraints, states, accessibility, and motion rules for all TUI components in the Brain design system.

---

## 1. StatusBar (Footer Status Line)
* **Purpose**: Displays quick helper labels, active shortcut keys, and real-time connectivity status.
* **Responsibilities**: Provides visual hints on context changes (e.g., indicating focus switching or streaming modes).
* **Inputs**: Connection status enum, focus region indicator.
* **Outputs**: None (presentation only).
* **States**: `Connected`, `Disconnected`, `Connecting`, `FocusEditor`, `FocusSidebar`.
* **Accessibility**: Screen readers read the status line as static text. No flashing characters allowed.
* **Motion**: Static text updates.
* **Keyboard Behavior**: Passive indicator; shifts styling when hotkeys are pressed.
* **Layout Constraints**: Height is locked to exactly `1` row at the absolute bottom of the terminal frame.
* **Design Invariants**: Must remain on screen at all times across all responsive breakpoints.

---

## 2. Sidebar (Navigation List)
* **Purpose**: Displays the active session list and chronological history items.
* **Responsibilities**: Enables users to switch between conversational memory caches.
* **Inputs**: Vector array of session metadata summaries, active selection index.
* **Outputs**: Dispatches loaded session requests on activation.
* **States**: `Focused` (Primary border highlight), `Unfocused` (Muted border), `Empty` (No sessions warning), `Populated`.
* **Accessibility**: List items are prefix-numbered (e.g. `[1]`, `[2]`) in screen reader mode for direct selection.
* **Motion**: Instantly scrolls list viewport to track selection index.
* **Keyboard Behavior**: `Up`/`Down` Arrow keys move selector; `Enter` activates the chosen session; `Backspace`/`Delete` archives or removes session.
* **Layout Constraints**: Locked to exactly `25` columns. Hidden in Compact mode (< 80 columns).
* **Design Invariants**: The active selection indicator must remain visible within the sidebar viewport boundaries.

---

## 3. Chat (Conversation Stream)
* **Purpose**: Displays the history of dialogue messages, reasoning plans, and tool logs.
* **Responsibilities**: Appends incoming text chunks smoothly, formatting code blocks and diff previews inline.
* **Inputs**: Ordered sequence of message items, current viewport vertical scroll offset, typewriter character buffer.
* **Outputs**: None.
* **States**: `Idle` (Scrollable), `Streaming` (Auto-scroll locked), `Loading` (Loading spinner indicator).
* **Accessibility**: Uses linear reading order. Auto-scrolling is disabled if user scrolls up.
* **Motion**: Drives character typewriter animation during token streaming.
* **Keyboard Behavior**: `PageUp`/`PageDown` scrolls half-page blocks.
* **Layout Constraints**: Minimum height of `10` rows. Fills all remaining space inside standard partitions.
* **Design Invariants**: Once text chunks are marked complete, they are strictly immutable.

---

## 4. Input (Prompt Editor)
* **Purpose**: Accepts user natural language inputs and slash commands.
* **Responsibilities**: Manages active edit strings, handles text wrap display, and anchors cursor.
* **Inputs**: Input text string buffer, cursor character index.
* **Outputs**: Emits submitted command/text on completion.
* **States**: `Focused` (Blinking cursor, Active border), `Unfocused` (Static cursor), `Locked` (Dimmed style, read-only mode during streaming).
* **Accessibility**: Exposed with explicit label `"Text input area"`.
* **Motion**: Blinking cell block cursor (`500 ms` cycle).
* **Keyboard Behavior**: Text typing, navigation via `Left`/`Right` arrows, word jump, delete, backspace. `Enter` submits query.
* **Layout Constraints**: Locked to exactly `3` rows (1 row text input, 2 rows borders).
* **Design Invariants**: Typing focus is protected. System updates cannot alter the input buffer contents.

---

## 5. Diff (Inline Code Comparison)
* **Purpose**: Displays code file modifications for developer review.
* **Responsibilities**: Shows line differences with explicit color highlights.
* **Inputs**: Original file content lines, proposed change lines.
* **Outputs**: None.
* **States**: `Active` (Expanded), `Collapsed` (Summary view).
* **Accessibility**: Diff lines must carry prefix indicators (`+` or `-`) inline. Color contrast must be high.
* **Motion**: Static draw.
* **Keyboard Behavior**: Keyboard scrolling overrides main screen scrolls when diff container is focused.
* **Layout Constraints**: Inline in chat. In Wide mode, expands to vertical split-screen panel.
* **Design Invariants**: Every colored line change must be accompanied by `+` or `-` characters to avoid color-only state mapping.

---

## 6. Progress (Status Bars)
* **Purpose**: Renders the completion metrics for daemon background syncs or file indexing.
* **Responsibilities**: Draws progress bars and fraction logs.
* **Inputs**: Completion percentage value (0.0 to 1.0).
* **Outputs**: None.
* **States**: `Running`, `Completed`.
* **Accessibility**: Screen reader mode prints `[Progress: X% Completed]`.
* **Motion**: Smooth block fill interpolation at 50ms intervals.
* **Layout Constraints**: Inline inside execution detail cards.
* **Design Invariants**: Numerical percentage text must always accompany the visual blocks.

---

## 7. Confirmation Dialog (Modal)
* **Purpose**: Warns and interrupts user flow before dangerous operations (e.g. folder trust, project purge).
* **Responsibilities**: Forces explicit selection and suspends main interface input loop.
* **Inputs**: Prompt warning message string.
* **Outputs**: Confirmed selection (boolean).
* **States**: `Active`, `Dismissed`.
* **Accessibility**: Overlays the screen, reading warning message. Blocks all other UI controls.
* **Motion**: Appears instantly in screen center.
* **Keyboard Behavior**: `y` / `n` keystrokes select; `Enter` confirms default option; `Esc` dismisses.
* **Layout Constraints**: Centered box (Height: 5, Width: 40).
* **Design Invariants**: Completely intercepts keyboard focus; cannot be dismissed without explicit keystroke.
