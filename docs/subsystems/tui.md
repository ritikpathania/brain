---
status: active
owner: tui
canonical: false
review_cycle: quarterly
last_reviewed: 2026-08-14
applies_to: v1.1+
subsystem: tui
owns:
  - crates/brain-tui
depends_on:
  - protocol
  - sdk
used_by:
  - app
canonical_specs:
  - docs/design/CLAUDE_VISUAL_CONTRACT.md
  - docs/design/CLAUDE_COMPONENT_MODEL.md
  - docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md
adrs: []
rfcs:
  - RFC-008
  - RFC-009
---

# Terminal User Interface (TUI) Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical TUI visual and interaction contracts live in [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](../design/CLAUDE_VISUAL_CONTRACT.md) and component models live in [`docs/design/CLAUDE_COMPONENT_MODEL.md`](../design/CLAUDE_COMPONENT_MODEL.md).

---

## 1. Purpose
The TUI subsystem provides a rich, interactive, full-screen terminal experience built using Rust, `Ratatui`, and `Crossterm`. It faithfully reproduces Claude's UI/UX presentation and interaction grammar while projecting Brain's relational memory, hybrid search, and streaming capabilities.

## 2. Responsibilities
- Manages alternate screen buffer (`EnterAlternateScreen`) initialization and teardown.
- Implements immediate-mode differential rendering loops at 60 FPS (16.6ms frame budget).
- Buffers network stream chunks into a smooth typewriter queue.
- Surfaces interactive tool permission dialogs (`RFC-009`), slash command autocompletions, and command palettes (`Ctrl+K`).

## 3. Out of Scope
- IPC frame encoding or network socket listener management (owned by **Protocol**).
- Domain entity mutations or graph operations (owned by **Compiler** / **Domain**).
- Database disk storage (owned by **Storage**).

## 4. Architecture Overview
```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Scrollable Message Canvas (flexGrow: 1, borderless floor)                │
│    ├── Typographic Greeting Header                                          │
│    ├── User Query & Assistant Response Stream (Markdown, Code Fences)       │
│    ├── Inline Collapsible Thinking Blocks (⠋ Thinking 2.4s)                 │
│    ├── Inline Tool Execution Cards (✓ Read 42 lines)                        │
│    └── Recalled Memory Provenance Chips (⟡ Recalled 4 memories)             │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. Pinned Bottom Region (flexShrink: 0)                                     │
│    ├── Floating Overlays (Slash Autocomplete Popup / Command Palette Ctrl+K)│
│    ├── Prompt Input Composer (Boxed, rounded borders, multiline expansion)  │
│    └── Status Line (Single-row borderless hint bar at y = height - 1)       │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 5. Runtime Flow
1. **Startup**: TUI enters alt-screen mode, initializes Crossterm raw mode, connects to UDS socket.
2. **Event Loop**: Listens to terminal keyboard input and UDS `StreamEvent` frames.
3. **Typewriter Queue**: Network chunks enter a typewriter buffer, draining sequentially onto the Ratatui canvas at 60 FPS.

## 6. Key Invariants
- **Theme Token Encapsulation**: Components use semantic theme tokens mapped to Claude's warm neutral/terracotta palette, never raw ANSI escape sequences.
- **SIGWINCH Resizing**: Layouts dynamically adapt using two-pass layout calculation without negative rectangle panics.
- **Non-Blocking Render Loop**: Network I/O and state updates run asynchronously from the draw loop.

## 7. Owning Crates
- [`crates/brain-tui`](../../crates/brain-tui/README.md): Layout containers, widgets, theme structures, typewriter pipeline.

## 8. Implementation Notes
- Access theme structures via `Theme` maps defined in `crates/brain-tui/src/ui/theme/mod.rs`.

## 9. Canonical References
- [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](../design/CLAUDE_VISUAL_CONTRACT.md): **Sole Canonical Visual & Interaction Authority**.
- [`docs/design/CLAUDE_COMPONENT_MODEL.md`](../design/CLAUDE_COMPONENT_MODEL.md): **Canonical Component Architecture**.
- [`docs/design/BRAIN_CLAUDE_SURFACE_MAPPING.md`](../design/BRAIN_CLAUDE_SURFACE_MAPPING.md): **Canonical Capability Surface Mapping**.
- [`docs/architecture/STABLE_UI_INVARIANTS.md`](../architecture/STABLE_UI_INVARIANTS.md): Component state invariants.
- [`docs/design/README.md`](../design/README.md): Index of all TUI design specifications.

## 10. Related ADRs
- None directly assigned.

## 11. Related RFCs
- [`RFC-008: Immediate-Mode Ratatui Differential Rendering`](../architecture/rfc/RFC-008.md)
- [`RFC-009: Plan Mode & Interactive Terminal Review Modals`](../architecture/rfc/RFC-009.md)

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
├── Visualizes: Retrieval results & Memory graph provenance
└── Consumes: Theme Tokens (crates/brain-tui)
```
