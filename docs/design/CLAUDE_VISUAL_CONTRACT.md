# Claude Visual Reference & Architecture Contract

> **Document Classification:** Official Audit & Reference Specification for Claude Code CLI TUI.
> **Source Reference Location:** `/Users/ritikpathania/Developer/src/DESIGN.md` and TypeScript source codebase in `/Users/ritikpathania/Developer/src`.

---

## 1. Executive Reference Facts

### [REFERENCE FACT] Core Technical Stack & Layout Model
- **Framework**: React + Ink (terminal React renderer) + Yoga Layout (flexbox character-cell engine).
- **Surface Floor**: Transparent by default. Text and borders render directly over the user's terminal emulator background context.
- **Character Grid Unit**: Spacing, margins, padding, and borders are measured strictly in integer character cells (`cols` × `rows`). No pixel math or fractional character offsets exist.
- **Typography Hierarchy**: Single monospaced terminal font. Hierarchy is established exclusively via ANSI styling attributes: **Bold**, *Italic*, <u>Underline</u>, Dim, Inverse, and color tokens.

### [REFERENCE FACT] Exact Color Token Definitions
Source: `/Users/ritikpathania/Developer/src/DESIGN.md` (Lines 31–119)

| Token Name | RGB Value | Hex Value | Primary Purpose in Claude UI |
|---|---|---|---|
| `claude` / `primary` | `rgb(215,119,87)` | `#D77757` | Brand warm orange. Logo, focused borders, active headers, spinner default. |
| `claudeShimmer` | `rgb(235,159,127)` | `#EB9F7F` | Lighter orange for breathing shimmer animation on odd frames (~80ms). |
| `text` | `rgb(255,255,255)` | `#FFFFFF` | Primary body text on dark backgrounds. |
| `inverseText` | `rgb(0,0,0)` | `#000000` | Selection text and inverted labels. |
| `inactive` | `rgb(153,153,153)` | `#999999` | Dimmed content, metadata, timestamps, idle statuses. |
| `inactiveShimmer` | `rgb(193,193,193)` | `#C1C1C1` | Lighter gray shimmer. |
| `subtle` | `rgb(80,80,80)` | `#505050` | Horizontal line rules (`─`), low-contrast chrome, dividers. |
| `promptBorder` | `rgb(136,136,136)` | `#888888` | Medium gray. Default unfocused prompt input box border. |
| `promptBorderShimmer` | `rgb(166,166,166)` | `#A6A6A6` | Shimmer for active prompt input box border. |
| `suggestion` / `permission` | `rgb(177,185,249)` | `#AFB9F9` | Electric blue-purple. Command palette types, search highlights, key hints. |
| `autoAccept` / `merged` | `rgb(175,135,255)` | `#AF87FF` | Violet. Auto-approve and merged states. |
| `bashBorder` | `rgb(253,93,177)` | `#FD5DB1` | Bright pink. Shell command execution container borders. |
| `success` | `rgb(78,186,101)` | `#4EBA65` | Green. Completed tasks, resolved files, status indicators. |
| `error` | `rgb(255,107,128)` | `#FF6B80` | Red. Errors, rejections, missing configuration alerts. |
| `warning` | `rgb(255,193,7)` | `#FFC107` | Amber. Caution states, rate-limit warnings. |
| `selectionBg` | `rgb(38,79,120)` | `#264F78` | Dark blue selection background highlight in menus and tables. |
| `userMessageBackground` | `rgb(55,55,55)` | `#373737` | Container background for user prompt message bubbles. |

---

## 2. Component Hierarchy & Screen Topography

### [REFERENCE FACT] Screen Layout Hierarchy

