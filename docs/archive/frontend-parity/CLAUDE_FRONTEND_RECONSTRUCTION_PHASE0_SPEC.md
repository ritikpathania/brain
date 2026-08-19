# Specification — Claude Code Frontend Reconstruction (Phase 0)

> **Document Status**: Approved Phase 0 Architecture Specification  
> **Target Subsystem**: `crates/brain-tui`, Presentation Layer, Layout Engine, Mock Fixture Engine  
> **Core Strategy**: **Claude First, Brain After** (Full Claude Frontend Reconstruction before Brainification)  
> **Authoritative Baseline Reference**: [`docs/design/FRONTEND_RECONSTRUCTION_AND_MIGRATION_RFC.md`](FRONTEND_RECONSTRUCTION_AND_MIGRATION_RFC.md)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

```text
PHASE 0 STRATEGY — CLAUDE FRONTEND RECONSTRUCTION
ORDER: Claude Frontend → Visual/Behavioral Parity → Brain Adapter → Brain Semantics → Feature Pruning
PRIORITY: MAKE THE FRONTEND CLAUDE FIRST. BRAIN COMES AFTER.
```

---

## 1. Executive Strategy & Core Principle

This specification governs **Phase 0: Full Claude Frontend Reconstruction**.

### The Core Principle
> Reconstruct Brain's frontend to match Claude Code's observable frontend/UIUX as completely as possible FIRST, using mocked/fake data where necessary. Only AFTER the Claude frontend is reconstructed should we adapt it to Brain semantics and remove unsupported Claude-specific features.

### Order of Execution:
1. **Claude Frontend Reconstruction** (Phase 0): Reproduce all 26 observable Claude frontend surfaces, interaction behaviors, and single-source layout geometry using deterministic mock fixtures.
2. **Visual & Behavioral Parity Verification**: Mechanically audit the standalone reconstructed frontend against the Claude source oracle.
3. **Brain FrontendAdapter**: Connect the presentation layer to Brain's UDS event stream and domain runtime state via a clean `FrontendAdapter` boundary.
4. **Brain Semantics Integration**: Adapt presentation items to Brain's relational memory graph, vector/BM25 retrieval, and tool engine.
5. **Feature Pruning**: Remove or map Claude-specific placeholder elements (`/model`, `/effort`, billing counters) that are non-applicable to Brain.

---

## 2. Complete Inventory of Reconstructed Claude Frontend Surfaces (26 Surfaces)

The Phase 0 reconstruction reproduces all 26 observable surfaces and components of Claude Code:

| Surface ID | Component Name | Visual Description & Contract | Phase 0 Strategy |
| :--- | :--- | :--- | :--- |
| **SURF-01** | **Top Header Bar** | Fixed 1-row top header displaying session title, model badge, and connection status. | Reproduce with mock state |
| **SURF-02** | **Sticky Prompt Header** | 1-row header `❯ <collapsed_prompt>` pinned to top row when prompt scrolls above viewport. | Reproduce (120-char truncation) |
| **SURF-03** | **User Query Block** | Left-aligned `❯ <user_prompt>` with distinct text styling and vertical margin. | Reproduce |
| **SURF-04** | **Assistant Text Response** | Markdown-rendered text with bold headers, lists, code fences, and syntax highlighting. | Reproduce |
| **SURF-05** | **Thinking Block Header** | `Thinking... (duration)` header with status symbol `⏺` / `✔` and duration timer. | Reproduce |
| **SURF-06** | **Thinking Collapsible Drawer** | Indented collapsible container displaying internal reasoning trace. | Reproduce (`Ctrl+O` toggle) |
| **SURF-07** | **Tool Execution Card (Pending)** | Tool call card in `PendingApproval` state with user action prompt. | Reproduce |
| **SURF-08** | **Tool Execution Card (Running)** | Tool call card in `Running` state with active status symbol `⏺` and timer. | Reproduce |
| **SURF-09** | **Tool Execution Card (Success)** | Tool call card in `Completed` state with success symbol `✔` and summary. | Reproduce |
| **SURF-10** | **Tool Execution Card (Failed)** | Tool call card in `Failed` state with error symbol `✖` and failure message. | Reproduce |
| **SURF-11** | **Tool Execution Card (Denied)** | Tool call card in `Denied` state with cancellation indicator. | Reproduce |
| **SURF-12** | **Tool Output Collapsible Drawer**| Indented tool output container with 20-line drawer cap and line numbers. | Reproduce (`Ctrl+O` toggle) |
| **SURF-13** | **Typewriter Streaming Buffer** | Monotonic token rendering buffer delivering smooth text insertion. | Reproduce |
| **SURF-14** | **Floating New Messages Pill** | Bottom-row overlay `↓ N new messages` shown when scrolled away from tail. | Reproduce |
| **SURF-15** | **Multiline Prompt Editor** | Dynamic height editor with hard-newlines, soft-wrapping, and visual cursor. | Reproduce |
| **SURF-16** | **Prompt Visual Cursor** | Block/bar cursor supporting visual-line Up/Down, Home/End, Ctrl+A/E/K/Y. | Reproduce |
| **SURF-17** | **Prompt History Escalation** | Escalates Up/Down arrow keypresses to prompt history at visual boundaries. | Reproduce |
| **SURF-18** | **Command Palette Overlay** | `Ctrl+K` modal overlay with fuzzy matching, ranking, and category badges. | Reproduce |
| **SURF-19** | **Slash Completion Menu** | Popup list triggered by `/` in prompt, offering auto-completion hints. | Reproduce |
| **SURF-20** | **Shortcuts Help Overlay** | `?` / `F1` modal dialog displaying complete keybinding reference matrix. | Reproduce |
| **SURF-21** | **Status / Token Footer Bar** | Bottom row status bar showing active model, cost/token metrics, and hints. | Reproduce |
| **SURF-22** | **Empty Landing Screen** | Welcome screen with brand logo, command hints, and recent sessions. | Reproduce |
| **SURF-23** | **Loading / Connection State** | Animated loading spinner and `Connecting...` banner indicators. | Reproduce |
| **SURF-24** | **Error / Alert Banners** | High-contrast error banners with diagnostic details and retry hints. | Reproduce |
| **SURF-25** | **Claude `/model` & `/effort` Controls** | Visual controls for selecting AI model and reasoning effort tier. | Reproduce in Phase 0 Mock |
| **SURF-26** | **Claude Cost & Token Counters** | Real-time session token consumption and estimated API cost indicators. | Reproduce in Phase 0 Mock |

