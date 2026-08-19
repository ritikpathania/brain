# Architecture Correction & Migration Plan: Literal Claude Frontend Baseline

> **Objective**: Remove all Brain-specific presentation inventions from `packages/brain-frontend` and adopt Claude Code's literal frontend source from `/Users/ritikpathania/Developer/src` as the independent presentation baseline, followed by a clean, non-intrusive integration boundary.  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
ARCHITECTURE RESTRUCTURING: TWO-PHASE MIGRATION
================================================================================

PHASE 1: LITERAL CLAUDE FRONTEND (INDEPENDENT PRESENTATION BASELINE)
  • Authoritative Source: /Users/ritikpathania/Developer/src
  • Zero Brain branding ("BRAIN", fake mascots, daemon status, memory status)
  • Literal Clawd mascot (POSES: default, look-left, look-right, arms-up)
  • Literal Claude LogoV2, CondensedLogo, FeedColumn, and Feed components
  • Literal PromptInput, PromptInputFooter, and StatusLine
  • Literal Markdown, HighlightedCode, Thinking, and Tool components
  • Fixture-driven rendering identical to Claude Code REPL

PHASE 2: THIN BRAIN INTEGRATION BOUNDARY
  • Connects Brain Rust Daemon via BrainUdsClient and Controller/Adapter
  • Maps Brain capabilities strictly INTO existing Claude UI surfaces:
      - Brain tool executions   ──► Claude ToolUse & ToolResult UI
      - Brain memory provenance ──► Standard Claude message metadata
      - Brain streaming tokens  ──► Standard Claude AssistantTextMessage
      - Brain sessions/search   ──► Standard Claude FuzzyPicker / Dialog
  • ZERO custom UI chrome or invented presentation widgets
================================================================================
```

---

## 1. Inventory of Identified Brain Presentation Inventions

The following Brain-specific inventions currently present in `packages/brain-frontend` must be completely removed:

| Identified Brain Invention | Current Location | Problem / Discrepancy | Correction Strategy |
|---|---|---|---|
| **Invented `***` Mascot** | `components/LogoV2.tsx` | Replaced Clawd with arbitrary `***` / ASCII art. | Replace with literal `Clawd.tsx` from Claude source with exact `POSES` dictionary. |
| **"BRAIN BRAIN V1.1.0" Title** | `components/LogoV2.tsx` | Injected Brain engine version and branding. | Restore Claude Code standard title ` Claude Code v<version> ` and `Welcome to Claude Code!`. |
| **Invented Memory / Daemon Status** | `components/LogoV2.tsx`, `StatusLine.tsx` | Injected `Brain (connected) · Memory (active)` into logo and footer. | Restore Claude's standard model line (`Claude 3.5 Sonnet · Pro Plan`) and working directory. |
| **Invented "Getting Started" Feed** | `components/LogoV2.tsx` | Injected custom Brain command list (`/reflect`, `/compile`, etc.). | Restore Claude's canonical `FeedColumn` and `Feed` (`Tips`, `What's New`, `Recent Activity`). |
| **Custom Memory Notification Chip** | `components/messages/UserMemoryInputMessage.tsx` | Created custom UI badge `⟡ Recalled N memories`. | Remove custom presentation widget; format provenance as standard message context if present. |
| **Invented Status Bar Footer** | `components/StatusLine.tsx` | Custom Brain footer displaying memory node/edge counts. | Restore canonical Claude `StatusLine` layout. |

---

## 2. Phase 1 Migration Plan: Literal Claude Component Tree

The presentation layer will be structured directly after `/Users/ritikpathania/Developer/src`:

