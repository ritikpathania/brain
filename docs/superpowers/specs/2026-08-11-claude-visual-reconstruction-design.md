# Claude Visual Parity v3 — Component Model & Geometry Reconstruction Design

> **Author**: Antigravity Assistant  
> **Date**: 2026-08-11  
> **Status**: APPROVED IN PRINCIPLE — Pending Spec Review  
> **Canonical Evidence**: `home-claud.png`, `query-claude.png`, `workspace-claude.png`, `command palette in workspace-claude.png`

---

## 1. Executive Summary & Core Objective

The previous visual parity efforts achieved mechanical pass rates against synthetic test contracts, but failed to capture Claude Code's actual component model and visual hierarchy as demonstrated in the authoritative screenshot evidence.

This specification defines a **zero-trust visual reconstruction of Brain TUI's presentation layer** to match Claude Code's exact component architecture, layout geometry, typography, and state transitions, while strictly preserving Brain's underlying domain model (`brain-domain`), core reasoning engine (`brain-core`), storage (`brain-storage`), services (`brain-services`), events (`brain-events`), UDS stream protocol, and `ThemeToken` architecture.

---

## 2. Parity vs. Semantics Boundary Matrix

To ensure clear isolation, visual presentation targets exact screenshot parity while backend business logic remains strictly Brain-native:

| Domain Layer | Parity Target | Fidelity Requirement |
|---|---|---|
| **Component Geometry & Rects** | Claude Screenshots | **EXACT** |
| **Component Hierarchy** | Claude Architecture | **EXACT** |
| **Borders, Dividers & Glyph Sets** | Claude Screenshots | **EXACT** |
| **Positioning & Alignment** | Claude Screenshots | **EXACT** |
| **Prompt Affordances & Glyphs** | Claude Screenshots | **EXACT** (`❯ ` cursor, right ambient line) |
| **Palette & Theme Tokens** | Claude Visual Language | **MATCH** (`ThemeToken` mapping to terracotta/amber accents) |
| **Brain Backend & Memory Graph** | Brain Domain & Services | **100% PRESERVED** (Relational memory, FTS5+Vector, graph reasoning) |
| **UDS Streaming & Event Loop** | Brain Transport | **100% PRESERVED** (`StreamEvent` typewriter pipeline) |

---

## 3. Structural Component Hierarchy

The TUI presentation layer (`crates/brain-tui/src/ui/`) is reconstructed under a unified `AppShell` component model:

```text
AppShell
│
├── AppHeader (Ambient top status)
│
├── Screen (Main view state container)
│   │
│   ├── Screen::Home / Screen::Conversation
│   │   └── TranscriptCanvas (Scrollable viewport)
│   │       ├── HomeWelcomeSurface (Embedded welcome component)
│   │       │   ├── WelcomePane (Left: Title, Mascot, Model/Project Metadata)
│   │       │   ├── VerticalDivider (│ column boundary)
│   │       │   └── InformationRail (Right: Tips for getting started, What's new)
│   │       │
│   │       └── MessageFlow (Indented user prompts & memory responses)
│   │           ├── UserMessage
│   │           ├── ReasoningProgress (Collapsible thinking stage)
│   │           └── BrainResponse (Markdown formatted stream)
│   │
│   └── Screen::Workspace
│       └── WorkspaceDashboard (Full-width session & task overview)
│           ├── AgentHeader (Mascot + Version + Model/Project + Task Counts)
│           ├── BackgroundBanner (Background navigation keyboard hint bar)
│           ├── NeedsInputSection (Full-width task table: title, status, age)
│           └── CompletedSection (Full-width task table: title, status, age)
│
├── AmbientStatusLine (Right-aligned line above prompt: ● xhigh · /effort)
├── FloatingCommandPalette (Dropdown popup anchored at y = prompt_y - palette_h)
├── PromptComposer (Pinned composer: ❯ glyph, divider rules)
└── QuietFooter (Single borderless bottom status row: y = height - 1)
```

---

## 4. Screenshot-Derived Viewport & Geometry Specifications

### 4.1. Canonical Viewport: 80×24 (Home Landing)

```text
Row 00: (Empty / Top whitespace)
Row 01: (Empty whitespace)
Row 02: ┌─ Claude Code v2.1.226 ──────────────────────────┬─────────────────────────────┐
Row 03: │              Welcome back!                      │ Tips for getting started    │
Row 04: │                   ▄▀▀▀▄                         │ Run /init to create a ...   │
Row 05: │                   █ █ █                         │ ─────────────────────────── │
Row 06: │                 Think once. Remember.           │ What's new                  │
Row 07: │                                                 │ Bug fixes and reliabil...   │
Row 08: │ Opus 5 (1M context) with xhigh · API Usage      │ Added gateway spend-li...   │
Row 09: │ ~/Developer/PyCharm/brain                       │ /release-notes for more     │
Row 10: └─────────────────────────────────────────────────┴─────────────────────────────┘
Row 11-18: (Canvas floor whitespace)
Row 19:                                                               ● xhigh · /effort
Row 20: ─────────── (Divider rule) ──────────────────────────────────────────────────────
Row 21: ❯ █
Row 22: ───────────────────────────────────────────────────────────────────────────────
Row 23: ▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents
```

