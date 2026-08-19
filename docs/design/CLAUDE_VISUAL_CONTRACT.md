# Claude Visual Contract: Canonical Frontend Design Authority

**Document Status**: `CANONICAL SPECIFICATION` (Sole Visual & UI/UX Authority for Brain Frontend)
**Target Architecture**: Native Rust Terminal User Interface (`crates/brain-tui` / Ratatui)
**Governance Scope**: Normative contract governing all visual layout, component hierarchy, typography, color tokens, and interaction states.
**Provenance & Ground Truth**: Derived directly from local Claude Code source tree (`/Users/ritikpathania/Developer/src`) and empirical evidence from commit `38cbb06b`.

---

## 1. Architectural Philosophy & Dual-Pillar Foundation

```text
                 ┌─────────────────────────────────────────────────────────────┐
                 │                Claude Source Ground Truth                   │
                 │    /Users/ritikpathania/Developer/src + 38cbb06b Evidence   │
                 └──────────────────────────────┬──────────────────────────────┘
                                                │ derives / verifies
                                                ▼
                 ┌─────────────────────────────────────────────────────────────┐
                 │                CLAUDE_VISUAL_CONTRACT.md                    │
                 │         Sole Normative Visual & Interaction Authority       │
                 └──────────────────────────────┬──────────────────────────────┘
                                                ▼
                 ┌─────────────────────────────────────────────────────────────┐
                 │                CLAUDE_COMPONENT_MODEL.md                    │
                 │          18 Reusable UI Component Primitives                │
                 └──────────────────────────────┬──────────────────────────────┘
                                                ▼
                 ┌─────────────────────────────────────────────────────────────┐
                 │              BRAIN_CLAUDE_SURFACE_MAPPING.md                │
                 │             Brain Capability Surface Projection             │
                 └──────────────────────────────┬──────────────────────────────┘
                                                ▼
                 ┌─────────────────────────────────────────────────────────────┐
                 │               Supporting Engineering Specs                  │
                 │   (TUI_DESIGN_SYSTEM, COMPONENT_LIBRARY, Layouts, Motion)   │
                 └──────────────────────────────┬──────────────────────────────┘
                                                ▼
                               Frontend Engine (crates/brain-tui)
```

### The Dual-Pillar Model
```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 CANONICAL DUAL-PILLAR MODEL                                 │
├──────────────────────────────────────────────┬──────────────────────────────────────────────┤
│         CLAUDE VISUAL CONTRACT               │            BRAIN PRODUCT MAPPING             │
│       (Visual & Structural Form)             │         (Capabilities & Semantics)           │
├──────────────────────────────────────────────┼──────────────────────────────────────────────┤
│ • Clean typographic hierarchy                │ • Relational Knowledge Graph Engine          │
│ • Borderless conversational canvas           │ • Hybrid BM25 + Vector Search + RRF Fusion   │
│ • Warm neutral / terracotta palette          │ • Temporal decay & memory consolidation      │
│ • Collapsible thinking & tool cards          │ • Multi-session conversation tracking        │
│ • Boxed, auto-expanding prompt composer      │ • Local SQLite transactional persistence     │
│ • Whitespace before chrome                   │ • Local UDS socket IPC streaming             │
└──────────────────────────────────────────────┴──────────────────────────────────────────────┘
```

> **Design Invariant**: The visual contract governs the *observable visual structure and presentation grammar* of the frontend. It does NOT remove Brain's backend capabilities or invent unbacked Claude cloud features (e.g. fake model or effort selectors).

---

## 2. Global Application Shell & Root Partitioning

### 2.1 Screen Buffer & Root Layout
- **Terminal Buffer Mode**: Full-screen alternate buffer (`EnterAlternateScreen` / `LeaveAlternateScreen`).
- **Layout Principle**: **Two-Region Vertical Stack** (`FullscreenLayout` in Monospace Character Cell Space):
  1. **Top Region (Scrollable Canvas)**: `flexGrow: 1`, `flexShrink: 1`. Holds welcome banner, conversation stream, assistant answers, thinking blocks, and tool activity cards.
  2. **Bottom Region (Pinned Input & Status)**: `flexGrow: 0`, `flexShrink: 0`. Pinned at bottom of viewport. Holds floating overlays, prompt composer, and single-row status bar.

