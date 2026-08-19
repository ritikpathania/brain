# Motion Specification

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`, strictly subordinate to and governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).


This document defines the behavior, timing, and visual specifications for motion and animations in the Brain TUI. Animations in the terminal must remain highly efficient, prevent visual flicker, and never block keyboard input.

---

## 1. Animation Specifications

| Animation | Character / Sequence | Tick Interval | Transition Duration | Behavior under Load |
| :--- | :--- | :--- | :--- | :--- |
| **Thinking Spinner** | `⠋`, `⠙`, `⠹`, `⠸`, `⠼`, `⠴`, `⠦`, `⠧`, `⠇`, `⠏` | 80 ms | N/A | Drops frames if CPU rendering lags. |
| **Typing Indicator** | `...` (pulsing text style) | 300 ms | N/A | Replaced instantly by text on first token. |
| **Typewriter Drain** | Varied (queued characters) | 10 ms | Capped at 150 ms lag | Speeds up drain if buffer queue depth grows. |
| **Progress Interpolation**| Block symbols (`█`) | 50 ms | 100 ms | Linear interpolation of percentages. |
| **Toast Fade Decay** | Color brightness transition | Frame-based | 3000 ms display, 100 ms fade | Instantly disappears on user scroll. |

---

## 2. Animation Behaviors

### 2.1. Typewriter Streaming Pacing
To prevent streaming text from popping in blocky chunks and causing eye strain:
1. **Queueing**: Incoming tokens from the Unix socket connection are pushed immediately to a client typewriter queue.
2. **Draining**: The rendering engine drains the queue at a target rate of 2-3 characters per frame (every 10ms frame tick).
3. **Flow Control**: If the network stream speeds up and the typewriter queue grows beyond 30 characters, the drain rate automatically increases proportionally to ensure total rendering lag never exceeds 150ms behind the network.

### 2.2. Toast Notification Lifetime
1. **Appearance**: The toast box appears instantly in the top-right corner.
2. **Display**: Stays fully active for `3000 ms`.
3. **Decay (Fade Out)**: Over the last `100 ms` before dismissal, the text style shifts down the color hierarchy (e.g. from `Text` -> `Muted` -> `Subtle`) across 3 consecutive frame updates before the toast is deleted.

### 2.3. Progress Bar Interpolation
* **Standard Progress Update**: When the daemon pushes progress percentage updates (e.g., during indexing or database migrations), the progress bar must not jump abruptly (e.g., from 10% to 50%).
* **Smooth Interpolation**: The client interpolates the visual fill blocks linearly over a `100 ms` window, running at a 50ms tick interval, preventing visual stutter.

### 2.4. Scroll Inertia and Anchoring
* **Inertial Scrolling**: When scrolling through long logs using `PageUp`/`PageDown`, the viewport shifts by half-page blocks instantly without inertial sliding.
* **Auto-Scroll Locking**: When the chat viewport is locked to the bottom during active streaming, its vertical scroll coordinate updates strictly on the frame tick that appends a typewriter character, maintaining visual alignment.
