# Claude Parity Root-Cause Analysis (v2)

> **CANONICAL ROOT-CAUSE ANALYSIS**: Systemic investigation into why Brain TUI feels visually and interactionally different from Claude Code.
> **OBJECTIVE**: Identify architectural and structural causes ranking by user-visible impact.
> **EVIDENCE TAGS**: `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]`, `[INFERRED]`, `[PROPOSED_ADAPTATION]`.

---

## 1. Systemic Root Causes (Ranked by Impact)

### Root Cause 1: Container Box Over-Use ("The Boxed Viewport Fallacy")
- **Impact Rating**: **CRITICAL (Rank 1)**
- **Systemic Issue**: Brain wraps almost every layout pane inside an explicit Ratatui `Block::default().borders(Borders::ALL)`. The Chat Viewport, Sidebar, Prompt Composer, and Status Bar all draw double or rounded border lines around themselves. `[VERIFIED_BRAIN]`
- **Why it differs from Claude**: Claude Code uses **zero borders** on its conversation canvas and window floor (`Color::Reset`). Whitespace and vertical rhythm separate elements. In Brain, borders consume ~30% of visible terminal character cells on an 80×24 screen, creating a crowded "dashboard" feel. `[VERIFIED_CLAUDE]`

---

### Root Cause 2: Permanent Multi-Pane Layout vs. Full-Width Single Canvas
- **Impact Rating**: **HIGH (Rank 2)**
- **Systemic Issue**: In `AppLayoutMode::Workspace`, Brain locks a permanent 22-column sidebar on the left side of the terminal screen by default. `[VERIFIED_BRAIN]`
- **Why it differs from Claude**: Claude Code allocates **100% of terminal width** to the conversation stream. Navigation (sessions, history, commands) is accessed ephemerally via `/session` or `Ctrl+K`. Locking a 22-column sidebar permanently compresses the conversation canvas on standard 80–120 column terminals. `[VERIFIED_CLAUDE]`

---

### Root Cause 3: Screen-Mode Switching vs. Continuous Canvas Scroll
- **Impact Rating**: **HIGH (Rank 3)**
- **Systemic Issue**: Brain handles Home vs. Workspace as mutually exclusive layout modes (`AppLayoutMode::Welcome` vs. `AppLayoutMode::Workspace`). Submitting a prompt on Home causes an abrupt screen switch to Workspace mode. `[VERIFIED_BRAIN]`
- **Why it differs from Claude**: In Claude Code, the Home logo (`LogoV2`) is simply the first block at the top of the scrollback history stream. Submitting a query on Home scrolls the logo up naturally as the conversation starts—there is no abrupt layout or screen mode shift. `[VERIFIED_CLAUDE]`

---

### Root Cause 4: Heavy Boxed Status Bar vs. Single-Line Quiet Hint Bar
- **Impact Rating**: **HIGH (Rank 4)**
- **Systemic Issue**: `StatusFooterWidget` renders a 1-row bordered panel with 4 explicit boxed metric slots (`READY`, `DAEMON`, `TOKENS`, `HELP`). `[VERIFIED_BRAIN]`
- **Why it differs from Claude**: Claude's `StatusLine.tsx` is a single-line borderless text row at the absolute bottom. It renders soft shortcut hints on the left (`? help  / commands  Ctrl+K palette`) and ambient status indicators on the right (`● Connected`), quiet and non-intrusive. `[VERIFIED_CLAUDE]`

---

### Root Cause 5: Dense Data Cards vs. Progressive Collapsible Chips
- **Impact Rating**: **MEDIUM (Rank 5)**
- **Systemic Issue**: Brain's `EvidenceCard` and `ReasoningProgressWidget` render full-width bordered cards and multi-step checklists directly in the stream. `[VERIFIED_BRAIN]`
- **Why it differs from Claude**: Claude Code aggressively uses **progressive disclosure**: long reasoning steps collapse into a single-line Braille spinner (`⠋ Thinking...`), and contextual details collapse into single-line chips (`▶ Read lib.rs (142 lines)`). `[VERIFIED_CLAUDE]`

---

## 2. Summary of Architectural Causes vs. Local Styling Fixes

```text
Dimension                    Architectural Ownership                      Styling / Widget Fix
─────────────────────────    ─────────────────────────                    ────────────────────
1. Borderless Floor          `AppRenderer` layout partitioning            Remove `Borders::ALL` from `ChatView`
2. Ephemeral Workspace       `AppLayoutMode` & `compute_layout`           Make sidebar togglable (`Ctrl+B`)
3. Single Canvas Scroll      `TuiMode::Conversation` state transition     Render Logo at top of chat scrollback
4. Single-Line Status Bar    `StatusFooterWidget` Ratatui primitive       Render borderless text row with hints
5. Collapsible Memory Chips  `EvidenceCard` view model representation     Render single-line collapsible chip (`🧠`)
```

---

*This document establishes the v2 root-cause analysis for Brain TUI UX Parity.*