```text
Terminal Viewport (cols × rows)
│
├── Content Area (flexGrow: 1, flexDirection: "column")
│   │
│   ├── HomeWelcomeSurface (Screen::Home)
│   │   ├── Top Border Box (y = 2, height = 9)
│   │   │   ├── Title: " Claude Code v2.1.226 " in claude orange
│   │   │   ├── Left Welcome Pane: Mascot logo, "Welcome back!", metadata
│   │   │   ├── Vertical Divider: │ at column 47
│   │   │   └── Right Information Rail: "Tips for getting started", "What's new"
│   │   └── Transcript Canvas (scrollable history buffer)
│   │
│   └── WorkspaceDashboard (Screen::Workspace)
│       ├── Header: "▄▀▀ Claude Code v2.1.226", context path, session counts
│       ├── Navigation Banner: "Your conversation moved to the background..."
│       ├── Needs Input Section: Task list with full-width row selection
│       └── Completed Section: Task list with idle status
│
├── Ambient Status Line (y = prompt_y - 1, right-aligned: "● xhigh · /effort")
│
├── Prompt Input Composer (bounded horizontal rules top & bottom)
│   ├── Prompt Prefix: "❯ " (bold)
│   └── Cursor / Input Line: "Ask anything..." or typed prompt
│
└── Status Footer Line (y = height - 1, borderless space-between paragraph)
    └── Left: "▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents"
```

---

## 3. Screen-by-Screen Geometry & Reference Contract

### A. HOME SCREEN CONTRACT

#### [REFERENCE FACT] Surface Bounds & Spacing
- **Top Margin**: Row `y = 2` (2 blank lines above welcome box).
- **Surface Height**: Fixed 9 rows (`y = 2..10`).
- **Surface Width**: Full width minus 2 side margins (`x = 1..78` in `80×24`).
- **Border Treatment**: Single-line box drawing (`┌`, `─`, `┐`, `│`, `└`, `┘`) styled with `claude` orange (`#D77757`).
- **Integrated Title**: Positioned in top border row at `y = 2`, starting at column `x = 3`: `" Claude Code v2.1.226 "`.
- **Vertical Divider**: Positioned at column `x = 47` from `y = 3..9` using box character `│` styled with `subtle` gray (`#505050`).
- **Ambient Status Row**: Positioned at row `y = 19` in `80×24` (dynamically `prompt_y - 1`), right-aligned: `"● xhigh · /effort"`.
- **Prompt Composer**: Positioned at rows `y = 20..22` in `80×24`.
  - Top Border Rule: Row `y = 20` (`──────────────────────────────`).
  - Input Row: Row `y = 21`, prefix `❯ `, placeholder `"Ask anything or type / for commands..."`.
  - Bottom Border Rule: Row `y = 22` (`──────────────────────────────`).
- **Quiet Footer Row**: Single borderless line at bottom row `y = 23`: `" ▍▍ manual mode on · ? for shortcuts · ⬅ 3 agents"`.

#### [INFERENCE] Responsive Scaling Rules for Home
- **Width > 80**: Left welcome pane expands proportionally (`58%`); vertical divider shifts right; right rail remains pinned to right border.
- **Height > 24**: Welcome box stays anchored at `y = 2..10`; transcript canvas fills vertical gap; prompt composer stays pinned to bottom relative to footer (`y = height - 4..height - 2`).

---

### B. QUERY / CONVERSATION SCREEN CONTRACT

#### [REFERENCE FACT] Message Flow & Canvas Integration
- **Continuous Scroll Canvas**: The `HomeWelcomeSurface` stays at the head of the transcript buffer. Submitting a query does NOT unmount the welcome surface; it scrolls upward into history.
- **User Prompt Bubble**:
  - Background fill: `userMessageBackground` (`rgb(55,55,55)`).
  - Prefix: `❯ ` or `You:`.
  - Margin: Indented 2 spaces left/right.
- **Assistant Response Block**:
  - Renders directly on transparent terminal floor.
  - Prefix / Header: `Claude:` in `claude` orange (`rgb(215,119,87)`).
  - Markdown styling: Syntax-highlighted code blocks, bold text, bullet lists.
  - Timing & Token Byline: Rendered below response in `inactive` gray (`rgb(153,153,153)`): `* Crunched for 0s`.

---

### C. WORKSPACE DASHBOARD SCREEN CONTRACT

#### [REFERENCE FACT] Full-Width Task & Session Dashboard
- **Layout**: Full-width dashboard (no 2-column sidebar split line).
- **Header Block**:
  - Mascot logo: `▄▀▀` in `claude` orange (`#D77757`).
  - Title: `Claude Code v2.1.226` in bold white (`#FFFFFF`).
  - Context Line: `Opus 5 (1M context) · ~/Developer/PyCharm/brain` in `inactive` gray (`#999999`).
  - Status Summary Line: `4 awaiting input · 0 working · 17 completed` (Counts highlighted in `claude` orange, `subtle` gray, and `text` white).
