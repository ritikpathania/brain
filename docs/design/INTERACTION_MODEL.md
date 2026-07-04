# Interaction Model

This document defines the lifecycle states, state transitions, and the presentation behaviors of the Brain TUI client. Every user interaction or background network event maps to one of these states.

---

## 1. Lifecycle States Matrix

| State | Input Prompt | Chat Viewport | Footer / Status | Cursor Behavior | Animations |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Idle** | Enabled & Focused | Scrollable | Shows general help & current connection | Blinking in editor | Static |
| **Typing** | Active Edit | Scrollable | Shows active edit shortcuts | Follows text insertion | Static |
| **Planning** | Locked & Dimmed | Locked | "Planning..." & request ID | Hidden | Pulse / Planning Spinner |
| **Running** | Locked & Dimmed | Locked | "Executing..." | Hidden | Step-level Spinners |
| **Tool Execution**| Locked & Dimmed | Locked | Tool name, parameters, runtime stats | Hidden | Activity Spinner |
| **Streaming** | Locked & Dimmed | Auto-scroll to bottom | "Streaming..." \| "Ctrl+C to Cancel" | Anchored at bottom of text | Typewriter buffer drain |
| **Completed** | Enabled & Focused | Scrollable | "Completed" \| General shortcuts | Blinking in editor | Static |
| **Follow-up** | Enabled & Focused | Scrollable | "Up/Down: Recall History" | Blinking in editor | Static |

---

## 2. Detailed State Behaviors

### 2.1. Idle State
* **Trigger**: Client initialization or completion of previous stream without further input.
* **UX Specification**:
  * **Footer**: Renders `" Tab: Switch Focus | Esc: Exit | Enter: Submit "`.
  * **Status**: Connection indicator active (e.g., `[Connected: Daemon]`).
  * **Cursor**: Blinks at position `0` or end of text in the prompt bar.

### 2.2. Typing State
* **Trigger**: Character input, backspace, or navigation keys pressed while prompt editor is focused.
* **UX Specification**:
  * **Input Editor**: Active text rendering, cursor moves horizontally corresponding to the string array buffer changes.
  * **Footer**: Displays contextual typing instructions (e.g., cursor reposition instructions).

### 2.3. Planning State
* **Trigger**: User presses `Enter` in the typing state.
* **UX Specification**:
  * **Input Editor**: Becomes read-only; text color switches to the `Muted` style token.
  * **Footer**: Changes to `" Esc: Cancel planning "`.
  * **Status**: Displays `"Awaiting plan from engine..."` with a progress spinner.

### 2.4. Running & Tool Execution State
* **Trigger**: Daemon sends the multi-step plan payload; begins running actions.
* **UX Specification**:
  * **Chat Panel**: Renders the checklist timeline. Completed items are prefixed with checkmarks (`✓` in success style); active tool items display a spinning character sequence.
  * **Interactive Dialogs**: If a step requires permission (e.g., code edit or shell execution approval), the prompt locks and pops up a modal window containing a `[y/N]` option. Typing in the main prompt is disabled until the dialog is closed.

### 2.5. Streaming State
* **Trigger**: Token chunks start arriving from the daemon's generator.
* **UX Specification**:
  * **Chat Panel**: Subscribes to the typewriter rendering queue. Tokens are appended smoothly to the active assistant box.
  * **Scroll Anchoring**: Viewport is forced to stick to the bottom limit (`scroll_offset == max`) to display text as it builds. If the user scrolls up manually during streaming, auto-scroll is temporarily suspended.
  * **Footer**: Shows `" Ctrl+C: Cancel streaming "`.

### 2.6. Completed & Follow-up State
* **Trigger**: Streaming finished (end-of-stream event) or cancelled.
* **UX Specification**:
  * **Input Editor**: Prompt unlocks; color restores to `Text` style; prompt is cleared.
  * **History Navigation**: User can press `Up` or `Down` arrow keys to recall previous inputs sequentially into the prompt editor.
