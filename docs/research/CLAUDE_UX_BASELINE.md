# Claude Code CLI — UX Baseline
> Research pass · Brain TUI Audit · 2026-08-10
> All claims labelled: **SOURCE-CONFIRMED** (code), **OBSERVED** (execution), or **INFERRED** (logical deduction)

---

## 1. Executive Summary

Claude Code is a production-quality Terminal User Interface built on **React + Ink + Yoga Layout** — the same component model as React Native, but targeting terminal character cells instead of pixels. Its strongest UX assets are:

- A mature, theme-aware design system with semantic color tokens and a shimmer animation framework
- Proportional, flex-driven responsive layout that gracefully degrades from 80→182+ columns without hardcoded breakpoints
- A rich slash-command ecosystem (60+ commands) with fuzzy completion
- A pluggable StatusLine driven by a user-configurable hook command
- A Clawd mascot logo that adapts between full (onboarding) and condensed (returning user) modes
- Live theme preview with per-token swatches

Its primary UX constraint is the terminal itself: no z-index, monospace-only typography, partial mouse support, and color-depth variability.

---

## 2. Environment

| Property | Value |
|---|---|
| Binary location | `/Users/ritikpathania/.local/bin/claude` |
| Version | `2.1.226 (Claude Code)` |
| Tech stack | React + Ink (terminal React) + Yoga Layout (flexbox) |
| Language | TypeScript / Bun |
| Source root | `/Users/ritikpathania/Developer/src` |
| Terminal automation | `osascript` available, `screencapture` available |

**[OBSERVED]** Claude confirmed via `claude --version` and `claude --help`.

---

## 3. Source Evidence

Key source files read for this baseline:

| File | Size | Purpose |
|---|---|---|
| `DESIGN.md` | 42 KB | Authoritative design system specification |
| `commands.ts` | 25 KB | Slash command registry |
| `components/StatusLine.tsx` | 49 KB | Footer status bar |
| `components/FullscreenLayout.tsx` | 84 KB | Primary layout container |
| `components/LogoV2/LogoV2.tsx` | 75 KB | Home screen / mascot |
| `utils/logoV2Utils.ts` | 9.8 KB | Home layout breakpoints |
| `components/PromptInput/PromptInput.tsx` | 355 KB | Prompt input (largest component) |
| `components/GlobalSearchDialog.tsx` | 44 KB | Ctrl+K global search |
| `components/ThemePicker.tsx` | 35 KB | Theme switcher dialog |
| `components/design-system/` | dir | 16 primitive design components |

---

## 4. Startup / Home Screen

### Layout Structure

**[SOURCE-CONFIRMED]** The root is an `<AlternateScreen>` that uses an isolated buffer. The primary layout is `FullscreenLayout` which vertically splits the terminal into:

1. **Scrollable region** — chat history / tool output (grows with `flexGrow: 1`)
2. **Pinned bottom region** — `PromptInput` + spinners + status bar (`flexShrink: 0`)

The logo/mascot (`LogoV2`) renders at the **top of the transcript**, not in a fixed-height slot. It is part of the scrollable message history.

### Logo / Mascot

**[SOURCE-CONFIRMED]** `LogoV2` has two modes:

| Condition | Mode | Component |
|---|---|---|
| New user OR has release notes | Full | `<Clawd>` + `<FeedColumn>` (recent activity, changelog) |
| Returning user, no new notes, no forced flag | Condensed | `<CondensedLogo>` |

**Full mode geometry** (from `logoV2Utils.ts`):

```typescript
const LEFT_PANEL_MAX_WIDTH = 50;          // Clawd art + welcome text
const MAX_LEFT_WIDTH = 50;                // Hard cap
const BORDER_PADDING = 4;                // Box border overhead
const DIVIDER_WIDTH = 1;                 // │ separator
const CONTENT_PADDING = 2;               // Inner whitespace
```

