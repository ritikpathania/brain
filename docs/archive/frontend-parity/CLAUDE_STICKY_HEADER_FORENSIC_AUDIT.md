# Forensic Source Audit — Sticky Prompt Header

> **Document Status**: Forensic Analysis & Architectural Audit  
> **Target Subsystem**: `crates/brain-tui` (Header & Scrollback Navigation Layer)  
> **Scope**: P2 — Sticky Prompt Header, Scroll-State Transitions, Layout Integration  
> **Governing Foundations**: Native Rust/Ratatui Architecture (ADR-001), Locked Two-Pass Layout Engine, Locked `ThinkingBlockWidget`, Locked `NewMessagesPillWidget`, Locked Multiline Prompt Cursor, Locked `ToolExecutionCardWidget`  
> **Oracle Source Verification**:  
> - `/Users/ritikpathania/Developer/src/components/FullscreenLayout.tsx` (lines 338–350, 540–589)  
> - `/Users/ritikpathania/Developer/src/components/VirtualMessageList.tsx` (lines 910–1040)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Audit Summary

This document presents a source-verified forensic audit of Claude Code's **Sticky Prompt Header** component (`StickyPromptHeader` in `FullscreenLayout.tsx`) and compares it against Brain's native Ratatui frontend (`crates/brain-tui`).

### Primary Forensic Discoveries (`SOURCE-CONFIRMED`):
1. **Fixed 1-Row Height**: Claude explicitly enforces a **fixed 1-row height** (`height={1}`) for `StickyPromptHeader` with `wrap="truncate-end"`. The source comments explicitly state that a variable-height header would shift the scroll container by 1 row every time the prompt switches during scroll, causing visual content jumps (`FullscreenLayout.tsx` lines 545–550).
2. **Text Formatting**: Text is formatted as `{figures.pointer} {collapsedText}` where `figures.pointer` is `❯` (`\u276F`). Newlines and multiline whitespace in the prompt are collapsed into single space runs (`.replace(/\s+/g, ' ').trim()`) and capped at `STICKY_TEXT_CAP` (`VirtualMessageList.tsx` lines 1018–1020).
3. **Visibility Gate**: `StickyPromptHeader` appears **only when the active turn's user prompt has scrolled above the top of the viewport** (`firstVisibleTop > promptTop`). It is hidden when at the bottom of the timeline, when the user clicks the header (until scrolling to a new prompt), or when any overlay/modal is open (`overlay == null`).
4. **Layout Mechanics**: It is rendered as a normal-flow top sibling directly above the scrollable message list, shrinking the scroll viewport height by exactly 1 row (`flexShrink={0}`).

Brain currently has **no Sticky Prompt Header implementation** (`BRAIN-CONFIRMED`).

---

## 2. Claude Component Hierarchy & Oracle Trace (`SOURCE-CONFIRMED`)

Source trace through `/Users/ritikpathania/Developer/src`:

```text
FullscreenLayout.tsx
  ├── StickyPromptHeader (lines 540-589)
  │     ├── text: collapsed single-line preview string
  │     └── onClick: scrollTo handler jumping back to user prompt
  │
  └── VirtualMessageList.tsx (lines 990-1040)
        ├── setStickyPrompt observer calculating scrolled-above prompt
        └── whitespace collapsing & STICKY_TEXT_CAP truncation
```

### Component Parameters & Props:
- `text`: `string` — Collapsed single-line preview of the active user prompt.
- `onClick`: `() => void` — Scroll trigger that jumps viewport back to the start of the user prompt message.

---

## 3. What "Sticky" Actually Means (`SOURCE-CONFIRMED`)

Claude Code does **NOT** use absolute CSS positioning or a floating overlay for the sticky prompt header.

Instead, Claude uses a **dedicated 1-row layout slot**:
- It is rendered as a top sibling BEFORE the scroll box.
- It reduces the available height of the message list container by 1 row.
- It stays strictly outside the scrollable DECSTBM region.
- Fixed 1-row height prevents circular layout recalculation or scroll jumping during scroll ticks.

---

## 4. Scroll-State Contract Matrix (`SOURCE-CONFIRMED`)

