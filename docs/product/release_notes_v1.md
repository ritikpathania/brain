# Release Notes — BRAIN v2 TUI (Version 1.0.0)

We are excited to announce the Release Candidate for **Version 1.0.0** of the Standalone Relational Memory Engine Terminal User Interface (`brain-v2`). This release focuses on transitioning the TUI from a single-turn search client into a complete, professional multi-turn conversational interface.

---

## Key Capabilities

### 💬 Multi-Turn Conversations
- **Persistent Timeline**: The chat history is now fully persistent. Subsequent queries append to the existing conversation instead of clearing the screen, allowing you to ask follow-up questions and see prior turns inline.
- **Session Selector & History Switching**: Creating multiple sessions (`Ctrl+N`) starts fresh, independent threads. Switching sessions in the sidebar archives active messages and restores the respective history seamlessly.

---

## User Experience (UX) Improvements

### 🖱️ Viewport-Relative Scrolling
- Navigation commands (`PageUp` / `PageDown` and `Ctrl+U` / `Ctrl+D`) now compute step sizes dynamically based on your current terminal height (scrolling by full-page and half-page increments respectively).
- Fixed-step mouse scrolling and single-line keyboard scroll key combinations (`Ctrl+Up` / `Ctrl+Down`) remain active for precise reading.

### 🛡️ Smart Scroll Pinning (Auto-Follow)
- When viewing a streaming query response, the viewport automatically follows the tail (auto-scroll).
- If you manually scroll up to read history, auto-scroll is pinned/frozen so you are not yanked back to the bottom. Scrolling back to the tail automatically re-engages follow-tail mode.

### ⚡ Connection Status & Recovery
- The top header bar provides real-time feedback on connection modes (`[Connecting...]`, `[Connected: Daemon]`, and `[Disconnected]`).
- If connection to the background daemon is lost, you can restart the daemon and simply press `Enter` to retry connection immediately from the prompt editor without needing to type dummy query text.

### 🧹 Clean Result Formatting
- Stripped empty JSON curly braces `{}` and redundant type badges from search previews, giving candidate memory listings a clean, polished appearance.

### 📱 Responsive Layouts & Focus cycling
- Focus correctly cycles between panels on large displays. 
- On small displays (width < 80 columns), the sidebar is cleanly hidden and focus is automatically restricted to the prompt editor to prevent keystrokes from leaking into invisible widgets.

---

## Known Limitations

- **Command Palette & Slash Completions**: Visual overlays for slash commands and the command palette are temporarily deferred to a future release as keyboard entry points have not yet been wired.
