# RFC — Brain Frontend Reconstruction & Migration Architecture

> **Document Status**: Approved Migration RFC & Architectural Specification  
> **Target Subsystems**: `crates/brain-tui`, `apps/brain`, Frontend Presentation Layer, Adapter Boundary  
> **Authoritative Baseline Reference**: [`docs/design/REPOSITORY_PRODUCTION_READINESS_AUDIT.md`](REPOSITORY_PRODUCTION_READINESS_AUDIT.md)  
> **Legacy Status**: `crates/brain-tui` classified as `LEGACY / REFERENCE IMPLEMENTATION` (Frozen Baseline)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Goal

Establish an architectural blueprint for reconstructing Brain's user interface layer while decoupling presentation geometry, layout calculation, and interaction state from Brain's core runtime domain model.

### Core Architectural Objectives:
1. **Decouple Presentation from Runtime State**: Move from tight coupling (`Brain State → Layout/Viewport/Scroll/Renderer/Semantics`) to a clean three-layer architecture (`Brain Runtime → Presentation Adapter → Frontend Layout Engine → Rendered UI`).
2. **Formalize the Adapter Boundary**: Define a strict, isolated frontend adapter boundary separating Brain's relational memory/retrieval engine from the UI rendering layer.
3. **Preserve Claude Parity Language**: Retain Claude Code's visual and interaction language (2-pass layout, sticky header, thinking blocks, tool execution cards, new messages pill, multiline prompt cursor) while powering it with Brain's backend products.
4. **Freeze `crates/brain-tui` as Reference Implementation**: Preserve existing `crates/brain-tui` as a frozen reference codebase containing working UDS transport integrations, view models, and 100 passing test suites.

---

## 2. Non-Goals

- **NOT a Claude Code Clone**: Brain will **NOT** reproduce Claude-specific settings, billing models, `/effort` settings, `/model` switches, or Anthropic-specific infrastructure tools.
- **NO Premature Frontend Rewrites**: Will **NOT** delete or rewrite `crates/brain-tui` before the frontend adapter API and replacement architecture are fully specified, tested, and validated.
- **NO Backend Runtime Changes**: Will **NOT** alter `brain-domain`, `brain-services`, `brain-storage`, `brain-core`, or UDS protocol event schemas. The backend runtime remains untouched.

---

## 3. The Three-Layer Contract System

To prevent presentation geometry, scrolling offsets, and layout measurement from coupling with backend product state, the frontend contract is partitioned into three decoupled layers:

```text
┌────────────────────────────────────────────────────────┐
│ Layer A: Visual Contract (Geometry, Spacing, Tokens)   │
├────────────────────────────────────────────────────────┤
│ Layer B: Interaction Contract (Keyboard, Scroll, Edit) │
├────────────────────────────────────────────────────────┤
│ Layer C: Brain Semantic Contract (Domain & Runtime)    │
└────────────────────────────────────────────────────────┘
```

### A. Layer A — Visual Contract (Presentation & Geometry)
Defines static visual boundaries, spacing, and styling independent of data origin:
- **Geometry & Spacing**: Canonical viewport bounds (80x24, 120x40, 182x53), 1-row fixed header height, 1-row status bar, dynamic prompt editor height (Pass 1 measurement).
- **Typography & Styling**: Muted secondary text, bold headers, HSL-derived color tokens, `wrap="truncate-end"` for headers, 20-line drawer caps for tool execution output.
- **Surface Layout**: Fixed top sticky prompt header slot, scrollable message viewport, floating bottom new-message pill, anchored prompt editor, portaled command palette/overlays.