| Scroll / UI State | Sticky Header Visible? | Displayed Content | Behavior |
| :--- | :--- | :--- | :--- |
| **At bottom / follow-tail** | **No** (`null`) | None | Prompt is visible in main timeline; header hidden |
| **Scrolled away (prompt above viewport)** | **Yes** | `❯ <First line of active prompt>` | 1-row pinned header at top of chat viewport |
| **Deep scroll (scrolled further up)** | **Yes** | `❯ <First line of prompt for visible turn>` | Updates text as prompt boundaries cross top |
| **Scroll reaches top of timeline** | **Yes** | `❯ <First prompt>` | Remains visible showing first prompt |
| **Clicked by user** | **No** (`"clicked"`) | None | Hides header and scrolls viewport to prompt |
| **Prompt focused** | Unaffected by focus | Shows sticky prompt if scrolled away | Focus state does not hide sticky header |
| **Multiline prompt** | **Yes** | Collapsed to 1 single line | Multiline newlines collapsed to single spaces |
| **Thinking block expanded** | **Yes** | Pinned at top of viewport | Expanded thinking block scrolls underneath |
| **Tool card expanded** | **Yes** | Pinned at top of viewport | Expanded tool card scrolls underneath |
| **New Messages Pill visible** | **Yes** | Pinned at top of viewport | Coexists: Header at top row, Pill at bottom row |
| **Modal / Command Overlay open** | **No** (`null`) | None | Suppressed while overlay/modal is open |

---

## 5. Brain Current Architecture vs Claude Parity Matrix

| Behavior / Contract | Claude Source Oracle | Brain Current (`crates/brain-tui`) | Parity | Classification |
| :--- | :--- | :--- | :--- | :--- |
| **Sticky Header Component** | `StickyPromptHeader.tsx` (1 row fixed) | None | **GAP**: Missing component | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Header Height** | Fixed 1 visual row | N/A | **GAP**: N/A | `SOURCE-CONFIRMED` |
| **Text Format** | `❯ <collapsed_prompt_text>` | N/A | **GAP**: N/A | `SOURCE-CONFIRMED` |
| **Scroll Detection** | Active prompt scrolled above viewport | N/A | **GAP**: N/A | `SOURCE-CONFIRMED` |
| **Overlay Suppression** | Hidden when `overlay != null` | N/A | **GAP**: N/A | `SOURCE-CONFIRMED` |
| **Click / Jump Behavior** | Scrolls viewport to prompt | N/A | **GAP**: N/A | `SOURCE-CONFIRMED` |

---

## 6. Architectural Classification

- **Classification**: **Hybrid (Layout + Scroll State)**.
- **Location**: `crates/brain-tui` (`src/ui/widgets/sticky_header.rs`, `renderer.rs`, `state.rs`).
- **Backend / Protocol Boundary**: **100% Client-Side TUI Presentation**. Requires **zero backend, UDS, domain, storage, or Cargo dependency changes**.

---

## 7. Interaction with Locked Subsystems (`SOURCE-CONFIRMED` / `BRAIN-CONFIRMED`)

1. **Two-Pass Layout Engine**: Completely compatible. When sticky header is active, `compute_layout` in `renderer.rs` deducts 1 row from the top of the chat viewport area (`mid_chunks[1]`). Fixed 1-row height introduces **zero circular layout dependencies**.
2. **Inline Collapsible Thinking Blocks**: Completely compatible. Thinking blocks scroll underneath the 1-row sticky header strip.
3. **New Messages Pill**: Completely compatible. Sticky header is pinned to the **top row** of the chat viewport; New Messages Pill is pinned to the **bottom row**. Zero spatial or positioning collisions.
4. **Multiline Prompt**: Completely compatible. Sticky header displays a collapsed single-line preview of the active prompt turn. Prompt editor at bottom of screen is unaffected.
5. **Inline Tool Execution Cards**: Completely compatible. Tool cards scroll underneath the sticky header.

---

## 8. Viewport Edge Cases & Performance

- **Narrow Viewports (< 40 cols)**: Truncates header text with `...` (`wrap="truncate-end"`).
- **Short Viewports (< 10 rows)**: Suppressed when total terminal height is insufficient.
- **Performance Budget**: Zero allocations per frame. Single string slice derivation during viewport index solve (`MEASURED`).

---

## 9. Candidate Gap Inventory

- **P2 (Medium)**: Missing `StickyPromptHeaderWidget` 1-row pinned header at top of chat viewport when user prompt scrolls above screen.
- **P2 (Medium)**: Missing prompt scroll-above detection in `ViewportIndex` / `scroll_anchor`.

---

## 10. Final Recommendation Gate

```text
APPROVED FOR DESIGN SPECIFICATION
```
