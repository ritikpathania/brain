---
version: beta
name: Claude-Code-CLI-Design-System
description: >
  Design system specification for the Claude Code CLI Terminal User Interface
  (TUI). The system renders rich interactive interfaces inside standard terminal
  emulators using React, Ink, and Yoga Layout. Color output supports truecolor
  (24-bit RGB), 256-color, and ANSI-16 fallback. Six named themes ship
  out-of-the-box: dark, light, dark-daltonized, light-daltonized, dark-ansi,
  and light-ansi. Brand voltage comes from a warm orange ("claude" token) paired
  with a blue-purple permission accent — deliberately warm and humanist where
  most AI CLI tools use plain white-on-black. Typography hierarchy is achieved
  through bold/dim/italic/underline/inverse rather than font sizes, since all
  terminal output is monospace. The system favours Unicode box-drawing
  characters, Braille-dot spinners, and block-fill progress bars for visual
  richness within terminal constraints.

rounded:
  sm: 0px
  md: 0px
  lg: 0px
  full: 0px

spacing:
  none: 0ch
  tight: 1ch
  normal: 2ch
  relaxed: 3ch
  section: 4ch

colors:
  # ── Brand & Accent ──────────────────────────────────────────────
  primary:                             "rgb(215,119,87)"   # Brand orange (alias of claude).
  claude:                              "rgb(215,119,87)"   # Brand orange. Headers, logo, focused borders.
  claudeShimmer:                       "rgb(235,159,127)"  # Lighter orange for animated shimmer on spinners.
  claudeBlue_FOR_SYSTEM_SPINNER:       "rgb(147,165,255)"  # Blue spinner for system-level operations.
  claudeBlueShimmer_FOR_SYSTEM_SPINNER: "rgb(177,195,255)" # Shimmer variant of system blue.
  autoAccept:                          "rgb(175,135,255)"  # Electric violet. Auto-approve / merged states.
  bashBorder:                          "rgb(253,93,177)"   # Bright pink. Shell command borders.
  permission:                          "rgb(177,185,249)"  # Blue-purple. Permission request highlights.
  permissionShimmer:                   "rgb(207,215,255)"  # Lighter blue-purple shimmer.
  planMode:                            "rgb(72,150,140)"   # Sage teal. Plan mode indicators.
  ide:                                 "rgb(71,130,200)"   # Muted blue. IDE integration indicators.
  fastMode:                            "rgb(255,120,20)"   # Electric orange. Fast execution mode.
  fastModeShimmer:                     "rgb(255,165,70)"   # Lighter orange shimmer for fast mode.

  # ── Chrome & UI ─────────────────────────────────────────────────
  promptBorder:                        "rgb(136,136,136)"  # Medium gray. Default input prompt border.
  promptBorderShimmer:                 "rgb(166,166,166)"  # Lighter gray shimmer for prompt border.
  text:                                "rgb(255,255,255)"  # Primary text (white on dark).
  inverseText:                         "rgb(0,0,0)"        # Inverted text for selections.
  inactive:                            "rgb(153,153,153)"  # Muted gray for dimmed content.
  inactiveShimmer:                     "rgb(193,193,193)"  # Lighter gray shimmer.
  subtle:                              "rgb(80,80,80)"     # Dark gray for dividers & low-contrast chrome.
  suggestion:                          "rgb(177,185,249)"  # Blue-purple. Search match highlights, hints.
  remember:                            "rgb(177,185,249)"  # Blue-purple. Memory system markers.
  background:                          "rgb(0,204,204)"    # Bright cyan. Rarely used standalone.
  merged:                              "rgb(175,135,255)"  # Violet (same as autoAccept). Merged PRs.
  chromeYellow:                        "rgb(251,188,4)"    # Chrome/Google yellow for brand refs.
  professionalBlue:                    "rgb(106,155,204)"  # Grove feature blue.

  # ── Semantic ────────────────────────────────────────────────────
  success:                             "rgb(78,186,101)"   # Green. Completed tasks, resolved files.
  error:                               "rgb(255,107,128)"  # Bright red. Errors, rejections, failures.
  warning:                             "rgb(255,193,7)"    # Amber. Rate-limit alerts, caution states.
  warningShimmer:                      "rgb(255,223,57)"   # Lighter amber shimmer.

  # ── Diff ────────────────────────────────────────────────────────
  diffAdded:                           "rgb(34,92,43)"     # Dark green bg for added lines.
  diffRemoved:                         "rgb(122,41,54)"    # Dark red bg for removed lines.
  diffAddedDimmed:                     "rgb(71,88,74)"     # Very dark green for subtle additions.
  diffRemovedDimmed:                   "rgb(105,72,77)"    # Very dark red for subtle removals.
  diffAddedWord:                       "rgb(56,166,96)"    # Medium green for word-level add highlight.
  diffRemovedWord:                     "rgb(179,89,107)"   # Medium red for word-level remove highlight.

  # ── Surfaces / Backgrounds ─────────────────────────────────────
  userMessageBackground:               "rgb(55,55,55)"     # Container bg for user messages.
  userMessageBackgroundHover:          "rgb(70,70,70)"     # Hover variant.
  messageActionsBackground:            "rgb(44,50,62)"     # Cool gray bg for action menus.
  selectionBg:                         "rgb(38,79,120)"    # Selection blue for pickers / focused items.
  bashMessageBackgroundColor:          "rgb(65,60,65)"     # Shell output background.
  memoryBackgroundColor:               "rgb(55,65,70)"     # Memory context card bg.
  clawd_body:                          "rgb(215,119,87)"   # Mascot body color (matches claude).
  clawd_background:                    "rgb(0,0,0)"        # Mascot background.

  # ── Rate Limit ──────────────────────────────────────────────────
  rate_limit_fill:                     "rgb(177,185,249)"  # Filled portion of rate-limit bar.
  rate_limit_empty:                    "rgb(80,83,112)"    # Empty portion of rate-limit bar.

  # ── Brief / Assistant Mode Labels ──────────────────────────────
  briefLabelYou:                       "rgb(122,180,232)"  # Blue label for "You:" prefix.
  briefLabelClaude:                    "rgb(215,119,87)"   # Orange label for "Claude:" prefix.

  # ── Subagent Identity Colors ────────────────────────────────────
  red_FOR_SUBAGENTS_ONLY:              "rgb(220,38,38)"
  blue_FOR_SUBAGENTS_ONLY:             "rgb(37,99,235)"
  green_FOR_SUBAGENTS_ONLY:            "rgb(22,163,74)"
  yellow_FOR_SUBAGENTS_ONLY:           "rgb(202,138,4)"
  purple_FOR_SUBAGENTS_ONLY:           "rgb(147,51,234)"
  orange_FOR_SUBAGENTS_ONLY:           "rgb(234,88,12)"
  pink_FOR_SUBAGENTS_ONLY:             "rgb(219,39,119)"
  cyan_FOR_SUBAGENTS_ONLY:             "rgb(8,145,178)"

  # ── Rainbow (Ultrathink Keyword Highlighting) ──────────────────
  rainbow_red:                         "rgb(235,95,87)"
  rainbow_orange:                      "rgb(245,139,87)"
  rainbow_yellow:                      "rgb(250,195,95)"
  rainbow_green:                       "rgb(145,200,130)"
  rainbow_blue:                        "rgb(130,170,220)"
  rainbow_indigo:                      "rgb(155,130,200)"
  rainbow_violet:                      "rgb(200,130,180)"
  rainbow_red_shimmer:                 "rgb(250,155,147)"
  rainbow_orange_shimmer:              "rgb(255,185,137)"
  rainbow_yellow_shimmer:              "rgb(255,225,155)"
  rainbow_green_shimmer:               "rgb(185,230,180)"
  rainbow_blue_shimmer:                "rgb(180,205,240)"
  rainbow_indigo_shimmer:              "rgb(195,180,230)"
  rainbow_violet_shimmer:              "rgb(230,180,210)"