### 2.2 Concrete Geometry Metrics (Recovered from Claude Source & `38cbb06b`)
- `MIN_INPUT_VIEWPORT_LINES = 3`: Minimum vertical height of prompt composer.
- `PROMPT_FOOTER_LINES = 5`: Reserved lines for prompt box + status line footer.
- `MODAL_TRANSCRIPT_PEEK = 2`: Rows of transcript context visible above floating modal dialog dividers.
- `LOGO_BREAKPOINT = 70`: Terminal column threshold for full two-panel greeting header vs compact header.
- `PICKER_HEIGHT_BOUND`: `visibleCount = Math.max(MIN_VISIBLE, Math.min(requestedVisible, rows - CHROME_ROWS))`.

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Scrollable Message Canvas (flexGrow: 1, overflow: hidden)                 │
│    ├── Top Welcome / Greeting Banner (scrolls naturally with conversation)  │
│    ├── Historical User Queries                                              │
│    ├── Assistant Responses & Inline Markdown Code Blocks                    │
│    ├── Collapsible Thinking Blocks (⠋ Thinking 2.4s)                       │
│    ├── Collapsible Tool Execution Cards (✓ Read file.rs)                    │
│    └── Recalled Context & Memory Provenance Chips                           │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. Pinned Bottom Region (flexShrink: 0)                                     │
│    ├── Floating Overlay Layer (Command Palette / Slash Autocomplete Popup) │
│    ├── Prompt Input Composer (Themed single-line rounded box)               │
│    └── Status Line (Single-row borderless hint bar: y = height - 1)         │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. The 20 Canonical Design Dimensions

### 1. Application Shell
- Full-terminal coverage with zero outer framing borders.
- Background floor is completely transparent / default terminal background (`Color::Reset`).
- No outer window chrome, titlebars, or bounding container boxes.

### 2. Sidebar & Navigation
- Primary mode is full-width single-column conversation.
- Session history drawer and workspace switcher are accessible as clean collapsible side-drawers (`Ctrl+S`) or via the command palette (`Ctrl+K`), maximizing horizontal space for code and reasoning.

### 3. Conversation Surface
- Free-flowing vertical stream where user messages and assistant responses sit directly on the floor.
- Clean 1-cell vertical separation between messages (`spacing.normal`).
- Streaming tokens drain smoothly via a typewriter queue at 60fps.

### 4. Message Presentation
- **User Prompt**: Rendered with subtle visual distinction (subtle bold prompt prefix `❯` with `userMessageBackground`).
- **Assistant Response**: Rendered in standard primary text color with rich markdown support (headers, lists, code fences with syntax highlighting).
- Code blocks are enclosed in clean, subtle single-line rounded boxes with language identifiers.

### 5. Composer & Input
- **Boxed Composer**: Always rendered inside a dedicated single-line rounded border (`BorderType::Rounded`).
- **Dynamic Multi-Line Expansion**: Automatically expands vertically from 3 lines up to 8 lines based on user typing before enabling internal scrolling.
- **Prompt Border States (Source Grounded)**:
  - `Default/Unfocused`: Neutral subtle gray (`promptBorder: rgb(136,136,136)` / `#888888`).
  - `Focused`: Claude brand terracotta/orange (`claude: rgb(215,119,87)` / `#D77757`).
  - `Streaming/Active`: Shimmering pulse animation (`claudeShimmer: rgb(235,159,127)` / `#EB9F7F`).
  - `Permission Review`: Soft violet (`permission: rgb(177,185,249)` / `#B1B9F9`).
  - `Error`: Coral red (`error: rgb(255,107,128)` / `#FF6B80`).

### 6. Empty States & Home Screen
- **Clean Typographic Greeting**: Rendered via `LogoV2` / `LogoHeader` at the head of the transcript:
  - `columns >= 70`: Two-panel split (Greeting title & brand on left `max 50`, `Tips for getting started` on right).
  - `columns < 70`: Compact single-panel header.
- No pixel art, no mascot avatars, no neon banners.
- Below the greeting: concise, subtle hint text indicating slash commands (`Type / for commands, Ctrl+K for palette`).
- **Crucial Invariant**: The greeting sits at the head of the scrollback buffer and scrolls naturally out of view once a conversation begins.