---

## 3. Presentation Boundary & Architecture

To prevent presentation geometry, scrolling offsets, and layout measurement from coupling with backend product state, the architecture enforces a strict presentation boundary:

```text
┌────────────────────────────────────────────────────────┐
│             Brain Runtime Engine / Daemon              │
└───────────────────────────┬────────────────────────────┘
                            │ UDS StreamEvent Protocol
                            ▼
┌────────────────────────────────────────────────────────┐
│                Brain FrontendAdapter                   │
│  (Translates StreamEvents / Domain into Presentation)  │
└───────────────────────────┬────────────────────────────┘
                            │ PresentationState Snapshot
                            ▼
┌────────────────────────────────────────────────────────┐
│             Presentation Components / Views            │
│  (Header, Sticky Prompt, Timeline, Pill, Editor)       │
└───────────────────────────┬────────────────────────────┘
                            │ Intrinsic Constraints
                            ▼
┌────────────────────────────────────────────────────────┐
│              Single Authoritative Layout Engine        │
│  (Pass 1 Intrinsic Measurement → Pass 2 Allocation)    │
└───────────────────────────┬────────────────────────────┘
                            │ Partitioned Viewport Rects
                            ▼
┌────────────────────────────────────────────────────────┐
│             Ratatui / Crossterm Renderer               │
└────────────────────────────────────────────────────────┘
```

### Architectural Guardrails:
1. **Zero Direct Subsystem Dependencies**: UI view components must **never** import `brain-storage`, SQLite, domain repositories, `ApplicationRuntime`, or UDS socket connections directly.
2. **Pure State Consumption**: All visual elements are driven by `PresentationState` data structures.
3. **Standalone Execution Capability**: The presentation layer must be executable in standalone mode using mock fixtures without requiring a running background daemon.

---

## 4. Single Authoritative Layout Engine Model

All layout geometry, wrapping width, viewport height, scroll bounds, cursor position, sticky header placement, and overlay portaling are computed by **ONE** single layout engine (`LayoutEngine`).

```rust
/// Single authoritative layout solver for the presentation layer.
pub struct LayoutEngine {
    viewport: Rect,
}

impl LayoutEngine {
    /// Constructs a layout engine for a given terminal viewport.
    pub fn new(viewport: Rect) -> Self;

    /// Computes two-pass layout allocation.
    /// Pass 1: Measure intrinsic prompt height & overlay constraints.
    /// Pass 2: Allocate non-overlapping Rects for Header, Sticky Header, Chat Viewport, New Messages Pill, and Prompt Editor.
    pub fn compute_layout(&self, state: &PresentationState) -> LayoutAllocation;
}

/// Consolidated layout geometry returned by Pass 2 allocation.
pub struct LayoutAllocation {
    pub header_area: Rect,
    pub sticky_header_area: Option<Rect>,
    pub chat_viewport_area: Rect,
    pub new_messages_pill_area: Option<Rect>,
    pub prompt_editor_area: Rect,
    pub footer_area: Rect,
    pub overlay_area: Option<Rect>,
}
```