typography:
  headline:
    fontFamily: Monospace
    fontSize: 12px
    fontWeight: 700
  body:
    fontFamily: Monospace
    fontSize: 12px
    fontWeight: 400
  label:
    fontFamily: Monospace
    fontSize: 12px
    fontWeight: 500

components:
  themed-text:
    textColor: "{colors.text}"
    typography: "{typography.body}"
  themed-text-inactive:
    textColor: "{colors.inactive}"
    typography: "{typography.body}"
  themed-text-inverse:
    textColor: "{colors.inverseText}"
    typography: "{typography.body}"
  dialog:
    backgroundColor: "{colors.userMessageBackground}"
    textColor: "{colors.text}"
    typography: "{typography.body}"
  dialog-hover:
    backgroundColor: "{colors.userMessageBackgroundHover}"
  dialog-border:
    textColor: "{colors.promptBorder}"
  dialog-border-shimmer:
    textColor: "{colors.promptBorderShimmer}"
  divider:
    textColor: "{colors.subtle}"
  divider-active:
    textColor: "{colors.claude}"
  divider-active-shimmer:
    textColor: "{colors.claudeShimmer}"
  status-icon-success:
    textColor: "{colors.success}"
  status-icon-warning:
    textColor: "{colors.warning}"
  status-icon-warning-shimmer:
    textColor: "{colors.warningShimmer}"
  status-icon-error:
    textColor: "{colors.error}"
  status-icon-info:
    textColor: "{colors.suggestion}"
  status-icon-pending:
    textColor: "{colors.inactiveShimmer}"
  status-icon-system:
    textColor: "{colors.claudeBlue_FOR_SYSTEM_SPINNER}"
  status-icon-system-shimmer:
    textColor: "{colors.claudeBlueShimmer_FOR_SYSTEM_SPINNER}"
  auto-accept:
    textColor: "{colors.autoAccept}"
  bash-border:
    textColor: "{colors.bashBorder}"
  permission-badge:
    textColor: "{colors.permission}"
  permission-badge-shimmer:
    textColor: "{colors.permissionShimmer}"
  plan-mode-badge:
    textColor: "{colors.planMode}"
  ide-badge:
    textColor: "{colors.ide}"
  fast-mode-badge:
    textColor: "{colors.fastMode}"
  fast-mode-badge-shimmer:
    textColor: "{colors.fastModeShimmer}"
  remember-badge:
    textColor: "{colors.remember}"
  background-box:
    backgroundColor: "{colors.background}"
  merged-badge:
    textColor: "{colors.merged}"
  chrome-yellow-ref:
    textColor: "{colors.chromeYellow}"
  professional-blue-ref:
    textColor: "{colors.professionalBlue}"
  diff-added:
    backgroundColor: "{colors.diffAdded}"
  diff-removed:
    backgroundColor: "{colors.diffRemoved}"
  diff-added-dimmed:
    backgroundColor: "{colors.diffAddedDimmed}"
  diff-removed-dimmed:
    backgroundColor: "{colors.diffRemovedDimmed}"
  diff-added-word:
    textColor: "{colors.diffAddedWord}"
  diff-removed-word:
    textColor: "{colors.diffRemovedWord}"
  action-menu:
    backgroundColor: "{colors.messageActionsBackground}"
  selection-box:
    backgroundColor: "{colors.selectionBg}"
  bash-message:
    backgroundColor: "{colors.bashMessageBackgroundColor}"
  memory-card:
    backgroundColor: "{colors.memoryBackgroundColor}"
  clawd-body:
    textColor: "{colors.clawd_body}"
  clawd-background:
    backgroundColor: "{colors.clawd_background}"
  rate-limit-fill:
    backgroundColor: "{colors.rate_limit_fill}"
  rate-limit-empty:
    backgroundColor: "{colors.rate_limit_empty}"
  brief-you:
    textColor: "{colors.briefLabelYou}"
  brief-claude:
    textColor: "{colors.briefLabelClaude}"
  subagent-1:
    textColor: "{colors.red_FOR_SUBAGENTS_ONLY}"
  subagent-2:
    textColor: "{colors.blue_FOR_SUBAGENTS_ONLY}"
  subagent-3:
    textColor: "{colors.green_FOR_SUBAGENTS_ONLY}"
  subagent-4:
    textColor: "{colors.yellow_FOR_SUBAGENTS_ONLY}"
  subagent-5:
    textColor: "{colors.purple_FOR_SUBAGENTS_ONLY}"
  subagent-6:
    textColor: "{colors.orange_FOR_SUBAGENTS_ONLY}"
  subagent-7:
    textColor: "{colors.pink_FOR_SUBAGENTS_ONLY}"
  subagent-8:
    textColor: "{colors.cyan_FOR_SUBAGENTS_ONLY}"
  rainbow-1:
    textColor: "{colors.rainbow_red}"
  rainbow-2:
    textColor: "{colors.rainbow_orange}"
  rainbow-3:
    textColor: "{colors.rainbow_yellow}"
  rainbow-4:
    textColor: "{colors.rainbow_green}"
  rainbow-5:
    textColor: "{colors.rainbow_blue}"
  rainbow-6:
    textColor: "{colors.rainbow_indigo}"
  rainbow-7:
    textColor: "{colors.rainbow_violet}"
  rainbow-1-shimmer:
    textColor: "{colors.rainbow_red_shimmer}"
  rainbow-2-shimmer:
    textColor: "{colors.rainbow_orange_shimmer}"
  rainbow-3-shimmer:
    textColor: "{colors.rainbow_yellow_shimmer}"
  rainbow-4-shimmer:
    textColor: "{colors.rainbow_green_shimmer}"
  rainbow-5-shimmer:
    textColor: "{colors.rainbow_blue_shimmer}"
  rainbow-6-shimmer:
    textColor: "{colors.rainbow_indigo_shimmer}"
  rainbow-7-shimmer:
    textColor: "{colors.rainbow_violet_shimmer}"
  primary-brand-ref:
    textColor: "{colors.primary}"