**[SOURCE-CONFIRMED]** Layout breakpoint:
```typescript
export function getLayoutMode(columns: number): LayoutMode {
  if (columns >= 70) return 'horizontal'   // Two-panel: Clawd left | Feed right
  return 'compact'                          // Single-panel, vertical stacking
}
```

At ≥70 columns: left panel (max 50 cols) + divider + right panel (min 30 cols).

**Right panel content** (from `feedConfigs.ts`): recent activity feed, "What's New" changelog, project onboarding steps, guest passes upsell.

### Prompt Vertical Position

**[INFERRED]** The prompt does NOT sit at a fixed proportional position on the Home screen. It lives below the transcript messages in the scrollable+bottom split. On a fresh session with no messages, the terminal shows:
- Top area: Logo / Clawd mascot
- Middle: Empty scrollable region (whatever remains)
- Bottom: Pinned prompt + status bar

The prompt is always at the physical bottom. Vertical breathing room comes from the empty scrollable region above it, not from explicit vertical anchoring logic.

---

## 5. Prompt Input

### Layout

**[SOURCE-CONFIRMED]** Key geometry constants in `PromptInput.tsx`:
- `PROMPT_FOOTER_LINES = 5` — reserved rows for footer/border/status
- `MIN_INPUT_VIEWPORT_LINES = 3` — minimum visible text rows

The input is always pinned to the bottom via `FullscreenLayout`.

### Border State Coding

**[SOURCE-CONFIRMED]**:
| State | Border Color |
|---|---|
| Default / unfocused | `promptBorder` (gray `rgb(136,136,136)`) |
| Focused | `claude` (brand orange `rgb(215,119,87)`) |
| Security / permission request | `permission` (blue-purple `rgb(177,185,249)`) |
| Error | `error` (red `rgb(255,107,128)`) |
| Shimmer animation | alternates base ↔ `*Shimmer` every ~80ms |

### Input Modes

**[SOURCE-CONFIRMED]**:
- **Standard mode**: Emacs-style line editing (default)
- **Vim mode**: Full `VimTextInput` component, toggled via `/vim` command or settings

### Keyboard

| Key | Action |
|---|---|
| `Enter` | Submit |
| `Shift+Enter` | Insert newline (multiline) |
| `Up` / `Down` | History previous / next |
| `Ctrl+C` | Cancel / interrupt |
| `Ctrl+D` | Exit (empty prompt) |
| `Esc` | Clear input / cancel |
| `Ctrl+_` / `Ctrl+Shift+-` | Undo |
| `Alt+V` (Win) / `Ctrl+V` (Mac/Linux) | Paste image |

### Features

- **Multiline input** with newline indicator
- **Command suggestions** in footer (typeahead fuzzy-match)
- **Queued commands** display when multiple are pending
- **Mode indicator** (Plan/Auto/Fast badge in footer)
- **Shimmer animation** on focused border

---

## 6. Slash Commands

**[SOURCE-CONFIRMED]** Full command registry from `commands.ts`. 60+ commands:

### Core Workflow
| Command | Purpose |
|---|---|
| `/help` | Help menu |
| `/clear` | Clear conversation |
| `/compact` | Compact conversation (summarize) |
| `/exit` | Exit Claude |
| `/context` | Show context window usage |
| `/cost` | Show session cost |
| `/stats` | Session statistics |
| `/status` | System status |

### Model & Settings
| Command | Purpose |
|---|---|
| `/model` | Switch model |
| `/theme` | Switch theme (with live preview) |
| `/config` | Configuration settings |
| `/vim` | Toggle Vim mode |
| `/effort` | Set effort level |
| `/fast` | Fast mode toggle |
| `/plan` | Plan mode toggle |
| `/outputStyle` | Output style picker |
| `/permissions` | Manage tool permissions |
| `/keybindings` | View/edit keybindings |
| `/hooks` | Configure hooks |
| `/statusline` | Configure status line |

### Memory & Sessions
| Command | Purpose |
|---|---|
| `/memory` | Manage memory |
| `/session` | Session management |
| `/resume` | Resume previous session |
| `/rewind` | Rewind conversation |
| `/export` | Export conversation |
| `/summary` | Summarize session |

