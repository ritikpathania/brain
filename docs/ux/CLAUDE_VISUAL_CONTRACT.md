# Claude Code TUI Reverse-Engineered Visual Contract (v2)

> **CANONICAL SPECIFICATION**: Reverse-engineered visual contract of Claude Code TUI extracted directly from source code (`/Users/ritikpathania/Developer/src`), Ink component trees, Yoga layout rules, and observed execution behaviors.
> **EVIDENCE TAGS**: All claims herein are explicitly tagged:
> - `[VERIFIED_CLAUDE]` (Source code or empirical observation of Claude Code)
> - `[VERIFIED_BRAIN]` (Source code or empirical observation of Brain TUI baseline)
> - `[INFERRED]` (Logical deduction from UX principles or architecture)
> - `[PROPOSED_ADAPTATION]` (Target design adaptation for Brain-native features)

---

## 1. Global Layout Architecture & Screen Division

### 1.1 Root Buffer & Screen Model
- **Screen Buffer Mode**: `<AlternateScreen>` full-terminal isolated buffer. `[VERIFIED_CLAUDE]`
- **Root Layout Engine**: Yoga Layout (Flexbox in Monospace Character Cell Space). `[VERIFIED_CLAUDE]`
- **Global Layout Container**: `FullscreenLayout.tsx` (`flexDirection: 'column'`, `width: '100%'`, `height: '100%'`). `[VERIFIED_CLAUDE]`

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Scrollable Message Canvas (`flexGrow: 1`, `flexShrink: 1`, `overflow: hidden`) │
│    ├── Welcome / Logo Block (renders at top of transcript stream, NOT fixed)│
│    ├── Historical User Messages                                             │
│    ├── Historical Assistant Responses & Tool Blocks                         │
│    └── Recalled Context / Memory summary chips                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. Pinned Bottom Region (`flexShrink: 0`, `flexGrow: 0`)                    │
│    ├── Thinking / Reasoning Spinner (`⠋ Thinking...`)                        │
│    ├── Active Tool Execution Progress (`Reading file...`)                  │
│    ├── Floating Overlay Layer (Command Palette / Slash Completion Dropdown)│
│    ├── Prompt Input Composer (Boxed / Focused Accent Border)                │
│    └── Status Footer Bar (`StatusLine.tsx` — single borderless hint row)   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Viewport Constraints & Cell-Level Dimensions
- **Vertical Partitioning**: `[VERIFIED_CLAUDE]`
  - `Scrollable Region`: Consumes `100% - Pinned Bottom Height`.
  - `Prompt Input Box`: Minimum `3` visible text rows (`MIN_INPUT_VIEWPORT_LINES = 3`), maximum reserved footer rows `5` (`PROMPT_FOOTER_LINES = 5`).
  - `Status Line`: Exactly `1` row height pinned at absolute bottom (`y = height - 1`).
- **Horizontal Partitioning**: `[VERIFIED_CLAUDE]`
  - Full-width `100%` terminal width for conversation canvas.
  - Logo horizontal breakpoint (`columns >= 70` → two-panel: Clawd art `max 50` cols | Feed right `min 30` cols; `columns < 70` → compact single panel).

---

## 2. Component Hierarchy & Z-Ordering

```text
FullscreenLayout [VERIFIED_CLAUDE]
 ├── ScrollableCanvas (flexGrow: 1) [VERIFIED_CLAUDE]
 │    ├── LogoV2 (at head of scrollback) [VERIFIED_CLAUDE]
 │    ├── MessageList [VERIFIED_CLAUDE]
 │    │    ├── UserMessageBlock [VERIFIED_CLAUDE]
 │    │    └── AssistantMessageBlock [VERIFIED_CLAUDE]
 │    │         ├── ThinkingBlock (collapsible) [VERIFIED_CLAUDE]
 │    │         ├── ToolActivityBlock (collapsible) [VERIFIED_CLAUDE]
 │    │         └── MarkdownContent [VERIFIED_CLAUDE]
 │    └── RecalledMemoryChip (inline collapsible) [PROPOSED_ADAPTATION]
 └── PinnedBottomRegion (flexShrink: 0) [VERIFIED_CLAUDE]
      ├── ActiveSpinner / ReasoningProgress [VERIFIED_CLAUDE]
      ├── FloatingOverlayLayer (rendered right above prompt) [VERIFIED_CLAUDE]
      │    ├── SlashCompletionPopup (anchored to prompt line) [VERIFIED_CLAUDE]
      │    ├── CommandPaletteModal (floating dropdown overlay) [VERIFIED_CLAUDE]
      │    └── HelpOverlayModal [VERIFIED_CLAUDE]
      ├── PromptComposer [VERIFIED_CLAUDE]
      │    ├── InputBorder (themed box) [VERIFIED_CLAUDE]
      │    └── InputText (Emacs/Vim line editor) [VERIFIED_CLAUDE]
      └── StatusLine (single-line borderless hint bar) [VERIFIED_CLAUDE]
```

