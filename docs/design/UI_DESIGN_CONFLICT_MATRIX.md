# UI/UX Design Conflict Matrix: Old Brain UI vs. Claude Visual Contract

**Document Purpose**: Systematic identification and categorization of all architectural, visual, and interaction conflicts between the obsolete Brain UI specifications and the target **Claude Visual Contract**.

---

## 1. Conflict Summary by Category

```
═══════════════════════════════════════════════════════════════════════════════════════════════
                               DESIGN CONFLICT CATEGORY SUMMARY
═══════════════════════════════════════════════════════════════════════════════════════════════

Category A: Visual Language         🔴 SEVERE CONFLICT (Branding, palette, pixel art, chrome)
Category B: Information Architecture 🔴 SEVERE CONFLICT (Dashboard-first vs. Conversation-first)
Category C: Interaction Model        🟡 MODERATE CONFLICT (Slash-command primacy vs. Prompt-first)
Category D: Component Model          🟡 MODERATE CONFLICT (Boxed widgets vs. Streamlined stream)
Category E: Responsive Behavior      🟢 COMPATIBLE / ADAPTABLE (Flexbox & terminal resize)
Category F: Frontend Architecture    🟢 COMPATIBLE / ALIGNED (Ratatui immediate-mode engine)
```

---

## 2. Detailed Systematic Conflict Analysis

### A. Visual Language & Aesthetics

| Dimension | Obsolete Brain UI Specification | Target Claude Visual Contract | Conflict Severity | Resolution Strategy |
| :--- | :--- | :--- | :---: | :--- |
| **Brand Identity & Mascot** | Features 8-bit pixel art mascot ("clawd" / Memory Core avatar) and decorative ASCII banners. | Minimalist clean logo typography; **zero pixel art**, zero mascots. | 🔴 **CRITICAL** | Eliminate pixel-art avatar and mascot references; adopt clean typographic logo header. |
| **Color Palette** | Electric violet (`#6C5CE7`), neon cyan (`#00CEC9`), synthwave/cyberpunk accents. | Warm neutral dark (`#1E1E1E`) and warm sand (`#FBF9F5`), Claude brand orange/terracotta (`#D97706` / `#CC785C`), neutral grays (`#888888`). | 🔴 **CRITICAL** | Replace palette tokens in theme definitions with Claude's warm neutral/terracotta palette. |
| **Surfaces & Chrome** | Heavy double-border boxes, nested panels, high visual noise, decorative borders on all regions. | **Whitespace before chrome**: Canvas floor is completely borderless (`Color::Reset`); boxes used exclusively on prompt composer and modals. | 🔴 **CRITICAL** | Remove outer viewport borders and panel containers; render messages directly on floor background. |
| **Typography & Hierarchy** | Mixed ASCII headers, uppercase badges with colored borders, gradient-style keywords. | Clean semantic hierarchy: subtle bold headings, dim timestamps/hints, clean monospace code blocks with syntax styling. | 🟡 **MODERATE** | Standardize typography tokens to match Claude's subtle hierarchy. |

---

### B. Information Architecture (IA)

| Dimension | Obsolete Brain UI Specification | Target Claude Visual Contract | Conflict Severity | Resolution Strategy |
| :--- | :--- | :--- | :---: | :--- |
| **Home Screen / Landing Page** | Dashboard layout with large telemetry boxes ("Connected: Yes | Ready: Yes"), 3 fixed action buttons (`/session new`, `/search`, `/help`). | Clean minimal conversational greeting at the head of the transcript; scrolls out of view as conversation progresses. | 🔴 **CRITICAL** | Supersede `LANDING_PAGE.md`; implement Claude scrollable welcome header. |
| **Main Screen Structure** | 3-pane fixed dashboard (Sidebar, Main Activity, Telemetry Inspector). | **Two-Region Vertical Stack**: Full-width scrollable transcript canvas (top) + pinned input/status region (bottom). | 🔴 **CRITICAL** | Transition top-level layout to Claude's two-region vertical stack (`FullscreenLayout`). |
| **Session & History Navigation** | Persistent multi-column sidebar eating 30% of horizontal terminal space. | Collapsible session drawer / overlay accessible via `Ctrl+S` or command palette, preserving full canvas width. | 🟡 **MODERATE** | Re-anchor session navigation to a collapsible drawer or command palette modal. |
| **Status Presentation** | Multi-row persistent status footer with telemetry chips, memory node counts, and engine stats. | Single-row, borderless, low-contrast status hint bar at absolute bottom (`StatusLine`). | 🟡 **MODERATE** | Condense status bar to Claude's single-row hint line. |

---

### C. Interaction Model

