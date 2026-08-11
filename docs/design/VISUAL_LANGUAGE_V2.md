# Brain Visual Language v2 Proposal

> **STATUS: PROPOSED DESIGN SPECIFICATION**  
> **RULE**: This document defines the visual design system evolution for Brain's TUI. All changes operate strictly within the existing frozen architecture (Projection-Layer principle, `ThemeToken`, `LANDING_PAGE.md` v1.0, `COMPONENT_LIBRARY.md`, `CommandRanker`, `StreamAdapter`, `ReasoningProgressState`, `EvidenceCard`).

---

## 1. Executive Summary & Design Vision

**Goal**: Achieve **Claude-grade visual polish** while preserving **Brain-native product identity and brand colors**.

Claude Code's key design advantage is not a specific color or logo; it is **interaction hierarchy, subtle surfaces, whitespace restraint, and single-line dividers**. Brain Visual Language v2 transfers this visual grammar into Brain's relational memory & knowledge exploration engine while maintaining Brain's distinct visual identity and color palette.

```text
Claude Visual Grammar                   Brain Visual Identity (v2)
─────────────────────                   ──────────────────────────
Whitespace hierarchy                    Memory Core Brand Identity
Restrained borders                      Relational Memory Hierarchy
Subtle surface fills                    Brain Neutral/Charcoal Palette
Muted chrome                            Brain Accent & Selection Tokens
Floating overlay pickers                Evidence Provenance Hierarchy
Single-line transient hints             Knowledge-Oriented Glyphs
Subtle vertical split rules (│)         Calm Document/Editor Feeling
```

---

## 2. Visual Audit & Brand Differentiation

| Visual Dimension | Claude Code CLI | Brain Visual Language v2 (Target) |
| :--- | :--- | :--- |
| **Brand Colors** | Warm Orange (`rgb(215,119,87)`), Permission Violet | **Brain-Native Accent & Selection Tokens** (Distinct identity) |
| **Chrome & Borders** | Single-cell `─` dividers, subtle borders | **Single subtle vertical divider rule (`│`)** between sidebar & chat |
| **Background Floor** | Native terminal floor (`Color::Reset`) | Native terminal floor (`Color::Reset`) |
| **Card Surfaces** | Muted container fills | **Subtle container fills with single-line headers** |
| **Command Palette** | Floating, minimal padding, single hint row | **Floating borderless overlay, single hint row** |
| **Workspace Layout** | Full-width stream, status line pinned | **Calm document/editor layout (subtle `│` split)** |
| **Reasoning Progress**| Braille dot spinner + verb label | **Preserved Phase C transient stage checklist (`○`/`●`/`✓`)** |
| **Evidence Cards** | Scannable metadata, subtle borders | **Preserved Phase D rank-preserving tiers** |
| **Responsive Rules** | `< 80` Compact, `80–120` Standard, `> 120` Wide | **Preserved Option A breakpoint (`< 70` Compact)** |

---

## 3. Typography & Hierarchy System

Terminal output is strictly monospace. Visual hierarchy is established via **text attributes, color tokens, and whitespace**:

```text
Visual Level       Style Treatment                                 Use Case
─────────────────────────────────────────────────────────────────────────────────────────────
Level 1 (Headline) Bold + Primary / HeaderPrimary Token           App Brand, Major View Headers
Level 2 (Section)  Bold + TextSecondary Token                     Section Titles, Section Dividers
Level 3 (Body)     Normal Weight + TextPrimary Token              Chat Messages, Prompt Input Text
Level 4 (Muted)    Normal Weight + TextMuted Token                Timestamps, Unfocused Descriptions
Level 5 (Accent)   Bold + Accent / Success / Danger Token         Selected Items, Badges, Statuses
```

---

## 4. Component-by-Component Delta

### A. Landing Page (`Screen::Home` — Welcome Mode)
- **Current State**: Unified Memory Core ASCII mark, state line (`● Ready`), tagline *"Think once. Remember forever."*, launcher items (`/session new`, `/search`, `/help`), prompt (`❯`).
- **v2 Proposal**: Zero layout changes (Preserves frozen `LANDING_PAGE.md` v1.0). Visual polish via whitespace padding and crisp `HeaderPrimary` text contrast.

### B. Command Palette Overlay (`PaletteWidget`)
- **Current State**: Floating borderless modal area, query line, section headers, command list, single-line keyboard hint footer. Clamped height.
- **v2 Proposal**:
  - Selected item row highlighted with `Accent` cursor (`▶ `) and `TextPrimary` bold name.
  - Section headers rendered in `TextSecondary` bold with single-cell indentation.
  - Bottom hint line styled cleanly in `TextMuted`.

### C. Workspace Layout & Conversation Stream
- **Current State**: Sessions sidebar (22 cols), chat stream, prompt input (3 rows), status footer (1 row).
- **v2 Proposal**:
  - Replace heavy panel borders between sidebar and chat with a single subtle vertical divider (`│` in `BorderSubtle` / `Muted`).
  - Conversation messages styled cleanly with generous vertical gap (1 blank line between messages).

### D. Search & Evidence Projection (`EvidenceCard` & `ConfidenceBadge`)
- **Current State**: Bordered evidence cards with score, source provenance, weight classification, and matched terms.
- **v2 Proposal**:
  - Restrained card headers (`[1] Evidence — Score 0.94`).
  - Score badge (`● HIGH (0.94)`) using semantic `Success` / `Warning` / `Danger` colors.
  - Matched terms rendered with subtle `CodeInline` background styling.

### E. Reasoning Progress Indicator (`ReasoningProgressWidget`)
- **Current State**: Scoped by `ExecutionId`. Stages (`○ Pending`, `● Active`, `✓ Completed`). Auto-collapses on first token.
- **v2 Proposal**: Preserved completely without layout mutation.

---

## 5. Architectural Pipeline Safeguards

All Visual Language v2 modifications strictly respect the **Projection-Layer Principle**:

```text
┌─────────────────────────────────────────────────────────┐
│                    Domain & Runtime                     │
│   (CommandRanker, StreamAdapter, Retrieval, Progress)   │
└────────────────────────────┬────────────────────────────┘
                             │ (Read-Only Data Events)
                             ▼
┌─────────────────────────────────────────────────────────┐
│                    UI State & ViewModels                │
│    (UiState, PaletteViewModel, MemoryResultsViewModel)  │
└────────────────────────────┬────────────────────────────┘
                             │ (Semantic Style Tokens)
                             ▼
┌─────────────────────────────────────────────────────────┐
│               Ratatui Presentation Layer                │
│      (AppRenderer, PaletteWidget, EvidenceCard, etc.)   │
└─────────────────────────────────────────────────────────┘
```

- **NO** business logic, scoring, or retrieval code inside widgets.
- **NO** hardcoded hex/RGB colors outside `Palette` definitions.
- **NO** changes to frozen contracts (`LANDING_PAGE.md`, `COMPONENT_LIBRARY.md`).