### 7. Loading & Thinking States
- **Thinking Block**: Active reasoning displayed as an inline spinner `⠋ Thinking (X.Xs)...`.
- On completion: Freezes the elapsed duration (e.g. `Thought for 3.2s`) and collapses to a single dim summary line.
- Expandable on demand via `Ctrl+O` to view full chain-of-thought tokens.

### 8. Tool Execution & Error States
- **Tool Cards**: 1-line collapsed summary showing tool name and target:
  - Active: `⠋ Reading crates/brain-core/src/lib.rs...`
  - Success: `✓ Read 42 lines from crates/brain-core/src/lib.rs`
  - Failure: `✗ Failed to read file: Not found` (styled in coral red).
- Expandable via `Ctrl+O` to view full arguments and outputs in a scrollable modal.

### 9. Menus & Popovers
- **Slash Command Autocomplete**: Floating popup menu (`FuzzyPicker`) anchored directly above the prompt composer box, displaying matched commands (`/help`, `/session`, `/memory`, `/doctor`), descriptions, and shortcuts.
- **Command Palette**: Centered modal overlay (`Ctrl+K` / `GlobalSearchDialog`) with fuzzy-matching search bar and keyboard navigation (`↑`/`↓` to select, `Enter` to execute).

### 10. Buttons & Controls
- Terminal-native keyboard-first controls.
- Action hints rendered in subtle dim brackets: `[y/n/always]`, `[Enter to confirm]`, `[Esc to dismiss]`.

### 11. Typography & Hierarchy
- Clean, semantic typography without large untracked text:
  - `H1 / Title`: Bold primary text with underline.
  - `H2 / Section`: Bold primary text.
  - `Body`: Clean regular text.
  - `Muted / Hints / Timestamps`: Dim gray text.
  - `Code / Symbols`: Monospace syntax-colored spans.

### 12. Spacing & Rhythm
- Character-cell mathematical grid:
  - `spacing.micro = 0 cells`: Inline badges, tags.
  - `spacing.normal = 1 cell`: Standard gap between message blocks, padding inside prompt.
  - `spacing.relaxed = 2 cells`: Margin above prompt composer.
  - `spacing.gutter = 2 cells`: Horizontal split gutters.

### 13. Source-Grounded Color & Theme System (`utils/theme.ts`)

| Theme Token | Exact Claude Source Value | Semantic Meaning |
| :--- | :--- | :--- |
| `claude` | `rgb(215,119,87)` / `#D77757` | Claude signature brand accent & focused prompt border |
| `claudeShimmer` | `rgb(235,159,127)` / `#EB9F7F` | Active streaming shimmer highlight |
| `promptBorder` | `rgb(136,136,136)` / `#888888` | Inactive / default prompt composer border |
| `promptBorderShimmer` | `rgb(166,166,166)` / `#A6A6A6` | Composer border shimmer highlight |
| `subtle` | `rgb(80,80,80)` / `#505050` | Dividers (`─`), subtle borders, muted glyphs |
| `permission` | `rgb(177,185,249)` / `#B1B9F9` | Permission request dialog highlight |
| `autoAccept` | `rgb(175,135,255)` / `#AF87FF` | Auto-accept permission status & merged facts |
| `planMode` | `rgb(72,150,140)` / `#48968C` | Plan Mode review and confirmation banner |
| `userMessageBackground` | `rgb(30,30,30)` / `#1E1E1E` | Subtle background fill for user message cards |
| `userMessageBackgroundHover`| `rgb(40,40,40)` / `#282828` | Hover background fill for message inspection |
| `text` | `rgb(255,255,255)` / `#FFFFFF` | Primary foreground text (Dark mode) |
| `inactive` | `rgb(153,153,153)` / `#999999` | Secondary / dimmed metadata and timestamps |
| `success` | `rgb(78,186,101)` / `#4EBA65` | Success indicators (`✓`) and healthy status |
| `error` | `rgb(255,107,128)` / `#FF6B80` | Error indicators (`✗`) and failure alerts |
| `warning` | `rgb(255,193,7)` / `#FFC107` | Warnings and caution notices |
| `diffAdded` | `rgb(34,92,43)` / `#225C2B` | Unified diff added line background |
| `diffRemoved` | `rgb(122,41,54)` / `#7A2936` | Unified diff removed line background |