### Discovery & Search
| Command | Purpose |
|---|---|
| `/review` | Code review |
| `/diff` | Show diff |
| `/files` | File browser |
| `/mcp` | MCP server management |
| `/agents` | Agent management |
| `/skills` | Skills management |

### Completion Mechanism

**[SOURCE-CONFIRMED]** Commands are aggregated in `getCommands()` (memoized). The prompt input uses `findSlashCommandPositions` to power a typeahead fuzzy-suggestion menu in the footer. Results include command name + description. Navigation: `Up`/`Down` arrows, `Enter` to select, `Esc` to cancel, `Tab` to confirm partial.

---

## 7. Global Discovery (Ctrl+K / Ctrl+Shift+F)

**[SOURCE-CONFIRMED]** Implemented in `GlobalSearchDialog.tsx`:

- **Engine**: Ripgrep (`rg`) via `ripGrepStream`
- **Limits**: `MAX_MATCHES_PER_FILE = 10`, `MAX_TOTAL_MATCHES = 500`
- **Debounce**: `DEBOUNCE_MS = 100`
- **Preview context**: `PREVIEW_CONTEXT_LINES = 4` lines above/below match

**Responsive preview placement**:
```typescript
const previewOnRight = columns >= 140;   // Wide: preview right
// Otherwise: preview below list
```

**[SOURCE-CONFIRMED]** Also via `QuickOpenDialog.tsx` (Ctrl+Shift+P):
- File fuzzy picker
- Preview panel moves to the right at `> 120` columns

---

## 8. Conversation / Workspace

### Message Layout

**[SOURCE-CONFIRMED]** `VirtualMessageList` manages a virtualized scroll buffer. Key features:

- **Unseen divider**: When the user scrolls up and new content arrives, Claude inserts an "N new messages" pill and tracks the unseen position. Clicking it invokes `scrollToBottom`.
- **Scroll shortcuts**: Vim-style (`Ctrl+U`/`Ctrl+D` half-page, `gg`/`G` top/bottom) and standard (`PageUp`/`PageDown`)
- **Message types**: User (card background), assistant (transparent), tool-use (pink bash border), system, memory

### Streaming

**[SOURCE-CONFIRMED]** During generation:
- Braille-dot spinner `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` at ~80ms per frame
- Shimmer alternation between `claude` / `claudeShimmer` on even/odd frames
- Rotating verb labels: "Thinking…", "Reading file…", "Writing code…" (from `spinnerVerbs.ts`)
- Token output renders inline as it arrives (streaming)
- `Ratchet` component prevents progress bars from going backwards

### Markdown Rendering

**[SOURCE-CONFIRMED]** `Markdown.tsx` handles:
- Bold, italic, strikethrough via terminal styling attributes
- Code blocks with syntax highlighting (`HighlightedCode`)
- Tables (`MarkdownTable`)
- Lists with indentation
- Blockquotes

### Interruption

**[SOURCE-CONFIRMED]** `Ctrl+C` triggers `app:interrupt`. Shows `<InterruptedByUser>` message inline. Streaming stops; partial response is kept in history.

---

## 9. Streaming / Progress

**[SOURCE-CONFIRMED]** Spinner component is monolithic (~88KB, ~1800 lines) handling:
- Braille-dot animation
- Tool-use progress (reading file, running bash, editing)
- Nested sub-agent status with 8 distinct identity colors
- Session cost display
- Compaction summary

---

## 10. Status Line

**[SOURCE-CONFIRMED]** Single-row footer, `space-between` flex layout:

| Segment | Position | Content | Token |
|---|---|---|---|
| Mode badge | Left | "Plan", "Auto", "Fast" | `planMode`, `fastMode` |
| Model name | Left-center | "Claude 4 Opus" | `claude` orange |
| Token usage | Center | Input/output token counts | `inactive` |
| Cost | Center-right | "$0.42" session cost | `warning` when high |
| Rate-limit bar | Right | Mini progress bar `█░` | `rate_limit_fill` |
| Key hints | Right | "Esc to cancel" | `subtle` |

