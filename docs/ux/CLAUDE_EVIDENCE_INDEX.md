# Claude Code vs. Brain TUI Canonical Evidence Index

> **CANONICAL EVIDENCE INDEX**: Single source of truth linking every visual contract rule, component primitive, gap classification, and proposed adaptation to explicit source code or empirical evidence.
> **MILESTONE**: `Claude Visual Parity Reconciliation — Forensics & Contract v2`

---

## 1. Evidence Classification Taxonomy

Every claim in the v2 specification is tagged with one of 4 evidence types:
1. `[VERIFIED_CLAUDE]`: Direct source code evidence (`/Users/ritikpathania/Developer/src`) or empirical execution observation of Claude Code CLI `2.1.226`.
2. `[VERIFIED_BRAIN]`: Direct source code evidence (`crates/brain-tui/src/`) or snapshot test output of Brain TUI baseline `1c3df23a059b5e7a545f63af5f8b4f08389d2767`.
3. `[INFERRED]`: Logical deduction from UX principles, layout geometry rules, or terminal constraints.
4. `[PROPOSED_ADAPTATION]`: Target design adaptation mapping Brain-native features (memory recall, sessions, UDS daemon) to Claude Code's visual grammar.

---

## 2. Master Evidence Index

| ID | Specification Item | Claude Source Evidence | Brain Source Baseline Evidence | Classification Tag |
|---|---|---|---|---|
| **E1** | Screen buffer mode | `<AlternateScreen>` in `FullscreenLayout.tsx` line 14 | `<AlternateScreen>` in `application.rs` line 80 | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E2** | Global layout container | `FullscreenLayout.tsx` flex column | `AppRenderer::compute_layout` in `renderer.rs` line 105 | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E3** | Canvas floor borders | Zero borders on `MessageHistory.tsx` floor | `Borders::ALL` on `ChatView` in `chat.rs` line 40 | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E4** | Logo header placement | Renders at head of transcript stream in `LogoV2.tsx` | Separate `AppLayoutMode::Welcome` screen mode in `renderer.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E5** | Logo width breakpoint | `columns >= 70` horizontal split in `logoV2Utils.ts` line 88 | `columns >= 70` in `renderer.rs` line 210 | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E6** | Prompt border focus color | `rgb(215,119,87)` (`claude` token in `PromptInput.tsx`) | `ThemeToken::Primary` in `style.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E7** | Spacing constants | `spacing.normal` = 1 cell, `spacing.relaxed` = 2 cells in `DESIGN.md` | Monospace line padding in `renderer.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E8** | Thinking spinner | Single-line Braille spinner in `LoadingState.tsx` | 4-row stage checklist in `reasoning_progress.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E9** | Tool execution summary | Single-line collapsible (`▶ Read lib.rs`) in `ToolProgress.tsx` | Boxed evidence cards in `evidence_card.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E10** | Status line structure | Single-line borderless row (`y = height - 1`) in `StatusLine.tsx` | 4-slot boxed status panel in `status_footer.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E11** | Command palette | Floating dropdown above prompt in `GlobalSearchDialog.tsx` | Centered modal box in `palette.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E12** | Workspace sidebar | No permanent sidebar; stream has 100% width | Permanent 22-column left split in `renderer.rs` | `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]` |
| **E13** | Relational memory recall | Not present in Claude | Collapsible single-line summary (`🧠 Recalled 3 memories`) | `[PROPOSED_ADAPTATION]` |
| **E14** | Session selection | `/session` command & `Ctrl+K` picker | Left sidebar selection & `/session` command | `[PROPOSED_ADAPTATION]` |

---

## 3. Cell-Buffer Visual State Diffing Protocol

Visual state validation for Phase 0 forensic verification operates at the **terminal cell level** (`80×24` viewport):

```text
Cell Buffer Model:
Cell { char: char, fg: Color, bg: Color, modifier: Modifier }

Deterministic Cell Diff Equation:
Diff(State) = CellBuffer(Brain_Actual) ⊕ CellBuffer(Target_Contract)
Gate Criterion: Zero visual noise, 0 unwanted outer container border cells on conversation floor.
```

---

## 4. Frozen System Boundaries (Guaranteed Non-Touch)

| Architecture / Subsystem | Frozen Boundary Guarantee |
|---|---|
| `brain-domain` | 100% Frozen (Entities, aggregates, pure domain events) |
| `brain-core` | 100% Frozen (Reasoning runner, contracts) |
| `brain-storage` | 100% Frozen (SQLite schema, durability, `SessionServiceImpl`) |
| `brain-services` | 100% Frozen (Service orchestration) |
| `brain-events` | 100% Frozen (UDS stream protocol, monotonic `StreamEvent`) |
| `ThemeToken` System | 100% Preserved (Semantic tokens, palettes, ANSI fallbacks) |

---

*This document establishes the official v2 Evidence Index for Claude Code vs. Brain TUI Visual Parity Reconciliation.*