| Dimension | Obsolete Brain UI Specification | Target Claude Visual Contract | Conflict Severity | Resolution Strategy |
| :--- | :--- | :--- | :---: | :--- |
| **Input Primacy** | Slash-command first: User expected to explicitly choose `/session`, `/search`, or `/query` before typing. | **Prompt-First**: User types natural language prompt directly; slash commands available as inline autocomplete dropdown. | 🟡 **MODERATE** | Make prompt editor the primary active focus; show slash commands as popup overlay above prompt. |
| **Command Palette** | Full-screen modal interrupting workflow. | Floating dropdown modal (`Ctrl+K`) positioned directly above the input box with fuzzy search and live preview. | 🟡 **MODERATE** | Align command palette geometry to Claude's floating overlay layer. |
| **Tool Approval & Permissions** | Textual prompts inside chat stream. | Themed permission card with soft violet border (`#B1B9F9`) and single-key approval (`[y/n/always]`). | 🟢 **LOW / ALIGNED** | Preserves RFC-009 modal pattern, aligned with Claude permission card styling. |

---

### D. Component Model

| Component | Obsolete Brain UI Specification | Target Claude Visual Contract | Conflict Severity | Resolution Strategy |
| :--- | :--- | :--- | :---: | :--- |
| **Thinking / Reasoning** | Static `[REASONING]` box taking 10+ rows of text in chat history. | Collapsible `⠋ Thinking (2.4s)` block; auto-collapses to single summary row on response completion; expandable via `Ctrl+O`. | 🟡 **MODERATE** | Adopt Claude thinking block lifecycle and keyboard expansion. |
| **Tool Execution Cards** | Heavy bordered tables showing raw arguments and outputs. | Streamlined 1-line summary cards (`Reading file.rs...`) with spinner during execution; checkmark `✓` on completion; expandable via `Ctrl+O`. | 🟡 **MODERATE** | Adopt Claude tool card 6-state lifecycle and 1-line collapsed representation. |
| **Prompt Composer** | Simple single-line text input field. | Multi-line auto-expanding boxed prompt composer with accent border on focus, cursor positioning, and file attachment chips. | 🟡 **MODERATE** | Adopt Claude multi-line prompt box with dynamic height expansion. |
| **New Messages Notification** | Static prompt notice. | Floating pill overlay (`↓ New messages`) anchored above the prompt when user is scrolled up in history. | 🟢 **LOW / ALIGNED** | Retain existing `new_messages_pill` widget, styling to Claude muted accent token. |
| **Sticky Header** | None or rigid header box. | Dynamic sticky context header that pins the active prompt when scrolling deep into long assistant responses. | 🟢 **LOW / ALIGNED** | Retain existing `sticky_header` widget with Claude styling. |

---

### E. Responsive Behavior

| Dimension | Obsolete Brain UI Specification | Target Claude Visual Contract | Conflict Severity | Resolution Strategy |
| :--- | :--- | :--- | :---: | :--- |
| **Terminal Dimensions** | Hardcoded 80×24 assumptions; broken layouts below 80 columns. | Fluid resize (`SIGWINCH`), graceful degradation for narrow terminals (<70 cols), two-panel layout on wide screens (>=100 cols). | 🟢 **ALIGNED** | Leverage existing two-pass layout engine and viewport matrix tests. |
| **Scroll Synchronization** | Global terminal scrolling. | Independent canvas scrollback with follow-tail pinning on new streaming tokens and manual scroll decoupling. | 🟢 **ALIGNED** | Retain existing follow-tail scroll invariants. |

---

### F. Frontend Architecture

| Architectural Layer | Obsolete Architecture / Remnants | Target Claude-Aligned Ratatui Architecture | Alignment Status |
| :--- | :--- | :--- | :---: |
| **Rendering Engine** | Historical React/Ink prototypes (retired in `packages/brain-frontend`). | Pure Rust **Ratatui** immediate-mode differential rendering loop with Crossterm backend. | 🟢 **100% ALIGNED** |
| **State Machine** | Scattered component state. | Centralized Elm/Redux-style **UI State Reducer** (`App::handle_action`) with immutable state snapshots. | 🟢 **100% ALIGNED** |
| **Data Transport** | Ad-hoc network calls. | Non-blocking **Unix Domain Socket (UDS)** streaming protocol with monotonic `StreamEvent` sequence IDs. | 🟢 **100% ALIGNED** |
| **Streaming Queue** | Direct stdout flushing causing flicker. | **Two-Stage Typewriter Queue**: network chunks buffered and drained smoothly at 60fps. | 🟢 **100% ALIGNED** |

---

## 3. Core Architectural Conclusion

The underlying Rust/Ratatui engineering architecture (`crates/brain-tui`), state reducers, UDS streaming protocol, and two-pass layout engine are **completely sound and aligned with modern high-performance terminal UI requirements**. 

The conflicts are strictly concentrated in **visual presentation, branding, color tokens, layout chrome, and information architecture**:
1. **Remove**: Pixel art, cyberpunk violet/neon palette, heavy boxed borders, static dashboard telemetry.
2. **Adopt**: Claude's clean typographic hierarchy, warm terracotta/sand palette, borderless conversation canvas, collapsible thinking/tool cards, and multi-line auto-expanding prompt composer.
3. **Preserve**: Brain's backend memory engine, hybrid RRF search, graph consolidation, and local session storage.