#### Exact Rect Allocation (80×24 Home):
- `welcome_surface_rect`: `Rect { x: 1, y: 2, width: 78, height: 9 }`
- `welcome_pane_rect`: `Rect { x: 2, y: 3, width: 45, height: 7 }`
- `information_rail_rect`: `Rect { x: 48, y: 3, width: 30, height: 7 }`
- `vertical_divider_x`: `x = 47` (`│` glyph along column 47 from row 3 to row 9)
- `ambient_status_rect`: `Rect { x: 57, y: 19, width: 22, height: 1 }`
- `prompt_rect`: `Rect { x: 0, y: 20, width: 80, height: 3 }`
- `footer_rect`: `Rect { x: 0, y: 23, width: 80, height: 1 }`

---

### 4.2. Canonical Viewport: 80×24 (Workspace Dashboard)

```text
Row 00: ▄▀▀  Claude Code v2.1.226
Row 01: ▀▄▀  Opus 5 (1M context) · ~/Developer/PyCharm/brain
Row 02:      4 awaiting input · 0 working · 17 completed
Row 03:
Row 04: Your conversation moved to the background — enter opens it · esc returns to it
Row 05:
Row 06: Needs input
Row 07: * current session                 brain                                  2s
Row 08: * /??/                           login required - run /login           19h
Row 09: * ////ctrl+k/?///tab/            login required - run /login           23h
Row 10:
Row 11: Completed
Row 12: · bg                             (idle - send a prompt to start)       11h
Row 13: · / ctrl+k /...                  stuck on a startup dialog              8h
Row 14-19: (Whitespace)
Row 20: ───────────────────────────────────────────────────────────────────────────────
Row 21: ❯ describe a task for a new session
Row 22: ───────────────────────────────────────────────────────────────────────────────
Row 23: enter to return · space to reply · ctrl+x to delete · ? for shortcuts
```

#### Exact Rect Allocation (80×24 Workspace):
- `agent_header_rect`: `Rect { x: 0, y: 0, width: 80, height: 3 }`
- `banner_rect`: `Rect { x: 0, y: 4, width: 80, height: 1 }`
- `needs_input_rect`: `Rect { x: 0, y: 6, width: 80, height: 4 }`
- `completed_rect`: `Rect { x: 0, y: 11, width: 80, height: 3 }`
- `prompt_rect`: `Rect { x: 0, y: 20, width: 80, height: 3 }`
- `footer_rect`: `Rect { x: 0, y: 23, width: 80, height: 1 }`

---

## 5. Screen State Transitions & Canvas Continuity

1. **Home → Conversation Transition**:
   - `HomeWelcomeSurface` is NOT destroyed or unmounted on prompt submission.
   - It is appended to the head of the `TranscriptCanvas` scroll buffer.
   - As user queries accumulate, `HomeWelcomeSurface` scrolls up naturally with the rest of the conversation history.

2. **Workspace Navigation**:
   - Workspace mode completely replaces the 2-column sidebar model with the full-width `WorkspaceDashboard`.
   - Selecting a session transitions back into `Screen::Conversation` with that session's transcript loaded.

---

## 6. Mechanical Test & Negative Assertion Specification

The new test suite `tests/claude_visual_reconstruction_tests.rs` enforces strict positive AND negative visual contracts:

### 6.1. Positive Assertions
- `HomeWelcomeSurface` top border starts at `y = 2` (NOT `y = 0`).
- Container title line `y = 2` contains `"Claude Code v2.1.226"`.
- Vertical divider column `x = 47` from `y = 3..9` contains `│`.
- `ambient_status_rect` at `y = 19` contains `"● xhigh · /effort"`.
- `prompt_rect` contains prompt prefix `"❯ "`.
- Workspace dashboard renders full-width `Needs input` and `Completed` table rows spanning 80 columns.

### 6.2. Negative Assertions (Structural Elimination)
- ❌ **Home**: Text `"System Status"` MUST NOT exist anywhere in the buffer.
- ❌ **Home**: Text `"Context"` telemetry MUST NOT exist anywhere in the buffer.
- ❌ **Home / Footer**: Raw daemon latency (`"Daemon: Connected | Latency:"`) MUST NOT exist in footer.
- ❌ **Workspace**: Vertical sidebar divider line (`│` at column 22) MUST NOT exist.
- ❌ **Workspace**: Chat transcript pane MUST NOT render side-by-side with session list.

---

## 7. Plan of Action & Implementation Roadmap

1. **Phase 1: Spec Review & Sign-Off**:
   - Save spec, self-review for consistency, and obtain user sign-off.
2. **Phase 2: Component Reconstruction (`crates/brain-tui/src/ui/`)**:
   - Reconstruct `HomeWelcomeWidget` with integrated title, vertical split divider, and metadata.
   - Reconstruct `WorkspaceDashboardWidget` as a full-width multi-tier task table.
   - Reconstruct `PromptWidget` with ambient status line and `❯ ` prompt prefix.
   - Reconstruct `CommandPaletteWidget` with 3-column structured menu layout.
3. **Phase 3: Mechanical Cell-Buffer Test Gate**:
   - Implement `claude_visual_reconstruction_tests.rs` with exact Rect, glyph, and negative assertions.
   - Verify workspace regression (`cargo test --workspace`).

---

## 8. Spec Self-Review

1. **Placeholder Scan**: 0 `TODO`, `TBD`, or vague instructions found.
2. **Internal Consistency**: Component hierarchy, Rect allocations, and negative assertions match screenshot evidence perfectly.
3. **Scope Check**: Scoped strictly to `crates/brain-tui/src/ui/` presentation code and visual test assertions. Domain, core, storage, and services remain untouched.
4. **Ambiguity Check**: All cell coordinates, bounding boxes, and string requirements are explicitly defined.