**Visibility rules** [SOURCE-CONFIRMED]:
- Hidden when `kairosActive` (assistant/agent mode)
- Hidden when `settings.statusLine === undefined`
- Collapses gracefully at < 80 columns via `flexShrink`

**Architecture** [SOURCE-CONFIRMED]: The status line content is driven by a **user-configurable hook command** (`settings.statusLine.command`) — not a hardcoded view. The `StatusLineCommandInput` passes model, cost, token counts, rate limits, vim mode, workspace dir, etc. to the hook. The hook returns ANSI text. This means users can fully customize what appears in the footer.

**Debouncing**: 300ms debounce on `scheduleUpdate`. Updates only trigger when `lastAssistantMessageId`, `permissionMode`, `vimMode`, or `mainLoopModel` changes.

---

## 11. Themes

**[SOURCE-CONFIRMED]** 6 named themes + `auto`:

| Theme | Depth | Use |
|---|---|---|
| `dark` | 24-bit RGB | Default |
| `light` | 24-bit RGB | Light terminals |
| `dark-daltonized` | 24-bit RGB | Color-blind (greens→blues) |
| `light-daltonized` | 24-bit RGB | Color-blind on light |
| `dark-ansi` | ANSI-16 | No truecolor fallback |
| `light-ansi` | ANSI-16 | Light + no truecolor |
| `auto` | Varies | OSC 11 query + `$COLORFGBG` detection |

**Core palette** [SOURCE-CONFIRMED]:

| Token | Dark value | Purpose |
|---|---|---|
| `claude` | `rgb(215,119,87)` | Brand orange. Logo, focused borders, spinner |
| `claudeShimmer` | `rgb(235,159,127)` | Shimmer companion |
| `permission` | `rgb(177,185,249)` | Blue-purple. Security dialogs |
| `autoAccept` | `rgb(175,135,255)` | Violet. Auto-approve |
| `promptBorder` | `rgb(136,136,136)` | Default input border |
| `text` | `rgb(255,255,255)` | Primary text |
| `inactive` | `rgb(153,153,153)` | Muted / secondary |
| `subtle` | `rgb(80,80,80)` | Dividers |
| `success` | `rgb(78,186,101)` | Green |
| `error` | `rgb(255,107,128)` | Red |
| `warning` | `rgb(255,193,7)` | Amber |
| `selectionBg` | `rgb(38,79,120)` | Picker selection |
| `userMessageBackground` | `rgb(55,55,55)` | User message cards |

**ThemePicker** [SOURCE-CONFIRMED]: Dialog with live preview — hovering an option calls `setPreviewTheme()` instantly; `Esc` calls `cancelPreview()` to restore original.

---

## 12. Keyboard Shortcuts

**[SOURCE-CONFIRMED]** From `defaultBindings.ts` and component event handlers:

| Shortcut | Action | Context |
|---|---|---|
| `Ctrl+C` | Interrupt / cancel | Global |
| `Ctrl+D` | Exit (empty input) | Global |
| `Ctrl+L` | Redraw terminal | Global |
| `Ctrl+Shift+F` / `Cmd+Shift+F` | Global file search | Global |
| `Ctrl+Shift+P` / `Cmd+Shift+P` | Quick file open | Global |
| `Enter` | Submit | Input |
| `Shift+Enter` | Newline | Input |
| `Up` / `Down` | History prev/next | Input |
| `Ctrl+_` | Undo | Input |
| `Esc` | Cancel / dismiss | Input / dialogs |
| `Shift+Tab` / `Meta+M` | Cycle mode | Input |
| `Meta+P` | Model picker | Input |
| `PageUp` / `PageDown` | Scroll page | Conversation |
| `Ctrl+U` / `Ctrl+D` | Scroll half-page | Conversation |
| `Ctrl+Home` / `Ctrl+End` | Scroll top/bottom | Conversation |
| `Ctrl+Shift+C` / `Cmd+C` | Copy selection | Conversation |
| `Up`/`Down`/`j`/`k` | Navigate | Pickers |
| `Enter` | Accept | Pickers |
| `Esc` | Cancel | Pickers |
| `Tab` | Confirm partial | Slash completion |

