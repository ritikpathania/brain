# Brain ↔ Claude Code Frontend Forensic Reconstruction Audit

> **Document Status**: Normative Architecture & Forensic Audit Specification (Pre-Implementation)  
> **Canonical Target**: Complete Structural, Visual & Interactive Parity with Claude Code Terminal Frontend  
> **Ground Truth Provenance**: Empirical source analysis of `/Users/ritikpathania/Developer/src` (114 React + Ink components)  
> **Backend Integration Boundary**: `BrainFrontendController` → `BrainFrontendAdapter` → `BrainUdsClient` → `Brain Rust Daemon` (100% FROZEN & UNCHANGED)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
BRAIN ↔ CLAUDE CODE FRONTEND FORENSIC RECONSTRUCTION AUDIT
================================================================================
OBJECTIVE:
  Reconstruct the exact Claude Code terminal frontend architecture in React + Ink + Yoga,
  seamlessly backed by Brain's relational memory engine and local UDS protocol.

DUAL-PILLAR MODEL:
  Frontend Shell:   Claude Code (Exact component taxonomy, layout, spacing, typography, tokens)
  Backend Engine:   Brain (Knowledge graph, hybrid search, reflection, tools, sessions, telemetry)

INVARIANTS:
  - Rust Backend (daemon/, crates/): 100% UNCHANGED (0 lines)
  - UDS IPC Protocol: 100% UNCHANGED (0 wire schema mutations)
  - Controller / Adapter / UDS Client Boundary: 100% UNCHANGED
================================================================================
```

---

## 1. Current Brain UI Architecture

### 1.1 Architecture Stack & Invariant Layers
The Brain terminal frontend is built with **React 18 + Ink 5 + Yoga Layout** running on **Bun**. It communicates with the background Rust daemon (`braind`) over Unix Domain Sockets (UDS) using JSONL streaming wire frames:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                       React + Ink Presentation Layer                        │
│                   (InteractiveApp -> App -> FullscreenLayout)               │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │ PresentationState (Read-Only)
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           BrainFrontendAdapter                              │
│              (State Store, Ingestion Reducer, Stream Event Parser)          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │ UDS Event Streams / Requests
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          BrainFrontendController                            │
│                 (Slash Command Router, Keyboard Orchestrator)               │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │ JSONL RPC Messages
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              BrainUdsClient                                 │
│                (Framed Unix Domain Socket IPC Client: braind.sock)          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │ /tmp/braind.sock
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Brain Rust Daemon                              │
│              (Domain Aggregates, Knowledge Graph, Hybrid Search)            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Current Render Tree
```text
InteractiveApp (main.tsx)
  │
  └── <App state={state} /> (App.tsx)
        │
        └── <FullscreenLayout /> (FullscreenLayout.tsx)
              │
              ├── [Header]: 1-row "BRAIN — Relational Memory Engine" + "● ready" + "──────"
              │
              ├── [Scrollable Region]: <Messages /> (Messages.tsx)
              │     ├── (Empty State): <WelcomeHero /> (2-panel welcome greeting)
              │     └── (Timeline): <MessageRow /> (MessageRow.tsx)
              │           ├── <UserTextMessage /> (Prompt ❯ text)
              │           ├── <AssistantThinkingMessage /> (✔ Thought for 2.4s)
              │           ├── <AssistantToolUseMessage /> (Tool card)
              │           ├── <UserToolResultMessage /> (20-line drawer)
              │           └── <AssistantTextMessage /> (<MarkdownText />)
              │
              ├── [Bottom Region]:
              │     ├── <BaseTextInput /> (Rounded prompt composer)
              │     └── <StatusLine /> (1-row status bar)
              │
              └── [Modal Overlays]:
                    ├── <GlobalSearchDialog /> (Command Palette / Ctrl+K)
                    └── <ShortcutsHelpModal /> (Shortcuts table)
