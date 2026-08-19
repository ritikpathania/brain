# Legacy UI vs. Claude Target Design Conflict Matrix

**Audit Scope**: Complete 18-Dimension Systematic Architectural & Visual Deconstruction  
**Reference Target**: [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)  
**Governance Model**: Dual-Pillar Authority Model (Separation of Visual Form from Product Semantics)

---

## 1. Executive Conflict Scorecard

```
═══════════════════════════════════════════════════════════════════════════════════════════════
                      18-DIMENSION LEGACY UI CONFLICT SCORECARD
═══════════════════════════════════════════════════════════════════════════════════════════════

• Critical / Direct Conflicts (Require Immediate Supersession): 5 dimensions
• Moderate Conflicts (Require Partial Reconciliation):           6 dimensions
• Compatible / Aligned Dimensions (Keep Supporting / Preserved): 7 dimensions
```

---

## 2. Comprehensive 18-Dimension Conflict Matrix

| Dimension | Legacy Brain UI Specification | Claude Target Visual Contract | Conflict Description | Recommended Action |
| :--- | :--- | :--- | :--- | :--- |
| **1. Visual Identity** | 8-bit pixel-art mascot ("clawd" / Memory Core avatar), decorative ASCII banner headers, synthwave gaming aesthetic. | Minimalist, elegant typographic wordmark header (`Brain 1.1.0 — Relational Memory & Coding Assistant`). **Zero pixel art, zero mascot avatars**. | 🔴 **CRITICAL CONFLICT**: Direct aesthetic contradiction. Pixel art adds visual clutter and conflicts with professional developer tool ergonomics. | **SUPERSEDE** (Eliminate mascot & pixel art; adopt clean typographic header) |
| **2. Palette** | Cyberpunk electric violet (`#6C5CE7`), neon cyan (`#00CEC9`), high-contrast gaming accents. | Warm neutral dark (`#1E1E1E`) / warm sand (`#FBF9F5`), Claude brand terracotta/orange (`#D97706` / `#CC785C`), neutral grays (`#888888`). | 🔴 **CRITICAL CONFLICT**: High-glare neon palette violates Claude's warm, low-fatigue visual identity. | **SUPERSEDE** (Replace theme tokens with Claude warm neutral/terracotta palette) |
| **3. Borders & Chrome** | Heavy double borders (`═`, `║`), nested box-in-a-box panels, outer viewport framing boxes. | **Whitespace before chrome**: Canvas floor is completely borderless (`Color::Reset`); boxes used exclusively on prompt composer and floating modals. | 🔴 **CRITICAL CONFLICT**: Unnecessary visual boxing reduces available character-cell canvas and increases rendering overhead. | **SUPERSEDE** (Remove outer viewport borders and panel containers) |
| **4. Layout** | 3-pane fixed dashboard layout (Sidebar, Main Activity, Telemetry Inspector). | **Two-Region Vertical Stack** (`FullscreenLayout`): Full-width scrollable transcript canvas (top) + pinned input/status region (bottom). | 🔴 **CRITICAL CONFLICT**: Multi-pane layout consumes 40%+ of terminal width with static telemetry rather than conversation. | **SUPERSEDE** (Adopt Claude two-region vertical stack layout) |
| **5. Navigation** | Slash-command-first modal launcher; user must choose `/session`, `/search`, or `/query` before typing. | **Prompt-First**: Conversational input prompt is the default primary focus; slash commands available as inline autocomplete dropdown. | 🟡 **MODERATE CONFLICT**: Interaction friction; users should type prompts directly without command mode switching. | **PARTIALLY_RECONCILE** (Make prompt box primary active focus; slash autocomplete as overlay) |
| **6. Prompt Composer** | Single-line text input field without multi-line expansion or rich focus states. | Multi-line auto-expanding boxed prompt composer (expands dynamically from 3 up to 8 lines) with terracotta focus border and cursor positioning. | 🟡 **MODERATE CONFLICT**: Severe ergonomic limitation when authoring multi-line prompts or pasting code blocks. | **PARTIALLY_RECONCILE** (Adopt Claude multi-line auto-expanding prompt box) |
| **7. Thinking / Reasoning** | Static uncollapsed `[REASONING]` box taking 10+ rows of fixed screen space in chat history. | Collapsible inline `⠋ Thinking (X.Xs)...` spinner block; auto-collapses to a single dim summary row on completion; expandable via `Ctrl+O`. | 🟡 **MODERATE CONFLICT**: Uncollapsed reasoning pushes assistant answers off-screen, requiring excessive manual scrolling. | **PARTIALLY_RECONCILE** (Adopt Claude collapsible thinking block lifecycle) |
| **8. Tool Execution** | Heavy bordered tables showing raw JSON arguments and unformatted output logs. | Streamlined 1-line summary cards (`✓ Read 42 lines from file.rs`); expandable on demand via `Ctrl+O` in a dedicated viewport modal. | 🟡 **MODERATE CONFLICT**: Large tool tables clutter conversation stream. | **PARTIALLY_RECONCILE** (Adopt Claude 1-line tool execution cards) |
| **9. Status** | Multi-row persistent status footer with telemetry chips, memory node counts, and engine stats. | Single-row, borderless, low-contrast status hint bar pinned at absolute bottom (`StatusLine`). | 🟡 **MODERATE CONFLICT**: Telemetry dashboard noise distracts from conversation stream. | **PARTIALLY_RECONCILE** (Condense status bar to Claude's single-row hint line) |
| **10. Command Palette** | Full-screen modal interrupting workflow and obscuring transcript context. | Floating dropdown modal overlay (`Ctrl+K`) positioned directly above input box with fuzzy search and live action dispatch. | 🟡 **MODERATE CONFLICT**: Full-screen modal breaks visual continuity. | **PARTIALLY_RECONCILE** (Align command palette geometry to floating overlay layer) |
| **11. Session History** | Persistent multi-column sidebar eating 30% of horizontal space. | Collapsible session drawer / overlay accessible via `Ctrl+S` or command palette, preserving full canvas width. | 🟢 **COMPATIBLE / ADAPTABLE** | **PARTIALLY_RECONCILE** (Re-anchor session drawer to keyboard toggle `Ctrl+S`) |
| **12. Empty State** | Dashboard telemetry cards ("Connected: Yes | Ready: Yes") with fixed pill buttons (`/session new`, `/search`, `/help`). | Clean minimal conversational greeting at the head of the transcript; scrolls out of view naturally as conversation progresses. | 🔴 **CRITICAL CONFLICT**: Telemetry cards prevent natural conversational flow and break scrollback semantics. | **SUPERSEDE** (Replace dashboard home with Claude scrollable welcome header) |
| **13. Streaming** | Direct stdout chunk printing causing flickering and irregular line breaks. | **Two-Stage Typewriter Queue**: network chunks buffered and drained sequentially at 60fps with monotonic `StreamEvent` IDs. | 🟢 **100% ALIGNED** | **KEEP_SUPPORTING** (Retain existing two-stage typewriter streaming pipeline) |
| **14. Responsive Behavior** | Hardcoded 80×24 layout assumptions; truncated panels on narrow screens. | Fluid Yoga-style flexbox layout with graceful degradation at 70 columns and automatic two-pass layout reflow. | 🟢 **100% ALIGNED** | **KEEP_SUPPORTING** (Retain existing two-pass layout engine and resize hooks) |
| **15. Accessibility** | Inconsistent contrast in cyberpunk neon themes; missing ANSI 16-color fallbacks. | Strict WCAG AA contrast (4.5:1 minimum), high-contrast black/white mode, and robust ANSI 16-color fallbacks. | 🟢 **COMPATIBLE / ALIGNED** | **KEEP_SUPPORTING** (Retain accessibility rules, updated to Claude contrast tokens) |
| **16. Motion & Animation** | Ad-hoc terminal refresh loops. | 60fps tick rate (16.6ms budget), 80ms braille spinner cycling (`⠋`, `⠙`, `⠹`, `⠸`), smooth typewriter drain. | 🟢 **100% ALIGNED** | **KEEP_SUPPORTING** (Retain existing motion and performance budgets) |
| **17. Typography** | Large untracked ASCII headers, decorative uppercase badges with glowing outlines. | Subtle semantic hierarchy: bold section headers, clean regular body text, syntax-highlighted monospace code blocks. | 🟡 **MODERATE CONFLICT**: Visual noise from excessive decorative headers. | **PARTIALLY_RECONCILE** (Standardize typography hierarchy to Claude conventions) |
| **18. Component Hierarchy** | Ad-hoc Z-indexing causing overlay collisions. | Strict 4-layer Z-ordering: Layer 0 (Floor) $\rightarrow$ Layer 1 (Stream) $\rightarrow$ Layer 2 (Pinned Composer/Status) $\rightarrow$ Layer 3 (Overlays). | 🟢 **100% ALIGNED** | **KEEP_SUPPORTING** (Retain existing 4-layer Z-ordering architecture) |

---

## 3. Structural Decision Boundary

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 CORE DECISION BOUNDARY                                      │
├──────────────────────────────────────────────────────────────┬──────────────────────────────┤
│               WHAT MUST CHANGE (VISUAL FORM)                 │  WHAT MUST NOT CHANGE        │
├──────────────────────────────────────────────────────────────┼──────────────────────────────┤
│ 1. Eliminate pixel art mascot, Memory Core avatar, and       │ 1. Relational Knowledge      │
│    synthwave electric purple/cyan branding.                  │    Graph data models.        │
│ 2. Replace 3-pane dashboard with borderless conversation     │ 2. Hybrid BM25 + Vector +    │
│    floor and pinned bottom composer stack.                   │    RRF search pipeline.      │
│ 3. Replace static telemetry home screen with scrollable      │ 3. SQLite transactional      │
│    typographic greeting header.                              │    persistence layer.        │
│ 4. Replace static reasoning/tool boxes with collapsible      │ 4. Unix Domain Socket (UDS)  │
│    Claude thinking blocks and 1-line tool execution cards.   │    streaming protocol.       │
│ 5. Adopt multi-line auto-expanding prompt composer.          │ 5. Multi-session tracking    │
│ 6. Replace neon theme tokens with Claude warm neutral/       │    and context pinning.      │
│    terracotta palette tokens.                                │ 6. Zero fake cloud features. │
└──────────────────────────────────────────────────────────────┴──────────────────────────────┘
```
