---
status: active
owner: tui
canonical: false
review_cycle: quarterly
last_reviewed: 2026-08-14
applies_to: v1.1+
---

# Terminal User Interface (TUI) Engineering & Layout Implementation Guide

> **AUTHORITY NOTICE**: This document is a **supporting engineering implementation guide** for `crates/brain-tui`.
> **CANONICAL DESIGN AUTHORITY**: All visual presentation, colors, typography, and component grammar are strictly governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) and [`docs/design/CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md).

---

## 1. System Architecture & Rendering Engine

The terminal client is implemented in Rust using `Ratatui` and `Crossterm`. It operates in an isolated alternate screen buffer (`EnterAlternateScreen`) using immediate-mode differential rendering with a target 60fps frame budget (16.6ms draw budget) to ensure zero flicker or visual tearing.

---

## 2. Layout Structure: Two-Region Vertical Stack (`FullscreenLayout`)

In strict accordance with [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md), the root layout is divided into a two-region vertical stack:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Scrollable Message Canvas (flexGrow: 1, borderless floor)                │
│    ├── Typographic Greeting Header (at top of scrollback history)           │
│    ├── User Query Blocks (with subtle '❯' prefix)                           │
│    ├── Assistant Response Blocks (markdown, syntax-highlighted code fences) │
│    ├── Inline Thinking Blocks (⠋ Thinking 2.4s)                            │
│    ├── Tool Execution Cards (✓ Read 42 lines from file.rs)                  │
│    └── Recalled Memory Provenance Chips (⟡ Recalled 4 memories)             │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. Pinned Bottom Region (flexShrink: 0)                                     │
│    ├── Floating Overlays (Slash Autocomplete Popup / Command Palette Ctrl+K)│
│    ├── Prompt Input Composer (Boxed, rounded borders, multiline expansion)  │
│    └── Status Line (Single-row borderless hint bar at y = height - 1)       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Responsive Geometry & Width Breakpoints

- **Wide Mode (>= 100 columns)**: Full comfort canvas; side-by-side drawer split for session drawer or help modal if opened.
- **Standard Mode (70–99 columns)**: Full-width single-column conversational flow.
- **Compact Mode (< 70 columns)**: Condensed greeting header, single-column prompt, overlays auto-clamped to terminal bounds.

---

## 4. Theme & Color Resolution

Theme styling is resolved at render time using semantic theme tokens defined in `crates/brain-tui/src/ui/theme/mod.rs`, mapping directly to the Claude warm neutral / terracotta palette specified in [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).
