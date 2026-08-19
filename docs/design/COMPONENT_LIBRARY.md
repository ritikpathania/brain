# Component Library Implementation Specification

> **AUTHORITY NOTICE**: This document is a **supporting engineering implementation specification** for `crates/brain-tui`.
> **CANONICAL COMPONENT AUTHORITY**: All component primitives, layout contracts, and states are strictly governed by [`docs/design/CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md) and [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).

---

## 1. Component Architecture Overview

Brain's terminal user interface implements the 18 reusable component primitives defined in [`CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md):

```text
Layer 0 (AppShell)
 ├── Layer 1 (ScrollableCanvas)
 │    ├── LogoHeader (Typographic Welcome Banner)
 │    ├── TimelineMessageStream
 │    │    ├── UserMessageBlock
 │    │    └── AssistantMessageBlock (Markdown, Code Fences)
 │    ├── ThinkingSpinnerBlock (⠋ Thinking 2.4s)
 │    ├── ToolExecutionBlock (✓ Read 42 lines from file.rs)
 │    └── RecalledMemoryChip (Inline Provenance: [Score: 0.94])
 ├── Layer 2 (PinnedBottomRegion)
 │    ├── PromptComposer (Boxed, rounded borders, multiline expansion)
 │    └── StatusLine (Single-row borderless hint bar: y = height - 1)
 └── Layer 3 (OverlayLayer)
      ├── SlashCompletionPopup (Anchored above prompt)
      ├── CommandPaletteDropdown (Ctrl+K floating search modal)
      ├── HelpOverlayModal (Keyboard & command reference)
      ├── ToolPermissionDialog (RFC-009 Security Approval)
      ├── ErrorBannerBlock (Coral red diagnostic notices)
      └── EmptyStateBlock (Clean empty view hints)
```

---

## 2. Core Primitive Specifications

### 1. `StatusLine` (Footer Bar)
* **Purpose**: Displays keyboard shortcuts and non-intrusive status hints.
* **Layout**: Exactly `1` row at the absolute bottom (`y = height - 1`), borderless (`Color::Reset`).
* **Content**: `Ctrl+K Commands  Ctrl+O Toggle Card  /help Menu`.

### 2. `PromptComposer` (Input Editor)
* **Purpose**: Primary conversational prompt and slash command editor.
* **Layout**: Boxed container with `BorderType::Rounded`. Dynamically expands vertically from 3 lines up to 8 lines based on content before enabling internal scrolling.
* **States**:
  - `Default/Unfocused`: Neutral subtle gray border (`#888888`).
  - `Focused`: Brand terracotta border (`#D97706` / `#CC785C`).
  - `Active/Streaming`: Subtle shimmering pulse animation.
  - `Permission Review`: Soft violet border (`#B1B9F9`).
  - `Error`: Coral red border (`#FF6B80`).

### 3. `ThinkingSpinnerBlock` (Reasoning Progress)
* **Purpose**: Displays active chain-of-thought analysis and graph traversal.
* **Layout**: Inline 1-row spinner (`⠋ Thinking (2.4s)...`) with 80ms Braille dot animation cycling.
* **Behavior**: Auto-collapses on completion to a single dim summary row (`Thought for 2.4s`); expandable via `Ctrl+O`.

### 4. `ToolExecutionBlock` (Tool Cards)
* **Purpose**: Displays tool execution progress, file reads, and search actions.
* **Layout**: Collapsed 1-line summary card (`✓ Read 42 lines from crates/brain-core/src/lib.rs`).
* **Behavior**: Expandable on demand via `Ctrl+O` to view full parameter tables and formatted diff outputs.

### 5. `RecalledMemoryChip` (Knowledge Provenance Projection)
* **Purpose**: Displays query provenance, retrieval scores, weight classifications, and confidence tiers from Brain's relational memory engine.
* **Confidence Tiers**:
  - `High`: $\text{score} \ge 0.85$ (`● HIGH` / `High Confidence`)
  - `Medium`: $\text{score} \ge 0.65$ (`◐ MED ` / `Medium Confidence`)
  - `Low`: $\text{score} \ge 0.40$ (`○ LOW ` / `Low Confidence`)
  - `Uncertain`: $\text{score} < 0.40$ (`Uncertain`)
* **Behavior**: Single-line inline chip at the head of assistant responses; expandable via `Ctrl+O` to inspect graph neighbor nodes and reflection provenance.