- **Background Banner**:
  - Italic text: `"Your conversation moved to the background — enter opens it · esc returns to it · ctrl+c twice quits"`.
- **Needs Input Table Section**:
  - Header: `Needs input` in bold white.
  - Selected Row Highlight: Full-width row background using `selectionBg` (`rgb(38,79,120)`).
  - Prefix: `* ` in `claude` orange for active selection.
  - Columns: Session Name (left), Path/Repo (center), Age (right-aligned).
- **Completed Section**:
  - Header: `Completed` in bold white.
  - Idle Row: `· bg  (idle - send a prompt to start)  11h` in `inactive` gray.

---

### D. COMMAND PALETTE CONTRACT

#### [REFERENCE FACT] 3-Column Structured Category Layout
- **Trigger**: Activated by `/` in prompt input or `Ctrl+K` global shortcut.
- **Placement**: Floating dropdown anchored directly above prompt composer line.
- **Width & Height**: Width matches prompt box width (78 cols at `80×24`); max height 6 rows with scroll indicator.
- **3-Column Grid Formatting**:
  - Column 1: **Command Name** (width ~22 chars) formatted with `claude` orange or `suggestion` blue-purple in bold.
  - Column 2: **Category / Type** (width ~15 chars) formatted with `suggestion` blue-purple (`rgb(177,185,249)`): `command ·` or `skill ·`.
  - Column 3: **Description** (remainder) formatted with `inactive` gray (`rgb(153,153,153)`).
- **Selection Highlight**: Selected item row renders with `selectionBg` (`rgb(38,79,120)`) background across all 3 columns.
- **Key Navigation**:
  - `Up` / `Down` arrows move selection highlight.
  - `Enter` executes selected command.
  - `Esc` closes palette and restores prompt focus.

---

## 4. Implementation Mapping: Claude Component → Brain Component

| Claude Component (Reference) | Source File (`/Users/ritikpathania/Developer/src`) | Brain Component (Target) | Target File (`crates/brain-tui/src/`) |
|---|---|---|---|
| `LogoV2` & Welcome Box | `components/LogoV2/LogoV2.tsx` | `HomeWelcomeWidget` | `ui/widgets/home_welcome.rs` |
| `StatusLine` | `components/StatusLine.tsx` | `StatusFooterWidget` | `ui/status_footer.rs` |
| `PromptInput` | `components/PromptInput/PromptInput.tsx` | `PromptWidget` | `ui/widgets/prompt.rs` |
| `TaskListV2` | `components/TaskListV2.tsx` | `WorkspaceDashboardWidget` | `ui/widgets/workspace_dashboard.rs` |
| `QuickOpenDialog` / `GlobalSearch` | `components/QuickOpenDialog.tsx` | `CommandPaletteWidget` | `ui/widgets/palette.rs` |
| Ambient Status Row | Inline in `FullscreenLayout.tsx` | `AmbientStatusWidget` | `ui/widgets/ambient_status.rs` |

---

## 5. Decision & Verification Log

### [IMPLEMENTATION DECISION] Theme Token Migration
- Refactor `ThemeToken` in `crates/brain-tui/src/ui/theme/` to map 1:1 to Claude's RGB definitions:
  - `ThemeToken::HeaderPrimary` → `rgb(215,119,87)` (`#D77757`)
  - `ThemeToken::Accent` → `rgb(215,119,87)` (`#D77757`)
  - `ThemeToken::BorderSubtle` → `rgb(215,119,87)` (`#D77757`) for home border box; `rgb(80,80,80)` for line rules.
  - `ThemeToken::Selection` → `rgb(38,79,120)` (`#264F78`)
  - `ThemeToken::Suggestion` → `rgb(177,185,249)` (`#AFB9F9`)

### [IMPLEMENTATION DECISION] Pure State-Driven Interactivity
- All state transitions (`Home` → `Conversation` → `Workspace` → `Command Palette`) must be driven by `UiState` and `Action` reducers.
- Keyboard routes:
  - `/` opens command palette.
  - `Ctrl+K` toggles global palette.
  - `Esc` closes palette / returns to session.
  - `Enter` submits query or opens session.