---

## 13. Terminal Geometry

**[SOURCE-CONFIRMED]** Responsive breakpoints from DESIGN.md and component sources:

| Columns | Name | Key Adaptations |
|---|---|---|
| < 70 | Compact | LogoV2 switches to vertical/condensed |
| < 80 | Very narrow | StatusLine shows critical info only; dialogs full-width |
| 80–120 | Standard | Default layout; dialogs centered; unified diffs |
| > 100 | Medium-wide | `HistorySearchDialog` preview moves to right |
| > 120 | Wide | `QuickOpenDialog` preview right; sidebar possible |
| > 140 | Extra-wide | `GlobalSearchDialog` preview right |

**Resize handling**: Ink re-renders on `SIGWINCH`. Flexbox recalculates automatically. No pixel math — all character-cell based.

---

## 14. Responsive Behavior

**[SOURCE-CONFIRMED]** Key patterns:
- `flexGrow: 1` / `flexShrink: 1` — elements expand/contract naturally
- `percentages` for major container widths (e.g. `'50%'`)
- Hard-coded minimum widths only for absolute necessities (min 20 chars for mascot art, min 30 chars for right panel)
- `truncate-middle` wrap mode for long file paths in StatusLine
- Status bar gracefully collapses segments at < 80 cols

---

## 15. Error States

**[SOURCE-CONFIRMED]** Error presentation:
- Input validation errors: text below the input field (no visual state change on border itself — noted as technical debt in DESIGN.md)
- API errors: inline in the conversation as an error message
- Tool errors: `FallbackToolUseErrorMessage` renders with `error` color border
- Rate limiting: `warning` spinner color + rate limit progress bar in status
- Permission denial: `FallbackToolUseRejectedMessage`

---

## 16. Empty States

**[INFERRED]** Fresh session:
- Full LogoV2 with Clawd mascot + feed columns (recent activity, changelog)
- Or `CondensedLogo` for returning users
- Prompt shows placeholder hint text

---

## 17. Accessibility / Contrast

**[SOURCE-CONFIRMED]**:
- `--ax-screen-reader` flag renders flat text, no decorative borders
- Daltonized themes replace greens/pinks with blue variants for color-blind users
- ANSI-16 fallback themes for non-truecolor terminals
- All interactions keyboard-accessible (mouse is supplementary)

---

## 18. Interaction State Machine

```
STARTUP
  │
  ├── has release notes or onboarding → FULL_LOGO
  └── returning user → CONDENSED_LOGO
        │
        ▼
    PROMPT (Home)
        │
        ├── "/" ──────────────────────► SLASH_PICKER
        │                                    │
        │                         ┌──────────┴──────────┐
        │                         │                     │
        │                       Enter                  Esc
        │                         │                     │
        │                      COMMAND               PROMPT
        │
        ├── Ctrl+Shift+F / Cmd+Shift+F ► GLOBAL_SEARCH
        │                                    │ Enter → opens file
        │                                    │ Esc → PROMPT
        │
        ├── Ctrl+Shift+P ──────────────► QUICK_OPEN
        │                                    │ same as above
        │
        ├── Up/Down ──────────────────► HISTORY_NAVIGATE
        │
        └── Enter (non-empty) ─────────► STREAMING
              │
              ├── Ctrl+C → INTERRUPTED (response kept)
              ├── Error → ERROR_STATE (inline message)
              └── Complete → PROMPT (Workspace mode)
                                │
                                ├── "/theme" → THEME_PICKER_DIALOG
                                │                 │ hover → LIVE_PREVIEW
                                │                 │ Enter → APPLY_THEME
                                │                 │ Esc → REVERT → PROMPT
                                │
                                ├── "/model" → MODEL_PICKER_DIALOG
                                │
                                └── "/compact" → COMPACT_SUMMARY
```