---

## 5. The 25 Deterministic Mock Data Fixtures

To validate all 26 reconstructed surfaces independently of the backend daemon, Phase 0 establishes **25 deterministic mock data fixtures**:

| Fixture ID | Name | Scenario Covered |
| :--- | :--- | :--- |
| **FIX-01** | `empty_landing` | Initial welcome screen with brand logo, command hints, and session list. |
| **FIX-02** | `single_user_query` | Simple 1-line user prompt in conversation timeline. |
| **FIX-03** | `short_assistant_response` | Single-paragraph assistant response with basic text. |
| **FIX-04** | `long_assistant_response` | Multi-paragraph assistant response with rich Markdown formatting. |
| **FIX-05** | `multiline_user_prompt` | 5-line user prompt with hard newlines and soft wrapping. |
| **FIX-06** | `active_streaming_response` | Mid-stream typewriter response showing active cursor. |
| **FIX-07** | `thinking_block_active` | Live `Thinking... (4.2s)` block with spinner symbol `⏺`. |
| **FIX-08** | `thinking_block_expanded` | Expanded thinking drawer showing internal reasoning text. |
| **FIX-09** | `tool_execution_pending` | Tool card in `PendingApproval` state requesting user permission. |
| **FIX-10** | `tool_execution_running` | Tool card in `Running` state with duration timer. |
| **FIX-11** | `tool_execution_completed` | Tool card in `Completed` state with success symbol `✔`. |
| **FIX-12** | `tool_execution_failed` | Tool card in `Failed` state with error traceback drawer. |
| **FIX-13** | `tool_drawer_expanded` | Indented tool output drawer capped at 20 lines with line numbers. |
| **FIX-14** | `scrolled_above_tail` | Conversation view scrolled 50 lines above stream tail. |
| **FIX-15** | `new_messages_pill_visible` | Scrolled-away view showing `↓ 3 new messages` pill. |
| **FIX-16** | `sticky_prompt_header_active` | Scrolled-above view showing top 1-row `❯ <collapsed_prompt>` header. |
| **FIX-17** | `command_palette_open` | `Ctrl+K` modal overlay active with fuzzy-ranked command list. |
| **FIX-18** | `slash_completion_open` | `/` popup completion menu active in prompt editor. |
| **FIX-19** | `shortcuts_help_open` | `?` / `F1` keybinding reference modal overlay active. |
| **FIX-20** | `viewport_narrow_69x24` | Compact 69x24 terminal viewport testing layout collapse. |
| **FIX-21** | `viewport_wide_182x53` | Large 182x53 terminal viewport testing full-width rendering. |
| **FIX-22** | `viewport_small_height_12` | Restricted 12-row terminal height testing minimum bounds. |
| **FIX-23** | `combined_thinking_tool_stream` | Complex item containing thinking block, tool card, and response. |
| **FIX-24** | `loading_connecting_state` | `Connecting to daemon...` spinner status state. |
| **FIX-25** | `error_banner_active` | High-contrast error banner showing transport failure details. |

---

## 6. Interaction Behavior Contracts

1. **Multiline Prompt Visual Cursor Movement**:
   - Up/Down arrow keys move the cursor visually line-by-line within multiline text buffers.
   - History escalation occurs **only** when Up is pressed on visual line 0 or Down is pressed on the final visual line.
2. **Key Routing & Target Resolution**:
   - `Ctrl+O` / `Alt+T` routing priority hierarchy:
     `Active Overlay → Active Thinking Block → Active Tool Card → Fallback`.
3. **Scroll Anchoring & Viewport Policy**:
   - When pinned to stream tail (`follow_tail == true`), incoming tokens auto-scroll the viewport.
   - When scrolled away (`follow_tail == false`), `ScrollAnchor` maintains exact user reading position during drawer expansions or stream insertions.
4. **Sticky Header & Pill Isolation**:
   - Sticky Prompt Header sits at top row `y = chat_area.y`.
   - New Messages Pill sits at bottom row `y = chat_area.y + height - 1`.
   - Zero spatial collisions or layout overlap (`VERIFIED`).

---

## 7. Next Phase Readiness

Upon completion of Phase 0:
- The standalone presentation layer renders all 26 Claude frontend surfaces cleanly via 25 mock fixtures.
- The single `LayoutEngine` guarantees zero visual flicker or resize layout panics.
- The project is ready for **Brainification** (Phase 1): mapping real UDS stream events into `PresentationState` via the `FrontendAdapter`, followed by pruning non-applicable Claude-specific features.
