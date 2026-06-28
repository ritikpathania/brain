# Ratatui TUI Client Migration: Parity & Legacy Removal Audit Report

**Report Version**: 1.0.0
**Audit Date**: 2026-06-28
**Commit Hash**: 25b2e60 (or current main snapshot)

---

## 📊 1. Validation Scope Boundary
This audit validates the user interface presentation parity, styling fidelity, and local command execution lifecycle of the native Rust **Ratatui TUI client** (`brain-tui`) replacing the legacy TypeScript **React/Ink TUI client** (`cli/`).
* **In-Scope**: Redraw loops, multiline editors, scroll offsets, session selectors, key event mapping, resize responsiveness, typing latencies, typewriter pacing, and cancellation propagation.
* **Out-of-Scope**: Dowstream LLM accuracy, network transport socket boundaries, transactional backend database write-speeds, and external OS process reliability.

---

## 🔍 2. Feature Parity Checklist
Every feature from the legacy TypeScript React/Ink client was audited against the new native Ratatui interface.

| Feature Area | Legacy Behavior | Ratatui TUI Behavior | Status | Severity | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Scrolling Viewport** | Scroll history up/down | Bounded viewport offset calculations | **PASS** | None | Scrolling bounds handled by `ViewportState`. |
| **Multiline Editor** | Multi-line text input | Text buffer array manipulation | **PASS** | None | Handles multiline wrapping and cursor locations. |
| **Streaming Output** | Progressive text delivery | Time-paced `TypewriterQueue` buffer | **PASS** | None | Typewriter speeds pacing uses frame-independent clock. |
| **Immediate Cancel** | Cancel generation on ESC | Drops active Tokio cancel token | **PASS** | None | Triggers immediate cancel and flushes anim queues. |
| **Shortcuts Footer** | Inline key help line | Status footer row widget | **PASS** | None | Renders status line dynamically at screen bottom. |
| **Resizing Canvas** | Redraws on resize | Listens to SIGWINCH Crossterm events | **PASS** | None | Automatically fits components, hiding sidebar if width < 80. |
| **Markdown Rendering** | Basic text styles | Preformatted markdown styles | **PASS** | None | Correctly parses and highlights text and code blocks. |
| **Keyboard Navigation** | TAB focus switches | Cycles input focus between Editor and Sidebar | **PASS** | None | Tab routes keys; Up/Down arrow moves sidebar row focus. |
| **Sessions List** | List historic threads | Sidebar widget with active/inactive threads | **PASS** | None | Lazy loads selected thread details on Enter. |

---

## ⚡ 3. Behavioral Parity Checks
Beyond baseline feature checklist verification, operational timing and UX interactions were verified.

* **Typing Latency**: Measured in microsecond scales. Drawing standard crossterm keypress inputs occurs synchronously within the same tick frame, eliminating the React re-render queue overhead.
* **Scroll Anchoring**: The scroll locks to the bottom (tail follow) while streaming is active, and unlocks when manually scrolled up, matching standard chat interface expectations.
* **Typewriter Pacing**: Buffered tokens are drained via elapsed time deltas (`10ms` frame rate) avoiding uneven bursts.
* **Resize Repartitioning**: Responsive layout calculations hide the conversation sidebar when width < 80 cells, maximizing real-estate for reading threads on compact terminal sizes.

---

## 🔌 4. Legacy Workspace Audit
A dependency audit was conducted across the Rust workspaces to ensure zero active dependencies on React, Ink, Bun, or Node remain:

- **Ink/Yoga**: Zero occurrences in Rust workspace crate manifests. Ratatui uses a pure Rust cassowary layout engine.
- **Bun/Node Runtime**: Active execution processes rely solely on the compiled native `brain` binary.
- **Dependency Graph isolation**: Checked `Cargo.toml` and verified that no Rust libraries compile or reference compatibility shims or node wrappers.

---

## 🔮 5. Known Limitations
The following items were considered but are intentionally deferred:
1. **Mouse Scroll Events**: Scrolling using physical mouse wheel events is currently unhandled (focus remains on keyboard arrow inputs).
2. **Dynamic Custom Theme Loading**: Semantic styling is currently locked to `Theme::default_dark` styling rules; run-time config file themes are planned for post-release improvements.