---

## 19. Reusable UX Patterns

1. **Shimmer animation pair** — every accent color has a lighter `*Shimmer` companion; 80ms alternation creates breathing effect without full re-render
2. **Pluggable status line** — hook-driven footer means the product team can iterate footer content without changing TUI code
3. **Live theme preview** — hover instantly applies, Esc reverts
4. **Unseen divider** — "N new messages" pill when scrolled up during streaming
5. **Ratchet progress** — prevents backwards progress bar movement from async updates
6. **FuzzyPicker primitive** — reusable fuzzy search component with `suggestion`-colored match highlights
7. **LogoV2 condensed mode** — intelligent degradation to compact logo for returning users
8. **Braille spinner with verb rotation** — "Thinking…" / "Reading file…" verb labels alongside animated spinner
9. **Subagent identity colors** — 8 distinct colors for concurrent sub-agent visual identification
10. **Column breakpoints via flexbox** — no media queries; `flexGrow`/`flexShrink` does the work

---

## 20. Claude-Specific Patterns

These exist for Claude's specific product requirements and should NOT be automatically copied:

1. **Rate limit progress bar** — tied to Claude's 5-hour / 7-day usage limits (Brain has different billing)
2. **Permission mode UI** — trust dialog, YOLO mode, sandboxed approval flow (different auth model)
3. **Subagent identity colors** — for Claude's multi-agent orchestration (Brain does single-session)
4. **Rainbow ultrathink highlighting** — Claude's extended thinking keyword animation
5. **Plugin/workflow/skill system** — Claude's extensibility model (Brain has different plugin architecture)
6. **Worktree session tracking** — Claude's git-worktree-aware multi-session model
7. **LogoV2 feed columns** — "What's New" changelog driven by Claude's release notes system
8. **KAIROS / assistant mode** — Claude's daemon-suppressed coordinator mode
9. **Status line as hook command** — Claude's hook system is user-scriptable; Brain's architecture is different

---

## 21. Evidence Matrix

| Claim | Source | Confidence |
|---|---|---|
| React + Ink architecture | DESIGN.md L284 | SOURCE-CONFIRMED |
| Brand orange `rgb(215,119,87)` | DESIGN.md L33-34 | SOURCE-CONFIRMED |
| 6 themes + auto | DESIGN.md L294, L360-368 | SOURCE-CONFIRMED |
| LogoV2 condensed at < onboarding threshold | LogoV2.tsx L118-123 | SOURCE-CONFIRMED |
| Horizontal layout at ≥ 70 cols | logoV2Utils.ts L35-38 | SOURCE-CONFIRMED |
| Left panel max 50 cols | logoV2Utils.ts L18 | SOURCE-CONFIRMED |
| Prompt 5 footer lines | PromptInput.tsx constant | SOURCE-CONFIRMED |
| StatusLine debounce 300ms | StatusLine.tsx L230-234 | SOURCE-CONFIRMED |
| StatusLine hook-driven | StatusLine.tsx L210 `executeStatusLineCommand` | SOURCE-CONFIRMED |
| Global search ripgrep max 500 | GlobalSearchDialog.tsx | SOURCE-CONFIRMED |
| Preview right at ≥ 140 cols | GlobalSearchDialog.tsx `previewOnRight` | SOURCE-CONFIRMED |
| Spinner 80ms / braille frames | DESIGN.md L488-511 | SOURCE-CONFIRMED |
| ThemePicker live preview | DESIGN.md L370-371 | SOURCE-CONFIRMED |
| Ratchet prevents backwards progress | DESIGN.md L300, 664-667 | SOURCE-CONFIRMED |
| Claude binary at `~/.local/bin/claude` | terminal execution | OBSERVED |
| Version 2.1.226 | `claude --version` output | OBSERVED |
| osascript works | agent execution | OBSERVED |
| Brain debug+release binary exists | agent execution | OBSERVED |