---

## 3. Padding, Margins & Cell-Level Rhythm

### 3.1 Spacing Constants (Character Cells) `[VERIFIED_CLAUDE]`

| Spacing Token | Cell Value | Use Case |
|---|---|---|
| `spacing.micro` | `0` cells | Tight inline badges, status badges |
| `spacing.normal` | `1` cell | Vertical gap between messages, padding inside prompt box |
| `spacing.relaxed` | `2` cells | Margin above prompt composer, major section separation |
| `spacing.gutter` | `2` cells | Horizontal gap between two-column splits (e.g. Logo & Feed) |

### 3.2 Cell-Level Vertical Rhythm Rules `[VERIFIED_CLAUDE]`
- **Gap between User Prompt & Assistant Answer**: Exactly `1` blank row (`spacing.normal`).
- **Gap between Messages**: Exactly `1` blank row (`spacing.normal`).
- **Gap above Prompt Composer**: Exactly `1` blank row (`spacing.normal`) for optical breathing room.
- **Footer Placement**: Immediately below prompt box with `0` cell margin, rendered on absolute last row (`y = height - 1`).

---

## 4. Border Usage & Rounded Box Behavior

### 4.1 Border Invariant: Whitespace Before Chrome `[VERIFIED_CLAUDE]`
- **Main Viewport Floor**: **BORDERLESS** (`Color::Reset`). The conversation stream, logo, and messages sit directly on the terminal background without outer container boxes (`│`, `─`, `┌`, `└`).
- **Prompt Composer**: **BOXED** using single-line borders (`Rounded` when supported, ASCII fallback).
- **Modals / Dropdowns**: **BOXED** with single-line subtle borders (`subtle` color `rgb(80,80,80)`).
- **Dividers**: Single-line horizontal dividers (`─`) in `subtle` color, never double borders.

### 4.2 Prompt Border State System `[VERIFIED_CLAUDE]`

| State | Border Color Token | RGB Color Value |
|---|---|---|
| Default / Unfocused | `promptBorder` | `rgb(136,136,136)` (Neutral Gray) |
| Focused | `claude` | `rgb(215,119,87)` (Brand Orange) |
| Permission Request | `permission` | `rgb(177,185,249)` (Soft Violet) |
| Error / Warning | `error` | `rgb(255,107,128)` (Coral Red) |
| Shimmering (Active) | Alternates `base` ↔ `lightShimmer` | ~80ms frame rate cycle |

---

## 5. Semantic Theme Integration (Preserving Theme Architecture) `[PROPOSED_ADAPTATION]`

Brain's semantic theme architecture (`ThemeToken`, `ActiveTheme`, `ThemePalette`) is preserved 100%. The visual refinement maps Claude's visual semantics to Brain's semantic tokens:

```text
Claude Semantic Token                 Brain ThemeToken Equivalent
─────────────────────                 ───────────────────────────
bgPrimary (Color::Reset)         →    ThemeToken::BackgroundFloor
bgSecondary (Surface fill)       →    ThemeToken::SurfaceCard
textPrimary (Body text)          →    ThemeToken::TextPrimary
textSecondary (Subtitles)        →    ThemeToken::TextSecondary
textMuted (Hints/Timestamps)     →    ThemeToken::TextMuted
accent (Brand highlight)         →    ThemeToken::AccentPrimary
success (Soft green)             →    ThemeToken::StatusSuccess
warning (Soft yellow)            →    ThemeToken::StatusWarning
error (Soft red)                 →    ThemeToken::StatusDanger
```

---

## 6. Deterministic Cell-Buffer Diff Specification `[PROPOSED_ADAPTATION]`

For a fixed terminal geometry (`80×24`), visual state validation operates via deterministic cell-buffer comparison (`character + fg + bg + attributes`):

```text
Viewport State = 80x24, Theme = Dark, Focus = Prompt

Expected Cell Buffer Rules:
1. Row 0..17: Conversation Canvas floor (bg: Reset, outer borders: NONE).
2. Row 18: Blank spacing row (bg: Reset).
3. Row 19..21: Prompt Composer Box (border: Single/Rounded, fg: AccentPrimary).
4. Row 22: Blank spacing row or overlay anchor (bg: Reset).
5. Row 23: Status Line (y = 23, bg: Reset, fg: TextMuted, borders: NONE).
```

---

*This document establishes the official v2 reverse-engineered Claude Code Visual Contract.*
