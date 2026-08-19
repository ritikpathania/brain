---
status: active
owner: tui
canonical: true
review_cycle: quarterly
last_reviewed: 2026-08-14
applies_to: v1.1+
---

# TUI Architecture & Stable UI Invariants

> **CANONICAL VISUAL CONTRACT**: Visual layout, typography, theme palettes, and component presentation are strictly governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](../design/CLAUDE_VISUAL_CONTRACT.md) and [`docs/design/CLAUDE_COMPONENT_MODEL.md`](../design/CLAUDE_COMPONENT_MODEL.md).
> This specification defines the **underlying Rust state reducer invariants, transport boundaries, and rendering loop purity**.

---

## 1. Architectural Boundaries

The TUI architecture enforces a clean separation between state management, networking/transports, and visual presentation:

```
┌────────────────────┐
│     UdsClient      │ <─── Network Transport (Pure I/O Adapter over UDS Stream)
└─────────┬──────────┘
          │ (Streams Monotonic StreamEvents)
          ▼
┌────────────────────┐
│      UiState       │ <─── Controller / Reducer State Machine (Unified State)
└─────────┬──────────┘
          │ (Immutable &UiState Snapshot)
          ▼
┌────────────────────┐
│    AppRenderer     │ <─── Presentation Layer (Stateless Differential Drawer)
└────────────────────┘
```

### Stateless Presentation Layer
- **Contract**: `AppRenderer::draw(&self, f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme)` accepts the application state as an immutable reference (`&UiState`).
- **Invariant**: The renderer is pure. It must never mutate any field on `UiState`, update scroll offsets, or trigger network queries. It only projects the precomputed view state onto character cells.
- **Layout Calculations**: Dynamic text wrapping, markdown visual span indexing, and viewport offset calculations must be computed in the state layer before rendering.

### Transport Adapter Isolation
- **Contract**: `UdsClient` is strictly a network transport adapter.
- **Invariant**: `UdsClient` does not maintain conversation history, session lists, sorting state, or user-interface variables. All session management resides inside the TUI application loop state.

---

## 2. Conversation & Viewport Models

The conversation history and scrolling behavior are governed by strict relational constraints.

### The Unified State Principle
- **Invariant**: `active_messages` is the single source of truth for the active session's message stream.
- **Session Boundary Sync**: When switching sessions or creating a new conversation, the current `active_messages` are archived into the `session_histories` map keying the active `SessionId`. When loaded, the history is restored fully into `active_messages`.

### Presentation Timeline (`TimelineBlock`)
- **Invariant**: `TimelineBlock` is a pure render-time model conforming to [`CLAUDE_COMPONENT_MODEL.md`](../design/CLAUDE_COMPONENT_MODEL.md). It does not carry domain rules or business facts.
- It is rebuilt dynamically on every event loop tick inside `state.rs::build_timeline_blocks` based on the wrapping width of the chat viewport.

### Viewport Scroll Semantics & Auto-Follow Behavior
- Scrolling uses two flags inside `ViewportState`: `scroll_offset` (index of first visible visual line) and `follow_tail` (boolean auto-scroll toggle).
- **Auto-Follow**: If the user is at the bottom of the timeline (`scroll_offset >= max_scroll`), `follow_tail` is set to `true`. While `follow_tail` is active, any incoming streamed token forces `scroll_offset = max_scroll`.
- **Manual Freeze**: If the user scrolls up (`Action::ScrollUp`), `follow_tail` is set to `false`. The scroll viewport remains fixed, letting the user read history while the stream progresses off-screen.
- **Resumption**: If the user scrolls back to the bottom (or if the offset naturally reaches the maximum), `follow_tail` is re-enabled, resuming auto-follow.

---

## 3. Focus Management Invariants

- Focus is managed via the `FocusRegion` enum: `Editor` (primary), `CommandPalette`, `SlashAutocomplete`, `SessionDrawer`, or `ModalDialog`.
- **Prompt-First Primacy**: Focus defaults to `FocusRegion::Editor`. Floating overlays capture focus only while active, and `Esc` immediately restores focus to the prompt composer without loss of draft text.