---

## Overview

Claude Code CLI is a terminal-based engineering assistant that provides a stateful, agentic interaction model directly in the user's terminal. The TUI runs entirely within terminal constraints using **React + Ink** (React for terminal) and **Yoga Layout** (flexbox for character cells).

The brand identity is warm and humanist — a deliberate contrast to the cool white-on-black aesthetic of most CLI tools. The signature `claude` token (`rgb(215,119,87)` — warm orange) appears on the logo, focused input borders, spinner animations, and active tab indicators. The `permission` token (`rgb(177,185,249)` — blue-purple) marks security-sensitive interactions. Together, the orange/blue-purple pairing gives the TUI a distinctive personality within the terminal.

The system supports three surface contexts:
1. **Default terminal background** — Text renders directly over the user's terminal background (transparent floor).
2. **Card surfaces** — `userMessageBackground`, `bashMessageBackgroundColor`, `memoryBackgroundColor` provide subtle container backgrounds for message types.
3. **Selection surfaces** — `selectionBg`, `messageActionsBackground` highlight focused or interactive elements.

### Key Characteristics
- **Six themes** (dark, light, dark-daltonized, light-daltonized, dark-ansi, light-ansi) plus auto-detection.
- **Shimmer animation system** — Most accent colors have a `*Shimmer` companion token. Spinners alternate between base and shimmer colors on even/odd frames to create a pulsing "breathing" effect.
- **Subagent identity colors** — Eight dedicated color tokens (`red_FOR_SUBAGENTS_ONLY` through `cyan_FOR_SUBAGENTS_ONLY`) are assigned to concurrent sub-agents so each has a visually distinct identity.
- **Rainbow highlighting** — Seven-color rainbow spectrum (with shimmer variants) for ultrathink keyword highlighting.
- **Box-drawing characters** — Dividers use `─`, borders use `┌─┐│└─┘`, and spinners use Braille dots (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`).
- **Block-fill progress** — `█` (filled) and `░` (empty) with `Ratchet` to prevent backwards movement.

### Output Styles

Output styles modify Claude's communication behavior without changing its coding capabilities. Three styles are available:

#### Default
No prompt modification. Standard Claude Code behavior.

#### Explanatory
Claude explains implementation choices and codebase patterns. Adds **Insight blocks**:
```
★ Insight ─────────────────────────────────────
[2–3 key educational points about the code]
─────────────────────────────────────────────────
```
`keepCodingInstructions: true` — coding abilities are unaltered.

#### Learning
Claude pauses and asks the user to write small code pieces for hands-on practice. Uses **Learn by Doing** blocks:
```
● Learn by Doing
Context: [what's built and why this decision matters]
Your Task: [specific function/section in file]
Guidance: [trade-offs and constraints to consider]
```
Inserts `TODO(human)` markers into the codebase and waits for the user to implement. After contributions, shares one connecting insight.

#### Custom Styles
Additional output styles can be defined via:
- **User settings** (`~/.claude/settings`)
- **Project settings** (`.claude/settings.json`)
- **Managed/policy settings** (organization-level)
- **Plugins** (can set `forceForPlugin: true` for auto-activation)

Priority order (lowest → highest): built-in → plugin → user → project → managed.

### Known Gaps & Constraints

- **Terminal font is user-controlled.** Custom Nerd Font glyphs, ligatures, or font weights are dependent on the user's terminal emulator. The TUI cannot set or detect the font. Glyphs must degrade gracefully.
- **No true z-index stacking.** Ink draws components sequentially. Modals work by rendering at the top of the component tree and capturing keyboard focus — they do not "float" over other content in the DOM sense.
- **Mouse support is partial.** Click support exists in some terminal emulators (iTerm2, Kitty, Windows Terminal) but is not universal. All interactions must be keyboard-accessible as primary.
- **Color depth detection is imperfect.** The TUI checks `$COLORTERM` and terminal capabilities, but some terminals misreport. The ANSI-16 fallback themes exist to handle worst-case scenarios.
- **Spinner component is monolithic.** The `Spinner.tsx` file (~88KB, ~1800 lines) handles multiple responsibilities (animation, tool progress, agent status, cost). This is identified technical debt.
- **Animation timings are not tokenized.** The ~80ms spinner frame interval and shimmer alternation are hardcoded, not exposed as configurable design tokens.
- **Form validation states are limited.** The TUI's input validation is primarily text-based (error messages below inputs) rather than visual state changes on the input itself.

---

## Colors

### Theme Architecture

Colors are resolved at render time through a `ThemeProvider` React context. Every `<ThemedText>` and `<ThemedBox>` component accepts either:
- A **theme key** (e.g., `"claude"`, `"error"`) → resolved via `getTheme(currentTheme)[key]`
- A **raw color** (e.g., `"rgb(255,0,0)"`, `"#ff0000"`, `"ansi:red"`) → passed through directly

The `color()` utility in `components/design-system/color.ts` performs this resolution. It checks if the value starts with `rgb(`, `#`, `ansi256(`, or `ansi:` to distinguish raw values from theme keys.

### Theme Modes

| Theme | Color Depth | Use Case |
|---|---|---|
| `dark` | 24-bit RGB | Default. Vibrant colors on dark backgrounds. |
| `light` | 24-bit RGB | High-contrast for light terminal backgrounds. |
| `dark-daltonized` | 24-bit RGB | Color-blind friendly. Greens → blues, pinks → blues. |
| `light-daltonized` | 24-bit RGB | Color-blind friendly on light backgrounds. |
| `dark-ansi` | 16 ANSI colors | Fallback for terminals without truecolor (e.g., older TTYs). |
| `light-ansi` | 16 ANSI colors | Fallback for light-background terminals without truecolor. |
| `auto` | Varies | Detects terminal dark/light mode via OSC 11 query + `$COLORFGBG`. |

The `ThemePicker` dialog shows all themes with **live preview** — hovering a theme option instantly applies it to the entire TUI via `setPreviewTheme()`. Dismissing without selecting restores the original. Each option displays a **color swatch row** (small `█` blocks) showing the theme's key colors.

### Brand & Accent Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `claude` | `rgb(215,119,87)` | `rgb(215,119,87)` | Brand orange. Logo, focused borders, active tabs, spinner default. |
| `claudeShimmer` | `rgb(235,159,127)` | `rgb(245,149,117)` | Lighter orange for shimmer breathing effect. |
| `autoAccept` | `rgb(175,135,255)` | `rgb(135,0,255)` | Electric violet. Auto-approve and merged states. |
| `bashBorder` | `rgb(253,93,177)` | `rgb(255,0,135)` | Bright pink. Border color for shell command execution blocks. |
| `permission` | `rgb(177,185,249)` | `rgb(87,105,247)` | Blue-purple. Security permission prompts. |
| `permissionShimmer` | `rgb(207,215,255)` | `rgb(137,155,255)` | Lighter blue-purple shimmer. |
| `planMode` | `rgb(72,150,140)` | `rgb(0,102,102)` | Sage teal. Active planning mode indicator. |
| `ide` | `rgb(71,130,200)` | `rgb(71,130,200)` | Muted blue. IDE bridge connection indicator. |
| `fastMode` | `rgb(255,120,20)` | `rgb(255,106,0)` | Electric orange. Fast execution mode. |
| `fastModeShimmer` | `rgb(255,165,70)` | `rgb(255,150,50)` | Lighter orange shimmer. |

### Chrome & UI Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `promptBorder` | `rgb(136,136,136)` | `rgb(153,153,153)` | Default input prompt border. |
| `promptBorderShimmer` | `rgb(166,166,166)` | `rgb(183,183,183)` | Prompt border shimmer. |
| `text` | `rgb(255,255,255)` | `rgb(0,0,0)` | Primary text. |
| `inverseText` | `rgb(0,0,0)` | `rgb(255,255,255)` | Text on inverted/selected backgrounds. |
| `inactive` | `rgb(153,153,153)` | `rgb(102,102,102)` | Dimmed content, secondary metadata. |
| `inactiveShimmer` | `rgb(193,193,193)` | `rgb(142,142,142)` | Lighter gray shimmer. |
| `subtle` | `rgb(80,80,80)` | `rgb(175,175,175)` | Divider lines, very low-contrast chrome. |
| `suggestion` | `rgb(177,185,249)` | `rgb(87,105,247)` | Search match highlights, auto-complete hints. |
| `remember` | `rgb(177,185,249)` | `rgb(0,0,255)` | Memory system markers. |
| `background` | `rgb(0,204,204)` | `rgb(0,153,153)` | Bright cyan. Rarely used standalone. |
| `merged` | `rgb(175,135,255)` | `rgb(135,0,255)` | Violet. Merged PR indicator (matches `autoAccept`). |
| `chromeYellow` | `rgb(251,188,4)` | `rgb(251,188,4)` | Chrome/Google brand yellow. |
| `professionalBlue` | `rgb(106,155,204)` | `rgb(106,155,204)` | Grove feature blue. |

### Semantic Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `success` | `rgb(78,186,101)` | `rgb(44,122,57)` | Green. Completed tasks, resolved files, `✔` icons. |
| `error` | `rgb(255,107,128)` | `rgb(171,43,63)` | Red. Errors, rejections, `✖` icons. |
| `warning` | `rgb(255,193,7)` | `rgb(150,108,30)` | Amber. Rate-limit alerts, caution states. |
| `warningShimmer` | `rgb(255,223,57)` | `rgb(200,158,80)` | Lighter amber shimmer. |

### Diff Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `diffAdded` | `rgb(34,92,43)` | `rgb(105,219,124)` | Background for added lines (`+`). |
| `diffRemoved` | `rgb(122,41,54)` | `rgb(255,168,180)` | Background for removed lines (`-`). |
| `diffAddedDimmed` | `rgb(71,88,74)` | `rgb(199,225,203)` | Subtle added-line background (context). |
| `diffRemovedDimmed` | `rgb(105,72,77)` | `rgb(253,210,216)` | Subtle removed-line background (context). |
| `diffAddedWord` | `rgb(56,166,96)` | `rgb(47,157,68)` | Word-level addition highlight. |
| `diffRemovedWord` | `rgb(179,89,107)` | `rgb(209,69,75)` | Word-level removal highlight. |

### Surface / Background Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `userMessageBackground` | `rgb(55,55,55)` | `rgb(240,240,240)` | Container background for user input messages. |
| `userMessageBackgroundHover` | `rgb(70,70,70)` | `rgb(252,252,252)` | Hover state. |
| `messageActionsBackground` | `rgb(44,50,62)` | `rgb(232,236,244)` | Cool gray for action menus. |
| `selectionBg` | `rgb(38,79,120)` | `rgb(180,213,255)` | Selection highlight in pickers and text selection. |
| `bashMessageBackgroundColor` | `rgb(65,60,65)` | `rgb(250,245,250)` | Shell output container background. |
| `memoryBackgroundColor` | `rgb(55,65,70)` | `rgb(230,245,250)` | Memory context card background. |
| `clawd_body` | `rgb(215,119,87)` | `rgb(215,119,87)` | Mascot body color. |
| `clawd_background` | `rgb(0,0,0)` | `rgb(0,0,0)` | Mascot background. |

### Rate Limit Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `rate_limit_fill` | `rgb(177,185,249)` | `rgb(87,105,247)` | Filled portion of the rate-limit mini bar. |
| `rate_limit_empty` | `rgb(80,83,112)` | `rgb(39,47,111)` | Empty portion of the rate-limit mini bar. |

### Brief Mode Label Tokens

| Token | Dark Value | Light Value | Purpose |
|---|---|---|---|
| `briefLabelYou` | `rgb(122,180,232)` | `rgb(37,99,235)` | Blue label for "You:" message prefix. |
| `briefLabelClaude` | `rgb(215,119,87)` | `rgb(215,119,87)` | Orange label for "Claude:" message prefix. |

### Subagent Identity Colors

Eight dedicated colors are assigned round-robin to concurrent sub-agents so each is visually distinguishable. These tokens are *only* used for subagent identification — never for general UI purposes.

| Token | Dark Value | Purpose |
|---|---|---|
| `red_FOR_SUBAGENTS_ONLY` | `rgb(220,38,38)` | Subagent 1 |
| `blue_FOR_SUBAGENTS_ONLY` | `rgb(37,99,235)` | Subagent 2 |
| `green_FOR_SUBAGENTS_ONLY` | `rgb(22,163,74)` | Subagent 3 |
| `yellow_FOR_SUBAGENTS_ONLY` | `rgb(202,138,4)` | Subagent 4 |
| `purple_FOR_SUBAGENTS_ONLY` | `rgb(147,51,234)` | Subagent 5 |
| `orange_FOR_SUBAGENTS_ONLY` | `rgb(234,88,12)` | Subagent 6 |
| `pink_FOR_SUBAGENTS_ONLY` | `rgb(219,39,119)` | Subagent 7 |
| `cyan_FOR_SUBAGENTS_ONLY` | `rgb(8,145,178)` | Subagent 8 |

### Rainbow Tokens (Ultrathink Highlighting)

Seven-color spectrum with shimmer variants for animated keyword highlighting during extended thinking sequences.

| Base Token | Value | Shimmer Token | Value |
|---|---|---|---|
| `rainbow_red` | `rgb(235,95,87)` | `rainbow_red_shimmer` | `rgb(250,155,147)` |
| `rainbow_orange` | `rgb(245,139,87)` | `rainbow_orange_shimmer` | `rgb(255,185,137)` |
| `rainbow_yellow` | `rgb(250,195,95)` | `rainbow_yellow_shimmer` | `rgb(255,225,155)` |
| `rainbow_green` | `rgb(145,200,130)` | `rainbow_green_shimmer` | `rgb(185,230,180)` |
| `rainbow_blue` | `rgb(130,170,220)` | `rainbow_blue_shimmer` | `rgb(180,205,240)` |
| `rainbow_indigo` | `rgb(155,130,200)` | `rainbow_indigo_shimmer` | `rgb(195,180,230)` |
| `rainbow_violet` | `rgb(200,130,180)` | `rainbow_violet_shimmer` | `rgb(230,180,210)` |

### Animation & Shimmer System

The shimmer system is the TUI's primary animation mechanism. It creates a pulsing "breathing" effect on spinners and active borders.

#### How Shimmer Works
- Most accent tokens have a `*Shimmer` companion (e.g., `claude` / `claudeShimmer`).
- The shimmer companion is a **lighter** variant of the base color.
- Spinners alternate between base and shimmer colors on alternating frames (~80ms cycle):
  - **Even frames** → base color (e.g., `claude` → `rgb(215,119,87)`)
  - **Odd frames** → shimmer color (e.g., `claudeShimmer` → `rgb(235,159,127)`)
- This creates a subtle color oscillation that makes the element feel "alive."

#### Shimmer Token Pairs

| Base Token | Shimmer Token | Context |
|---|---|---|
| `claude` | `claudeShimmer` | Default spinner (Claude thinking) |
| `claudeBlue_FOR_SYSTEM_SPINNER` | `claudeBlueShimmer_FOR_SYSTEM_SPINNER` | System-level operations |
| `permission` | `permissionShimmer` | Permission request pending |
| `promptBorder` | `promptBorderShimmer` | Prompt border animation |
| `inactive` | `inactiveShimmer` | Idle / waiting state |
| `warning` | `warningShimmer` | Rate-limited / warning state |
| `fastMode` | `fastModeShimmer` | Fast execution mode |
| `rainbow_*` | `rainbow_*_shimmer` | Ultrathink highlighting (7 pairs) |

#### Spinner Frames
The primary spinner uses **Braille dot** characters:
```
⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
```
These cycle at approximately 80ms per frame. The spinner also displays a rotating **verb label** (e.g., "Thinking…", "Reading file…", "Writing code…") sourced from `constants/spinnerVerbs.ts`.

---

## Typography

All text is monospace. Font choice is determined by the user's terminal emulator (e.g. JetBrains Mono, SF Mono, Menlo, Consolas). Hierarchy is created exclusively via bold, italic, underline, dim, and inverse styling.

### Hierarchy via Styling

Since font sizes are uniform, visual hierarchy is established through text attributes:

| Style | Effect | Use |
|---|---|---|
| **Bold** | Increased weight | Headlines, file paths, command names, primary highlights |
| *Italic* | Slanted text | Comments, secondary descriptions, hints, placeholders |
| <u>Underline</u> | Underlined | Clickable file-path links (OSC 8 hyperlinks), key shortcuts |
| Dim (inactive color) | Muted gray tone | Timestamps, line numbers, metadata, secondary info |
| Inverse | Swapped fg/bg | Selections, active tags, highlight regions, text selection |
| Strikethrough | Line through text | Deprecated items, cancelled tasks |

### Text Wrap Strategies

Controlled via the `wrap` prop on `<ThemedText>`:

| Mode | Behavior | Best For |
|---|---|---|
| `wrap` (default) | Multi-line word-wrapped output | Chat messages, explanations, markdown |
| `truncate` | Single line, `…` at end | Status bar segments, compact labels |
| `truncate-middle` | Single line, `…` in middle | Long file paths (`src/…/utils.ts`) |
| `truncate-start` | Single line, `…` at start | URLs, paths where the end matters most |
| `truncate-end` | Single line, `…` at end | Same as `truncate` |

---

## Layout

### Character-Cell Grid
TUI spacing is measured in **character cells** (columns × rows), not pixels. Ink's `<Box>` exposes `padding`, `margin`, and `gap` as integer character-cell units.

| Spacing | Characters | Use |
|---|---|---|
| `0` | None | Inline text, tight layouts |
| `1` | 1 cell | Tight padding inside cards, between icon and label |
| `2` | 2 cells | Standard margin between content blocks |
| `3` | 3 cells | Relaxed spacing between sections |
| `4` | 4 cells | Major section breaks |

### Layout Engine
Ink uses **Yoga Layout** (the same flexbox engine that powers React Native). All layout properties map to flexbox:

- `flexDirection`: `'row'` (horizontal) or `'column'` (vertical, default)
- `justifyContent`: `'flex-start'`, `'center'`, `'flex-end'`, `'space-between'`, `'space-around'`
- `alignItems`: `'flex-start'`, `'center'`, `'flex-end'`, `'stretch'`
- `flexGrow` / `flexShrink`: Controls how elements expand/contract with terminal resize
- `width` / `height`: Can be absolute (character count) or percentage (`'100%'`)

### Visual Dividers
- Horizontal lines are drawn using `─` (box-drawing horizontal) via the `<Divider>` component.
- Dividers default to the `subtle` color and fill the available width.
- Optional centered titles break the line: `──── Title ────`

### Responsive Behavior

Terminals do not support CSS media queries. Responsiveness is handled by Ink's flexbox layout engine, which recalculates on terminal resize events (`SIGWINCH`).

#### Column Breakpoints

| Range | Name | Key Adaptations |
|---|---|---|
| < 80 columns | Compact | StatusLine simplifies to critical info only. Dialogs take full width. Tables hide secondary columns. Diffs use unified mode only. |
| 80–120 columns | Standard | Default layout. Dialogs centered with margins. Diffs in unified mode. All StatusLine segments visible. |
| > 120 columns | Wide | Diffs can use side-by-side mode. Extra horizontal padding on messages. StatusLine can show extended metadata. |

#### Scrolling
- Chat history uses scroll buffers managed by `ScrollKeybindingHandler`.
- Keyboard shortcuts for scrolling: Vim-style (`Ctrl+D` / `Ctrl+U` for half-page, `gg` / `G` for top/bottom) and standard (`Page Up` / `Page Down`).
- Code blocks and diagnostic results that exceed width allow horizontal scroll within their container rather than wrapping.

#### Graceful Degradation
- **No truecolor support**: Falls back to ANSI-16 theme where all colors map to standard terminal palette names (`ansi:red`, `ansi:blueBright`, etc.).
- **No Unicode support**: Figures library provides ASCII fallbacks for box-drawing characters.
- **Very narrow terminals (< 40 columns)**: Content wraps aggressively. Borders may be omitted. Not a primary design target.

---

## Elevation & Depth

TUIs cannot use drop shadows or z-index. Visual depth is created through **borders**, **background colors**, and **dimming**:

| Level | Treatment | Use |
|---|---|---|
| Flat | No border, no background | Body text, inline content |
| Subtle divider | `─` line in `subtle` color | Section separators |
| Card surface | Background fill (`userMessageBackground`, `bashMessageBackgroundColor`) | Message containers, shell output |
| Bordered pane | `single` border in `promptBorder` | Input fields, standard panels |
| Alert border | `single` border in `permission` or `error` | Security dialogs, error panels |
| Modal dialog | `round` border, captures keyboard focus | Dialogs, pickers, confirmations |
| Full-screen overlay | Alt-screen buffer, clears terminal | Theme picker, history search, settings |

### Focus States
- **Unfocused input**: Border in `promptBorder` (gray).
- **Focused input**: Border shifts to `claude` (brand orange) or `suggestion` (blue-purple).
- **Security prompt**: Border in `permission` (blue-purple) with shimmer.
- **Error state**: Border in `error` (red).

---

## Shapes

Since the Claude Code CLI runs entirely inside a text terminal, all containers and elements are rectangular. Corner rounding is not supported by standard terminal cells. Visual shapes are simulated using unicode box-drawing border characters (e.g., rounded box corners `┌`, `┐`, `└`, `┘` vs. double or single borders).

---

## Components

The design system is built from 16 primitive components in `components/design-system/`.

### ThemedText
Theme-aware `<Text>` wrapper. Resolves theme keys to raw colors. Accepts `color`, `backgroundColor`, `bold`, `italic`, `underline`, `strikethrough`, `inverse`, `dimColor`, and `wrap`. Also respects `TextHoverColorContext` for cascading hover colors across component boundaries.

### ThemedBox
Theme-aware `<Box>` wrapper. Resolves theme keys for `borderColor`, `borderTopColor`, `borderBottomColor`, `borderLeftColor`, `borderRightColor`, and `backgroundColor`. All other Box/flexbox props pass through.

### Dialog
Centered modal card with `round` border style. Renders a title bar, content area, and optional bottom action row (`KeyboardShortcutHint` buttons). Captures keyboard focus — `Esc` dismisses. Border color defaults to `promptBorder`.

### Divider
Full-width horizontal line using a repeated character (default `─`). Optionally includes a centered title that breaks the line. Color defaults to `subtle`. Measures available container width and fills it.

### Tabs
Horizontal tab row with keyboard navigation (←/→ arrows). Active tab is highlighted with `claude` color and a bottom border indicator. Inactive tabs use `inactive` color. Tabs can display badge counts (e.g., error count in `error` color).

### Pane
Bordered container (panel/card) with optional title rendered inline in the top border. Supports `single`, `round`, or `double` border styles. Default border color is `promptBorder`.

### FuzzyPicker
Full-featured fuzzy search picker. Text input at top with a scrollable result list below. Matched characters are highlighted in `suggestion` color. The selected item gets `selectionBg` background. Supports `Up`/`Down` arrow navigation, `Enter` to select, `Esc` to cancel. Shows a scroll indicator when list exceeds `maxVisible`.

### ListItem
Selectable row component for use in picker/menu interfaces. Supports a prefix icon, primary label, secondary description (dimmed), and suffix badge. When `isSelected`, renders with `selectionBg` background. When `isHighlighted`, matching portions of the label are bolded or colored with `suggestion`.

### StatusIcon
Single-character semantic status indicator. Maps status types to glyphs and colors:
- `success` → `✓` in `success` green
- `error` → `✗` in `error` red
- `warning` → `⚠` in `warning` amber
- `info` → `●` in `suggestion` blue
- `pending` → `○` in `inactive` gray
- `running` → animated spinner in `claude` orange
- `skipped` → `–` in `inactive` gray

### ProgressBar
Horizontal progress bar using `█` (filled) and `░` (empty) block characters. Default fill color is `claude`; default empty color is `subtle`. Accepts `percent` (0–1) and `width` (character count). Uses `Ratchet` internally to prevent backwards movement during async updates. Optionally displays percentage text.

### Ratchet
Logic wrapper (no visual output) that holds the "high water mark" of a numeric value. Prevents progress values from going backwards. The child render function receives the ratcheted value.

### LoadingState
Centered spinner with an optional loading message. Used as a placeholder during async waits (e.g., "Connecting to model…"). Spinner color defaults to `claude`.

### KeyboardShortcutHint
Inline badge showing a keyboard shortcut (e.g., `⌘K`, `Ctrl+C`, `Esc`). Joins multiple keys with `+`. Renders keys in a visually distinct way (bold) with an optional dimmed label.

### Byline
Horizontal metadata row with items separated by `·` (middle dot). Items can be individually colored. Used beneath messages to show model name, token count, cost, and timing. Default color is `inactive`.

### Spinner (top-level component)
Large component (~88KB) that handles the animated Braille-dot spinner, verb label rotation, tool-use progress indicators, nested agent status, cost display, and compaction summaries. Spinner color is context-dependent (see Shimmer Token Pairs table). The verb label rotates through terms like "Thinking…", "Reading file…", "Writing code…" from `constants/spinnerVerbs.ts`.

### StatusLine
Single-row footer pinned to the bottom of the interactive screen. Uses flexbox with `space-between` layout:

| Segment | Position | Content | Token |
|---|---|---|---|
| Mode indicator | Left | Active mode badge ("Plan", "Auto", "Fast") | `planMode`, `fastMode`, `autoAccept` |
| Model name | Left-center | Active model (e.g., "Claude 4 Opus") | `claude` |
| Token usage | Center | Input/output token counts | `inactive` |
| Cost | Center-right | Session cost (e.g., "$0.42") | `warning` when high |
| Rate-limit bar | Right | Mini progress bar | `rate_limit_fill`, `rate_limit_empty` |
| Key hints | Right | Active shortcuts ("Esc to cancel") | `subtle` |

Segments use `flexShrink` to gracefully collapse when the terminal is narrow.

### LogoV2
ASCII art "claude" wordmark rendered in `claude` (brand orange) at the top of fresh sessions. Approximately 6–8 lines tall, ~50 characters wide. A version indicator (e.g., "code") is rendered below in `inactive` (dimmed). Uses `claudeShimmer` for an animated entrance effect.

### ThemePicker
Dialog-based theme picker showing all 7 theme options (6 named + auto). Each option displays a **color swatch row** — small `█` blocks previewing the theme's key colors (`claude`, `success`, `error`, `warning`, `permission`, `suggestion`). The currently active theme has a `✓` prefix. **Live preview**: hovering over an option instantly applies that theme to the entire TUI. Dismissing without selecting restores the original via `cancelPreview()`.

---

## Do's and Don'ts

- Do use theme tokens everywhere. Wrap all text in `<ThemedText>` and boxes in `<ThemedBox>`. Never use raw hex (e.g., `color="#ff0000"`) directly in components.
- Do design for terminal resizing. Use `flexGrow`/`flexShrink` and percentage widths. Handle narrow terminals (< 80 columns) by hiding secondary columns and simplifying layouts.
- Do test in both dark and light themes. Ensure adequate contrast in both. Also spot-check in an ANSI theme to verify 16-color fallback.
- Do use Unicode box-drawing characters. Use the `figures` library or standard Unicode (`─`, `│`, `┌`, `┐`, `└`, `┘`) for borders and dividers. Avoid ASCII fallbacks (`+-+`, `|`) unless the terminal lacks Unicode support.
- Do show activity during async operations. Always display a `Spinner` or `ProgressBar` when waiting for API calls, file operations, or background tasks. The REPL must never appear frozen.
- Do use `Ratchet` for progress values. Wrap any progress percentage in `<Ratchet>` to prevent visual regression from async timing.
- Do respect the shimmer convention. When adding a new accent color that animates, provide a `*Shimmer` companion token that is ~30–40 RGB units lighter.
- Don't use pixel-based attributes. Ink properties are character-cell based. Never set `padding="10px"`, `fontSize="14px"`, or `borderRadius`. These concepts don't exist in terminal rendering.
- Don't hardcode absolute widths. Avoid `width={120}` on major layout containers. Terminals can be any size. Use percentage widths or flex properties.
- Don't assume a specific background color. Terminals can have transparent, dark, light, or custom-colored backgrounds. Keep the base floor transparent; only use background tokens (`userMessageBackground`, `bashMessageBackgroundColor`) for specific card containers.
- Don't use subagent colors for general UI. The `*_FOR_SUBAGENTS_ONLY` tokens are reserved for multi-agent identity. Using them elsewhere would break the visual distinction between agents.
- Don't block the UI thread. Heavy computation or I/O must be non-blocking. The React/Ink render loop must stay responsive for keyboard input and spinner animation.
- Don't mix raw colors with theme keys. Either use a theme key for full theme adaptability, or use a raw `rgb()` value if the color must be theme-independent. Don't switch between approaches for the same UI element across different render paths.
- Don't assume mouse support. Mouse click and drag behaviors are terminal-dependent. Keyboard navigation (arrows, Vim keys, tab cycling) is the primary input mechanism. Mouse interactions are supplementary.