### B. Layer B — Interaction Contract (User Events & Input)
Defines user input handling, cursor navigation, and scroll state transitions:
- **Prompt Input & Multiline Cursor**: Soft-wrapping, hard-newline insertion, visual-line Up/Down navigation, Home/End, Ctrl+A/E, Ctrl+K (kill line), Ctrl+Y (yank), image token navigation.
- **History Escalation**: Up/Down arrow navigation escalates to prompt history **only** when the cursor resides on visual line boundaries.
- **Key Routing & Target Resolution**: Priority resolution hierarchy (`Overlay → Active Thinking Block → Active Tool Card → Fallback`). `Ctrl+O` / `Alt+T` toggles expansion without focus collisions.
- **Scroll Anchoring & Viewport Policy**: Follow-tail mode when pinned to stream end; `ScrollAnchor` reading position retention when scrolled away. Height deltas during card expansion/collapse do not cause scroll drift.

### C. Layer C — Brain Semantic Contract (Domain & Products)
Defines Brain's domain models and application capabilities:
- **Query & Knowledge Pipeline**: Graph query evaluation, sentence ingestion, vector/BM25 hybrid retrieval, RRF fusion, context node assembly.
- **Session & Memory**: Knowledge graph pinning, session archiving, temporal state records, confidence scoring.
- **Streaming Protocol & UDS**: Monotonic tagged stream events (`stream_start`, `stream_progress`, `stream_chunk`, `stream_end`, `stream_cancelled`).

---

## 4. Proposed Frontend Architecture

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                           Brain Runtime Engine                          │
│     (Domain, Services, Storage, Retrieval, Sessions, Tools, UDS)        │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ Monotonic Tagged Stream Events
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Brain Frontend Adapter                          │
│         (Maps Domain Events & State into Pure Presentation State)        │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ Pure Presentation State
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Frontend Layout Engine                           │
│        (Pass 1 Intrinsic Measurement → Pass 2 Geometry Allocation)       │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ Partitioned Viewport Geometry
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           Rendered UI Surface                           │
│             (Header, Sticky Prompt, Timeline, Pill, Editor, Footer)     │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Brain $\leftrightarrow$ Frontend Adapter API

The Frontend Adapter acts as the sole translation boundary between Brain Runtime events and UI presentation state:

```rust
/// Pure presentation state snapshot consumed by the Frontend Layout Engine.
pub struct PresentationState {
    pub session_id: String,
    pub session_title: String,
    pub timeline_items: Vec<PresentationTimelineItem>,
    pub sticky_header: Option<StickyHeaderPresentation>,
    pub scroll_pill: Option<ScrollPillPresentation>,
    pub prompt_buffer: String,
    pub prompt_cursor: CursorPosition,
    pub active_overlay: OverlayPresentation,
    pub connection_status: ConnectionStatusPresentation,
}

/// Abstract Frontend Adapter trait decoupled from rendering engine.
pub trait FrontendAdapter: Send + Sync {
    /// Ingests a backend UDS StreamEvent and produces an updated PresentationState.
    fn handle_stream_event(&mut self, event: brain_core::events::StreamEvent) -> PresentationState;
    /// Ingests a user keyboard or terminal event and returns a dispatchable Action.
    fn handle_user_input(&mut self, input: UserInputEvent) -> Option<BrainAction>;
}
```

---

## 6. Claude $\rightarrow$ Brain Feature Mapping

| Claude Surface / Command | Brain Action | Justification |
| :--- | :--- | :--- |
| **Sticky Prompt Header** | **Reproduce** | Essential for scrollback context awareness when viewing deep history. |
| **Two-Pass Layout Engine** | **Reproduce** | Eliminates prompt/chat height oscillation and visual flickering. |
| **Thinking Block Widget** | **Reproduce Presentation** | Maps to Brain's reflection, reasoning, and plan generation steps. |
| **Tool Execution Cards** | **Reproduce Presentation** | Maps to Brain's tool execution engine and approval workflows. |
| **New Messages Pill** | **Reproduce Behavior** | Provides frictionless scroll-to-bottom navigation during streaming. |
| **Multiline Prompt Cursor** | **Reproduce Navigation** | Provides Claude-parity multiline text editing and visual line movement. |
| **Command Palette (`Ctrl+K`)** | **Reproduce Architecture** | Powers Brain commands (`/query`, `/ingest`, `/config`, `/status`, `/help`). |
| `/model` Command | **Remove** | Brain uses local daemon configuration and provider routing. |
| `/effort` Command | **Remove** | Claude-specific parameter; non-applicable to Brain engine. |
| Claude Billing / Tokens | **Remove** | Brain operates local relational memory; non-applicable. |