```

---

## 2. Actual Claude UI Architecture & Reconstruction (Ground Truth)

Derived directly from `/Users/ritikpathania/Developer/src`:

### 2.1 Claude Component Architecture
```text
App (App.tsx)
  │
  └── <FullscreenLayout /> (components/FullscreenLayout.tsx)
        │
        ├── [ScrollBox Canvas]: <VirtualMessageList /> / <Messages />
        │     ├── <LogoV2 /> / <WelcomeV2 /> (Typographic greeting header in scrollback)
        │     │     ├── Left Panel: Clawd brand, version, status, session path
        │     │     └── Right Panel: FeedColumn / Onboarding / Getting Started
        │     │
        │     └── Messages Stream: <MessageRow />
        │           ├── <UserPromptMessage /> (User query card with userMessageBackground #1E1E1E)
        │           ├── <AssistantThinkingMessage /> (∴ Thinking [Ctrl+O to expand] in italic dim)
        │           ├── <AssistantToolUseMessage /> (ToolUseLoader, formatted name/args)
        │           ├── <UserToolResultMessage /> (CollapsedReadSearchContent / Diff view)
        │           └── <AssistantTextMessage /> (Markdown parser, syntax tokens, code boxes)
        │
        ├── [Pinned Bottom Stack] (flexShrink: 0, PROMPT_FOOTER_LINES = 5):
        │     ├── <SlashAutocompletePopup /> (FuzzyPicker anchored directly above input)
        │     ├── <PromptInput /> (Auto-expanding 3-8 line box with brand terracotta border #D77757)
        │     └── <StatusLine /> (1-row borderless footer: cwd, model/mode, shortcuts)
        │
        └── [Absolute Overlay Pane] (MODAL_TRANSCRIPT_PEEK = 2):
              ├── <GlobalSearchDialog /> (Centered FuzzyPicker with search query and item preview)
              └── <HelpV2 /> (Grouped keyboard shortcut sheets)
```

---

## 3. Claude ↔ Brain Component Mapping Table

| Claude UI Concept | Actual Claude Behavior (`/Developer/src`) | Brain Equivalent | Current Implementation | Required Change |
|---|---|---|---|---|
| **Shell Root** | `FullscreenLayout.tsx`: 2-region stack (Scrollable `ScrollBox` + Pinned bottom + floating `modal` with `MODAL_TRANSCRIPT_PEEK=2`). | App viewport & modal manager | `FullscreenLayout.tsx` | Align geometry constants (`MODAL_TRANSCRIPT_PEEK=2`, `PROMPT_FOOTER_LINES=5`, native floor). |
| **Startup / Greeting** | `LogoV2.tsx` / `WelcomeV2.tsx`: Scrollable greeting header at top of transcript. Two panels on $\ge 70$ cols (`LEFT_PANEL_MAX_WIDTH=50`), compact on $<70$. | Brain Logo Header | `WelcomeHero.tsx` / `LogoHeader.tsx` | Rename/reconstruct to `LogoHeader.tsx`; render inside scrollable transcript top; support breakpoint 70. |
| **User Prompt Card** | `UserPromptMessage.tsx`: Card on `userMessageBackground` (`#1E1E1E` / `rgb(55,55,55)`), subtle padding, truncated at 10,000 chars. | User query message | `UserTextMessage.tsx` | Wrap in `userMessageBackground` container with subtle padding and clean prompt indicator. |
| **Thinking Lifecycle** | `AssistantThinkingMessage.tsx`: `∴ Thinking [Ctrl+O to expand]` in dim italic text. Expands to dimmed markdown. | Stage & reasoning trace | `AssistantThinkingMessage.tsx` | Use canonical glyph `∴ Thinking`, dim italic styling, inline `[Ctrl+O to expand]`. |
| **Tool Invocations** | `AssistantToolUseMessage.tsx`: Formatted action label (e.g. `Read(path)`), spinner while active, status dot when done. | Tool call request | `AssistantToolUseMessage.tsx` | Render compact structured action cards with formatted parameters, permission prompt for `pending`. |
| **Tool Results** | `CollapsedReadSearchContent.tsx` / `UserToolResultMessage`: 1-line collapsed summary with line count, expandable via `Ctrl+O`. | Tool output drawer | `UserToolResultMessage.tsx` | Format line-numbered drawer capped at 20 lines with `[Ctrl+O to collapse]`. |
| **Assistant Markdown** | `Markdown.tsx` + `HighlightedCode.tsx`: AST markdown with rounded code fences, syntax coloring, tables, diffs. | Assistant response | `AssistantTextMessage.tsx` + `MarkdownText.tsx` | Refine syntax keywords, diff colors, and table support with zero external dependencies. |
| **Prompt Composer** | `PromptInput.tsx`: Single-line rounded box, auto-expanding 3-8 lines, focused border `claude` (`#D77757`). | Prompt editor | `BaseTextInput.tsx` | Apply terracotta focused border (`#D77757`), prompt glyph `❯ `, multiline expansion cues. |
| **Slash Autocomplete** | `FuzzyPicker.tsx`: Floating popup menu positioned directly above composer when buffer starts with `/`. | Slash command completion | `SlashAutocompletePopup.tsx` | Create `SlashAutocompletePopup.tsx` anchored above prompt box with active selection pointer `▶ `. |
| **Command Palette** | `GlobalSearchDialog.tsx`: Centered modal with search query bar, live filter, preview pane, and key hints. | Command & memory search | `GlobalSearchDialog.tsx` | Reconstruct with `FuzzyPicker` geometry, live filtering, and memory search dispatch. |
| **Status Bar** | `StatusLine.tsx`: Single-line borderless bar pinned at bottom row with session info, working dir, shortcut hints. | Footer status line | `StatusLine.tsx` | Align exact layout: Left: session / working directory; Right: `[Ctrl+K] Palette │ [/help] Commands`. |
| **Memory Provenance** | `RecalledMemoryChip.tsx`: Inline chip `⟡ Recalled 4 memories · [Ctrl+O View Graph]` in dim violet. | Relational memory provenance | `RecalledMemoryChip.tsx` | Create `RecalledMemoryChip.tsx` using `ThemeTokens.glyphs.memoryChip` (`⟡`) and `ThemeTokens.colors.permission`. |

---

## 4. Visual Contract & Tokens

Derived from Claude Code `theme.ts` (`darkTheme`):

```typescript
export const ThemeTokens = {
  colors: {
    // Claude Terracotta Primary Brand Accent
    claude: '#D77757',
    accent: '#D77757',
    accentBright: '#E08567',
    brandGold: '#D97706',

    // Surface & Border Tokens
    promptBorder: '#888888',
    subtle: '#505050',
    borderSubtle: '#505050',
    borderFocused: '#D77757',
    borderError: '#E11D48',
    borderWarning: '#D97706',

    // Badges & Permissions
    permission: '#B1B9F9',
    autoAccept: '#AF87FF',
    userMessageBackground: '#1E1E1E',

    // Text Hierarchy
    textPrimary: '#FFFFFF',
    textSecondary: '#888888',
    textMuted: '#505050',
    textDim: true,

    // Semantic Status Tokens
    statusConnected: '#4D9375',
    statusConnecting: '#D97706',
    statusDisconnected: '#E11D48',
    statusThinking: '#D77757',

    // Code & Syntax Surface
    codeKeyword: '#D77757',
    codeString: '#4D9375',
    codeComment: '#505050',
    codeNumber: '#D97706',
  },

  // Glyphs & Iconography
  glyphs: {
    prompt: '❯',
    pointer: '▶',
    statusDot: '●',
    connectingDot: '◐',
    disconnectedDot: '○',
    thinking: '∴',
    success: '✔',
    failure: '✖',
    dividerHorizontal: '─',
    dividerVertical: '│',
    arrowDown: '↓',
    arrowUp: '↑',
    memoryChip: '⟡',
  },

  // Geometry Constants
  layout: {
    headerHeight: 1,
    footerHeight: 1,
    promptMinHeight: 3,
    promptMaxHeight: 8,
    modalWidthPercent: 80,
    drawerMaxLines: 20,
    logoBreakpoint: 70,
    leftPanelMaxWidth: 50,
    minRightWidth: 30,
    modalPeekRows: 2,
    promptFooterRows: 5,
  },
} as const;
```

---

## 5. Interaction Contract

1. **Typing Slash Commands (`/`)**:
   - When the prompt buffer starts with `/` and no modal is active, `<SlashAutocompletePopup />` mounts floating above the prompt composer.
   - Arrow keys (`↑`/`↓`) navigate matching slash commands; `Enter` or `Tab` autocompletes the selected command.
2. **Command Palette (`Ctrl+K`)**:
   - Opens centered `<GlobalSearchDialog />` overlay.
   - User types query; live filtered list updates; `Enter` executes command or dispatches knowledge graph search.
3. **Interactive Tool Approvals**:
   - When a tool requires approval (`state === 'pending'`), the prompt area or tool card displays `❯ Permission required: [y/Enter to approve, n/Esc to deny]`.
   - Pressing `y` or `Enter` approves execution; pressing `n` or `Esc` denies execution with error notice.
4. **Drawer Toggling (`Ctrl+O` / `Alt+T`)**:
   - Expands or collapses thinking trace blocks and tool output drawers.
5. **Session Switching & History (`/sessions` / `/session <id>`)**:
   - Restores past session timeline with full tool/thinking state.

---

## 6. Target Render Tree

```text
InteractiveApp (main.tsx)
  │
  └── <App state={state} /> (App.tsx)
        │
        └── <FullscreenLayout /> (FullscreenLayout.tsx)
              │
              ├── [Top Canvas]: <Messages /> (Messages.tsx)
              │     │
              │     ├── <LogoHeader /> (LogoHeader.tsx - 2-panel greeting header)
              │     │
              │     └── Stream: <MessageRow /> (MessageRow.tsx)
              │           ├── <RecalledMemoryChip /> (⟡ Recalled N memories)
              │           ├── <UserTextMessage /> (Card with userMessageBackground)
              │           ├── <AssistantThinkingMessage /> (∴ Thinking [Ctrl+O])
              │           ├── <AssistantToolUseMessage /> (Action card + [y/n] prompt)
              │           ├── <UserToolResultMessage /> (Line-numbered drawer)
              │           └── <AssistantTextMessage /> (MarkdownText + syntax highlighting)
              │
              ├── [Pinned Bottom]:
              │     │
              │     ├── (Conditional): <SlashAutocompletePopup /> (FuzzyPicker above composer)
              │     │
              │     ├── <BaseTextInput /> (Rounded prompt composer with ❯ and #D77757 border)
              │     │
              │     └── <StatusLine /> (1-row footer: cwd, engine, status, shortcuts)
              │
              └── [Modal Overlays]:
                    ├── <GlobalSearchDialog /> (Command Palette / Ctrl+K)
                    └── <ShortcutsHelpModal /> (Grouped keyboard shortcuts reference)
```

---

## 7. Keyboard Contract

| Keybinding | Context | Action |
|---|---|---|
| `Enter` | Prompt Composer | Submit query or slash command |
| `Shift+Enter` | Prompt Composer | Insert newline without submitting |
| `Ctrl+K` | Global Viewport | Open Command Palette & Memory Search modal |
| `Ctrl+O` / `Alt+T` | Global Viewport | Toggle expand/collapse of Thinking and Tool Drawers |
| `y` / `Enter` | Pending Tool Approval | Approve tool execution |
| `n` / `Esc` | Pending Tool Approval | Deny tool execution |
| `↑` / `↓` | Autocomplete / Palette | Navigate list items |
| `Tab` / `Enter` | Autocomplete Popup | Autocomplete selected slash command |
| `Esc` | Modal / Popup | Dismiss active modal overlay |
| `Ctrl+C` | Global Viewport | Interrupt active stream or double-press to exit |

---

## 8. State Mapping (`presentation.ts` ↔ Components)

| PresentationState Path | Target Component Consumer | Usage |
|---|---|---|
| `state.session` | `StatusLine`, `LogoHeader` | Session ID, title, active working directory |
| `state.timeline` | `Messages`, `MessageRow` | Full conversation message stream |
| `state.streaming` | `AssistantTextMessage`, `Messages` | Active streaming text and trailing cursor `▌` |
| `state.thinking` | `AssistantThinkingMessage` | Reasoning state, duration, thinking trace |
| `state.tools` | `AssistantToolUseMessage`, `UserToolResultMessage` | Interactive tool executions, arguments, approvals |
| `state.prompt` | `BaseTextInput`, `SlashAutocompletePopup` | Input buffer, cursor offset, slash trigger |
| `state.overlays` | `GlobalSearchDialog`, `ShortcutsHelpModal` | Active modal type, live search query |
| `state.footer` | `StatusLine`, `LogoHeader` | Daemon status, memory status, engine version |
| `state.connection` | `FullscreenLayout`, `StatusLine` | Connection state, error banners |

---

## 9. Components to Reuse
1. `src/components/FullscreenLayout.tsx` — Viewport partitioner and modal layer.
2. `src/components/StatusLine.tsx` — Pinned footer bar.
3. `src/components/BaseTextInput.tsx` — Pinned prompt composer.
4. `src/components/MessageRow.tsx` — Message dispatcher.
5. `src/components/Messages.tsx` — Stream container.
6. `src/components/messages/MarkdownText.tsx` — Native markdown & syntax engine.
7. `src/components/messages/AssistantTextMessage.tsx` — Markdown wrapper.
8. `src/components/GlobalSearchDialog.tsx` — Command palette overlay.
9. `src/components/ShortcutsHelpModal.tsx` — Shortcuts table overlay.

---

## 10. Components to Replace / Refactor
1. `src/components/WelcomeHero.tsx` $\rightarrow$ Refactor into `LogoHeader.tsx` (matches Claude `LogoV2.tsx` specification).
2. `src/components/messages/UserTextMessage.tsx` $\rightarrow$ Refactor with `userMessageBackground` container fill.
3. `src/components/messages/AssistantThinkingMessage.tsx` $\rightarrow$ Refactor with canonical `∴ Thinking` glyph.
4. `src/components/messages/AssistantToolUseMessage.tsx` $\rightarrow$ Refactor with Claude action formatting.
5. `src/components/messages/UserToolResultMessage.tsx` $\rightarrow$ Refactor with bounded drawer.

---

## 11. Components to Delete
- None. (Previous prototype components have already been replaced).

---

## 12. Components to Create
1. `src/components/LogoHeader.tsx` — 2-panel responsive greeting header at the top of scrollback history.
2. `src/components/SlashAutocompletePopup.tsx` — Floating autocomplete menu anchored above composer.
3. `src/components/messages/RecalledMemoryChip.tsx` — Inline relational memory provenance chip.

---

## 13. Backend Invariants (Strict Protection)
- **Rust Backend**: `daemon/**`, `crates/**` remain 100% UNCHANGED (0 lines).
- **UDS Protocol**: JSONL wire frames remain 100% UNCHANGED (0 lines).
- **Client & Controller Boundary**: `BrainUdsClient`, `BrainFrontendController`, `BrainFrontendAdapter` remain 100% UNCHANGED.

---

## 14. Phased Migration Plan

- **Phase 6.1**: Create `RecalledMemoryChip.tsx`, `LogoHeader.tsx`, and `SlashAutocompletePopup.tsx`.
- **Phase 6.2**: Reconstruct `UserTextMessage.tsx`, `AssistantThinkingMessage.tsx` (`∴ Thinking`), and `AssistantToolUseMessage.tsx`.
- **Phase 6.3**: Connect `App.tsx` and `Messages.tsx` to mount `LogoHeader` in transcript top and `SlashAutocompletePopup` above prompt.
- **Phase 6.4**: Run regression suite (`bun test`, `cargo check`), execute multi-viewport tests, and certify final release.

---

## 15. Verification Strategy
1. **Automated Behavioral Test Suite**: All existing tests must pass (`bun test`).
2. **Rust Workspace**: `cargo check` must return clean Exit Code 0.
3. **Fixture Terminal Visual Verification**: Verify `LogoHeader`, `SlashAutocompletePopup`, `RecalledMemoryChip`, `Thinking`, and `Tools` across 70x20, 80x24, and 100x30 viewports.
4. **Zero Protocol Mutation**: Ensure 0 changes to Rust or UDS contracts.

---

```text
================================================================================
AUDIT VERDICT: PROCEED
ALL CLAUDE CODE SOURCE STRUCTURES RECOVERED & MAPPED TO BRAIN CAPABILITIES
================================================================================
```
