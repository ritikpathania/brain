---
status: active
owner: tui
canonical: false
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
subsystem: tui
owns:
  - crates/brain-tui
depends_on:
  - protocol
  - sdk
used_by:
  - app
canonical_specs:
  - docs/design/TUI_DESIGN_SYSTEM.md
  - docs/design/THEME_TOKENS.md
adrs: []
rfcs:
  - RFC-008
  - RFC-009
---

# Terminal User Interface (TUI) Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical TUI layout rules live in [`docs/design/TUI_DESIGN_SYSTEM.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/TUI_DESIGN_SYSTEM.md) and theme tokens live in [`docs/design/THEME_TOKENS.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/THEME_TOKENS.md).

---

## 1. Purpose
The TUI subsystem provides a rich, interactive, full-screen terminal experience built using Rust, `Ratatui`, and `Crossterm`. It enables keyboard/mouse interactions, plan mode reviews, and real-time streaming displays inside standard terminal emulators.

## 2. Responsibilities
- Manages alternate screen buffer (alt-screen) initialization and teardown.
- Implements immediate-mode differential rendering loops.
- Buffers network stream chunks into a smooth typewriter queue.
- Surfaces interactive permission dialogs, plan mode reviews, and subagent peek panels.

## 3. Out of Scope
- IPC frame encoding or network socket listener management (owned by **Protocol**).
- Domain entity mutations or graph operations (owned by **Compiler**).
- Database disk storage (owned by **Storage**).

## 4. Architecture Overview
```text
┌─────────────────────────────────────────────────────────────────────────┐
│ Status Bar / Header (Connected | Active Session | Memory Load)           │
├────────────────────────────────┬────────────────────────────────────────┤
│ Sidebar Panel (Compact <80col) │ Chat Viewport / Timeline               │
│ - Sessions List                │ - Markdown Render Stream               │
│ - Tool Status Cards            │ - Search Result Cards                  │
├────────────────────────────────┴────────────────────────────────────────┤
│ Prompt Editor / Input Area (Multiline | History Nav | Status Hints)     │
└─────────────────────────────────────────────────────────────────────────┘
```

## 5. Runtime Flow
1. **Startup**: TUI enters alt-screen mode, initializes Crossterm raw mode, connects to UDS socket.
2. **Event Loop**: Listens to terminal keyboard/mouse input and UDS `StreamEvent` frames.
3. **Typewriter Queue**: Network chunks enter a typewriter buffer, draining sequentially onto the Ratatui canvas.

## 6. Key Invariants
- **Theme Token Encapsulation**: Components use semantic theme tokens, never raw ANSI escape sequences.
- **SIGWINCH Resizing**: Layouts dynamically adapt using percentage bounds and flexbox properties.
- **Non-Blocking Render Loop**: Network I/O and state updates run asynchronously from the 60 FPS draw loop.

## 7. Owning Crates
- [`crates/brain-tui`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-tui/README.md): Layout containers, widgets, theme structures, typewriter pipeline.

## 8. Implementation Notes
- Access theme structures via `Theme` maps defined in `crates/brain-tui/src/ui/theme/mod.rs`.

## 9. Canonical References
- [`docs/design/TUI_DESIGN_SYSTEM.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/TUI_DESIGN_SYSTEM.md): Canonical layout and viewport specifications.
- [`docs/design/THEME_TOKENS.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/THEME_TOKENS.md): Canonical theme token palette specifications.
- [`docs/architecture/STABLE_UI_INVARIANTS.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/STABLE_UI_INVARIANTS.md): Component state invariants.
- [`docs/design/README.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/design/README.md): Complete index of all 15 TUI design specifications.

## 10. Related ADRs
- None directly assigned.

## 11. Related RFCs
- [`RFC-008: Immediate-Mode Ratatui Differential Rendering`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-008.md)
- [`RFC-009: Plan Mode & Interactive Terminal Review Modals`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-009.md)

## 12. Operations
- Target draw frame latency: $< 16\text{ ms}$ (60 FPS).

## 13. Testing
- Integration tests in `crates/brain-tui/tests/` verify state reducers and layout computations.

## 14. Extension Points
- Implement custom Ratatui widgets inside `crates/brain-tui/src/ui/components/`.

## 15. Subsystem Dependencies
```text
TUI Subsystem
├── Depends on: Protocol (brain-integrations) & Client SDK (brain-sdk-rs)
├── Communicates with: Background Daemon (daemon) over UDS
├── Visualizes: Retrieval results & Compiler plans
└── Consumes: Theme Tokens (crates/brain-tui)
```