---

## 7. Technology Evaluation Matrix

| Criterion | Native Rust / Ratatui (Current) | Ink / React / Yoga (Claude Stack) | Custom Web / Electron / Tauri |
| :--- | :--- | :--- | :--- |
| **ADR-001 Compliance** | 100% (Native Rust binary) | Requires Node/Bun runtime | Requires Chromium/Browser runtime |
| **Performance / Footprint** | Extremely low memory (< 15MB) | Medium memory (50-100MB) | High memory (> 200MB) |
| **Layout Flexibility** | High (Custom Two-Pass engine) | Native Flexbox (Yoga) | CSS Flexbox / Grid |
| **Terminal Mode Recovery** | Synchronous Crossterm reset | `signal-exit` / `writeSync` | N/A (GUI) |
| **Verdict** | **KEEP & RESTRUCTURING TARGET** | Reference Oracle Only | Non-Compliant |

**Decision**: Retain native Rust (`Ratatui` / `Crossterm`) as the target frontend technology, but refactor its architecture to enforce strict separation between `PresentationState`, `LayoutEngine`, and `ApplicationRuntime`.

---

## 8. Phased Incremental Migration Plan

```text
Phase 1: Shell & Adapter Boundary
  ├── Establish PresentationState & FrontendAdapter trait
  └── Isolate AppRenderer into pure layout solver

Phase 2: Conversation Timeline & Streaming
  ├── Route StreamEvents through FrontendAdapter
  └── Migrate ChatView & Timeline blocks to PresentationTimelineItem

Phase 3: Overlays, Commands & Keyboard Router
  ├── Move Ctrl+O / Alt+T routing into FrontendAdapter
  └── Connect Command Palette & Slash Completion to PresentationState

Phase 4: Workspace Views & Tool Cards
  ├── Migrate ToolExecutionCardWidget to pure presentation view model
  └── Connect Dashboard & Explorer views to adapter snapshot

Phase 5: Legacy Clean-up & Baseline Freeze
  └── Remove deprecated state fields from UiState and finalize release binary
```

---

## 9. Compatibility & Rollback Strategy

- At every phase of the migration, the existing `brain-tui` reference implementation serves as a functional fallback.
- The `FrontendAdapter` interface ensures that both the old and new frontend architectures consume the exact same `UDSClient` protocol and `StreamEvent` schemas.
- If a phase encounters a regression, rollback involves reverting only the adapter layer while `brain-domain` and `brain-services` remain completely untouched.

---

## 10. Risks & Mitigations

- **Risk**: Regression in terminal mode recovery during unexpected process termination.
  - **Mitigation**: Retain Crossterm's synchronous cleanup hooks (`disable_raw_mode`, `LeaveAlternateScreen`, `Show`).
- **Risk**: Performance degradation during high-throughput typewriter chunk streaming.
  - **Mitigation**: Maintain the two-stage client queue pipeline where network chunks buffer independently of render queue drains.

---

## 11. Acceptance Criteria

```text
✓ Pure presentation state decoupled from domain models
✓ Clean FrontendAdapter boundary
✓ Zero changes to brain-domain, brain-services, or UDS contracts
✓ 100% test suite pass across brain-tui (100 tests)
✓ Full compliance with ADR-001 (Native Rust, 0 external JS runtimes)
```

---

## 12. Decision Gate

```text
APPROVED FOR MIGRATION ARCHITECTURE
```