### 14. Borders & Surfaces
- **Whitespace over Chrome**: Strictly no double borders (`═`, `║`).
- Container boxes use single-line rounded corners (`╭`, `╮`, `╯`, `╰`) when UTF-8 is available, falling back to clean ASCII (`+`, `-`, `|`).
- Zero nested card containers.

### 15. Responsive Layouts & Breakpoints
- **Breakpoint 1 (Wide: >= 100 cols)**: Full comfort canvas, side-by-side split for help/session drawer if opened.
- **Breakpoint 2 (Standard: 70–99 cols)**: Full-width single-column conversational flow, two-panel greeting header.
- **Breakpoint 3 (Compact: < 70 cols)**: Condensed logo, single-column prompt, overlays auto-clamp to terminal bounds.

### 16. Interaction States
- Five discrete UI state modes:
  1. `Idle / Prompt Focused`: Default state, prompt box highlighted.
  2. `Streaming / Generating`: Prompt in shimmering state, follow-tail auto-scrolling active.
  3. `Scrolled Up / Reviewing`: User manually scrolled history, `↓ New messages` pill displayed.
  4. `Autocomplete / Palette Active`: Floating overlay captures arrow keys and Enter.
  5. `Modal Permission Review`: Input focus transferred to tool permission dialog.

### 17. Motion & Animation
- **Tick Rate**: 60fps UI refresh loop (16.6ms frame budget).
- **Smooth Streaming**: Tokens delivered over UDS are buffered into a typewriter queue and drained sequentially to eliminate network chunk stutter.
- **Spinner Rates**: 80ms braille spinner cycling (`⠋`, `⠙`, `⠹`, `⠸`, `⠼`, `⠴`, `⠦`, `⠧`, `⠇`, `⠏`).

### 18. Accessibility
- **WCAG AA Compliance**: 4.5:1 minimum contrast ratio across all text tokens against background.
- **ANSI 16-Color Mode**: Graceful degradation to standard 16 terminal colors when 24-bit TrueColor is unavailable.
- **High-Contrast Theme**: Pure black/white high-contrast mode for screen readers and accessibility profiles.

### 19. Component Hierarchy & Z-Ordering
- Strict 4-layer Z-ordering in the renderer:
  - **Layer 0 (Base)**: Background floor canvas.
  - **Layer 1 (Stream)**: Scrollable messages, thinking blocks, and tool cards.
  - **Layer 2 (Pinned Footer)**: Prompt composer box and bottom status line.
  - **Layer 3 (Overlays)**: Floating slash popup, command palette modal, tool permission dialog, `new_messages_pill`.

### 20. Viewport & Terminal Lifecycle
- Handles terminal window resizing dynamically (`SIGWINCH`) without text corruption or negative rectangle panics.
- Restores terminal mouse mode and alternate screen buffer cleanly on exit or `SIGINT` / `Ctrl+C`.

---

## 4. Capability Provenance & Negative Guardrails

| Visual Component | Brain Projection | Backend Implementation | Allowed Cloud Capabilities |
| :--- | :--- | :--- | :--- |
| Message Canvas | Session transcript | `crates/brain-services` | Native Brain conversation |
| Thinking Spinner | Knowledge compilation / Reflection | Background UDS events | Streaming reflection facts |
| Tool Card | Execution progress | `crates/brain-tools` | Local tool operations |
| Memory Provenance Chip | Recalled facts / RRF score | `crates/brain-storage` | Local relational facts |
| Session Drawer | Saved workspaces | `crates/brain-services` | Local SQLite sessions |
| **Model Selector** | **NOT PERMITTED** | **None** | **Strictly Forbidden (No fake models)** |
| **Effort Selector (`/effort`)** | **NOT PERMITTED** | **None** | **Strictly Forbidden (No fake effort)** |
| **Cloud Billing UI** | **NOT PERMITTED** | **None** | **Strictly Forbidden (No fake cloud)** |

---

## 5. Normative Authority Statement

This document constitutes the **sole canonical design authority** for Brain's frontend visual presentation. All subsequent UI implementations in `crates/brain-tui` must strictly conform to the specifications herein.
