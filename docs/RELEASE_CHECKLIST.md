# Release Checklist — BRAIN v2 TUI

This document lists the official quality checks and acceptance criteria required for releasing versions of the BRAIN v2 Terminal User Interface (TUI). Every release candidate must pass these checks under manual and automated validation before delivery.

---

## 1. Core Daemon & Connection Status
- [ ] **Daemon Lifecycle**: Starting `./brain daemon start` creates a socket at `~/.brain/daemon.sock` and stops cleanly with `./brain daemon stop`.
- [ ] **Startup Connection Mode**: Launching the TUI (`brain-v2`) with the daemon stopped starts the TUI in `[Disconnected]` mode without crashing or hanging.
- [ ] **Dynamic Reconnection**:
  - Start the TUI with the daemon offline.
  - Start the daemon in the background.
  - Press `Enter` in the prompt editor (with empty prompt).
  - Verify the TUI connects successfully and transitions the status bar and header to `[Connected: Daemon]`.
- [ ] **Connection Loss Notification**: Stopping the daemon while TUI is running immediately transitions the status header to `[Disconnected]` and status bar to `⚠ Connection lost — press Enter to retry`.

---

## 2. Conversation & Multi-Turn Experience
- [ ] **Chronological Timeline Persistence**: Submitting multiple queries in the active session appends the user prompt and the assistant's streaming markdown response to the timeline, keeping prior turns visible on screen.
- [ ] **Timeline Content Rendering**: Markdown formatting (headers, lists, tables, code blocks) is rendered correctly with appropriate syntax highlighting colors.
- [ ] **No Stranded Placeholders**: Cancelling or stopping a stream immediately (e.g. before any tokens are produced) does not leave an empty/orphaned placeholder in the timeline or offset subsequent turns.
- [ ] **Search Result Candidate Formatting**: Hybrid/vector search previews appear without empty JSON braces `{}` or redundant type markers.

---

## 3. Viewport Scrolling & Focus
- [ ] **Dynamic Scroll Increments**:
  - `PageUp` / `PageDown` scrolls the chat viewport by a full page (computed dynamically based on active terminal height).
  - `Ctrl+U` / `Ctrl+D` scrolls the chat viewport by exactly half a page.
  - Mouse Wheel / `Ctrl+Up` / `Ctrl+Down` scrolls by a small step (3 lines).
- [ ] **Viewport Boundaries**: Scrolling up or down stops hard at the timeline boundaries (no out-of-bounds rendering or visual glitching).
- [ ] **Auto-Follow / Tail Semantics**:
  - Viewport follows the end of the streaming response by default.
  - Manually scrolling up freezes/pins the scroll position, allowing the user to read history while streaming continues.
  - Scrolling back down to the bottom automatically re-engages the follow-tail auto-scrolling.
- [ ] **Focus Management**:
  - Focus correctly cycles between the sidebar and the editor on wide displays using `Tab`.
  - Resizing below 80 columns hides the sidebar and dynamically redirects focus to the prompt editor.
  - Toggling focus (`Tab` / `Esc`) on compact displays (< 80 columns) is safely trapped and does not leak keyboard input to hidden components.

---

## 4. Session Persistence & Management
- [ ] **Ctrl+N Flow**: Pressing `Ctrl+N` starts a fresh conversation thread, clears the screen, resets viewport offsets, and shifts input focus to the prompt editor.
- [ ] **Session Switching**: Switching sessions in the sidebar and pressing `Enter` correctly loads the historical messages from `session_histories` or requests them asynchronously from the daemon.
- [ ] **Session Boundary Sync**: Deactivating a session correctly commits the active session's message list into `session_histories` so switching back restores all turns accurately.
- [ ] **Visual Distinction**: The active session in the sidebar list is highlighted with bold primary styling even when the cursor is focused elsewhere.

---

## 5. Performance Invariants
- [ ] **Rendering Latency**: Draw frame time remains below 66 ms even under virtualized rendering with heavy markdown streams.
- [ ] **Graceful Cancellation**: Pressing `Esc` or `Ctrl+C` during an active query terminates the daemon execution channel immediately and flushes the typewriter queue without delay.
- [ ] **Memory Overhead**: RSS usage of the TUI process remains stable during session switching and long conversations.