```text
packages/brain-frontend/src/
├── components/
│   ├── FullscreenLayout.tsx                 # Literal Claude 2-region flex container
│   ├── Messages.tsx                         # Literal Claude transcript canvas
│   ├── MessageRow.tsx                       # Literal Claude turn row
│   ├── LogoV2/
│   │   ├── Clawd.tsx                        # Literal Clawd ASCII mascot (POSES segments)
│   │   ├── LogoV2.tsx                       # Literal Claude 2-panel welcome box
│   │   ├── CondensedLogo.tsx                # Literal compact welcome header (<70 cols)
│   │   ├── FeedColumn.tsx                   # Literal FeedColumn layout
│   │   └── Feed.tsx                         # Literal Feed (Tips / What's New)
│   ├── PromptInput/
│   │   ├── PromptInput.tsx                  # Literal prompt composer (1-8 rows, ▌ cursor)
│   │   └── PromptInputFooter.tsx            # Literal shortcut hints footer
│   ├── StatusLine.tsx                       # Literal borderless status line
│   ├── GlobalSearchDialog.tsx               # Literal command palette dialog
│   ├── HelpV2.tsx                           # Literal shortcuts modal
│   ├── FuzzyPicker.tsx                      # Literal command autocomplete popup
│   ├── Markdown.tsx                         # Literal Markdown lexer & AST renderer
│   ├── HighlightedCode.tsx                  # Literal rounded code box
│   └── messages/
│       ├── UserPromptMessage.tsx            # Literal #1E1E1E prompt card
│       ├── AssistantTextMessage.tsx         # Literal streaming text message
│       ├── AssistantThinkingMessage.tsx     # Literal ∴ Thinking indicator & trace
│       ├── AssistantToolUseMessage.tsx      # Literal ● tool_name(args) & permission UX
│       └── UserToolResultMessage.tsx        # Literal 20-line line-numbered output drawer
└── theme/
    └── tokens.ts                            # Literal Claude darkTheme tokens & glyphs
```

---

## 3. Phase 2 Integration Architecture

Once Phase 1 is verified with identical visual rendering:

```text
       ┌─────────────────────────────────────────────────────────┐
       │             LITERAL CLAUDE PRESENTATION LAYER           │
       │    (Clawd, LogoV2, PromptInput, Messages, ToolUse)      │
       └────────────────────────────┬────────────────────────────┘
                                    │ (standard Claude props)
       ┌────────────────────────────▼────────────────────────────┐
       │             THIN BRAIN INTEGRATION LAYER                │
       │  (Translates Brain runtime events into Claude props)    │
       └────────────────────────────┬────────────────────────────┘
                                    │ (JSONL / IPC)
       ┌────────────────────────────▼────────────────────────────┐
       │              BRAIN UDS CLIENT & CONTROLLER              │
       └────────────────────────────┬────────────────────────────┘
                                    │ (UNIX Domain Socket)
       ┌────────────────────────────▼────────────────────────────┐
       │                    BRAIN RUST DAEMON                    │
       │        (Relational memory, hybrid search, tools)        │
       └─────────────────────────────────────────────────────────┘
```

---

## 4. Execution Steps

1. **Step 1 — Port Canonical Claude Mascot (`Clawd.tsx`)**:
   Implement the exact `POSES` segments (`default`, `look-left`, `look-right`, `arms-up`) from `/Users/ritikpathania/Developer/src/components/LogoV2/Clawd.tsx`.
2. **Step 2 — Port Canonical Claude Header (`LogoV2.tsx`, `CondensedLogo.tsx`, `FeedColumn.tsx`, `Feed.tsx`)**:
   Implement the exact rounded border box with ` Claude Code v<version> `, `Welcome to Claude Code!`, model descriptor, cwd, and `FeedColumn`.
3. **Step 3 — Clean Up Prompt & Footer (`PromptInput.tsx`, `PromptInputFooter.tsx`, `StatusLine.tsx`)**:
   Align prompt composer, footer hints, and status line to pure Claude layout.
4. **Step 4 — Purge Invented Presentation Chips**:
   Remove `UserMemoryInputMessage.tsx` custom badge and any leftover Brain-specific UI chrome.
5. **Step 5 — Verify Pure Claude Presentation Fixture Rendering**:
   Render Claude landing, turn, thinking, tool, and modal states; verify output visually and mechanically.
6. **Step 6 — Wire Thin Integration Boundary**:
   Connect `BrainFrontendController` to populate Claude's props without altering UI structure.
