# Claude Code Reusable Component Model Specification (v2)

**Document Status**: `CANONICAL SPECIFICATION` (Component Architecture for Brain Frontend)  
**Target Architecture**: Native Rust Terminal User Interface (`crates/brain-tui` / Ratatui)  
**Authority Hierarchy**: Subordinate to [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md)  
**Provenance**: Derived from Claude Code component tree (`/Users/ritikpathania/Developer/src/components/`) and empirical blueprints in commit `38cbb06b`.

---

## 1. Complete Taxonomy of 18 Reusable Component Primitives

Brain's terminal UI is built from 18 reusable component primitives directly structured after Claude Code's component hierarchy:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                             1. AppShell Layer                               │
│                   (`FullscreenLayout`, `<AlternateScreen>`)                 │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
            ┌──────────────────────────┴──────────────────────────┐
            ▼                                                     ▼
┌───────────────────────┐                             ┌───────────────────────┐
│ 2. ScrollableCanvas   │                             │ 3. PinnedBottomRegion │
│ (`flexGrow: 1`)       │                             │ (`flexShrink: 0`)     │
└───────────┬───────────┘                             └───────────┬───────────┘
            │                                                     │
 ┌──────────┴───────────┐                              ┌───────────┴───────────┐
 │                      │                              │                       │
 ▼                      ▼                              ▼                       ▼
┌───────────────┐ ┌───────────────┐           ┌─────────────────┐   ┌─────────────────┐
│ 4. LogoHeader │ │ 5. Timeline   │           │ 6. OverlayLayer │   │ 7. Prompt       │
│ (Greeting Head│ │ Message Stream│           │ (Palette/Modal) │   │ Composer        │
└───────────────┘ └───────┬───────┘           └────────┬────────┘   └────────┬────────┘
                          │                            │                     │
     ┌────────────────────┼────────────────────┐       │                     ▼
     ▼                    ▼                    ▼       │            ┌─────────────────┐
┌───────────────┐  ┌───────────────┐  ┌───────────────┐│            │ 8. StatusLine   │
│ 9. UserMsg    │  │ 10. Assistant │  │ 11. Memory    ││            │ (Footer Bar)    │
│ Block         │  │ Msg Block     │  │ Chip (Inline) ││            └─────────────────┘
└───────────────┘  └───────┬───────┘  └───────────────┘│
                           │                           │
             ┌─────────────┴─────────────┐   ┌─────────┴─────────┬──────────────────┐
             ▼                           ▼   ▼                   ▼                  ▼
    ┌─────────────────┐         ┌─────────────────┐ ┌───────────┐ ┌──────────────┐ ┌──────────────┐
    │ 12. Thinking    │         │ 13. Tool        │ │ 14. Slash │ │ 15. Command  │ │ 16. Help     │
    │ Spinner Block   │         │ Execution Block │ │ Popup     │ │ Palette      │ │ Modal        │
    └─────────────────┘         └─────────────────┘ └───────────┘ └──────────────┘ └──────────────┘
                                                                        │                  │
                                                                        ▼                  ▼
                                                                  ┌───────────┐      ┌───────────┐
                                                                  │ 17. Error │      │ 18. Empty │
                                                                  │ Banner    │      │ State Card│
                                                                  └───────────┘      └───────────┘
```

---

## 2. Component Primitive Specifications (All 18 Primitives)

### Primitive 1: `AppShell`
- **Responsibility**: Root viewport manager dividing terminal character cells into scrollable canvas vs. pinned bottom region.
- **Layout Contract**: `flexDirection: column`, `width: 100%`, `height: 100%`, borderless floor background (`Color::Reset`).
- **Claude Source Reference**: `components/FullscreenLayout.tsx`.

### Primitive 2: `ScrollableCanvas`
- **Responsibility**: Houses conversation history, tool activity, and top greeting header. Handles auto-scroll follow-tail locking.
- **Layout Contract**: `flexGrow: 1`, `flexShrink: 1`, `overflow: hidden`, background `Color::Reset`.
- **Claude Source Reference**: `components/VirtualMessageList.js`, `MessageHistory.tsx`.

### Primitive 3: `PinnedBottomRegion`
- **Responsibility**: Anchors input composer, spinners, overlays, and status line to terminal floor.
- **Layout Contract**: `flexShrink: 0`, `flexGrow: 0`, fixed minimum height `PROMPT_FOOTER_LINES = 5`.
- **Claude Source Reference**: `components/FullscreenLayout.tsx`.

### Primitive 4: `LogoHeader`
- **Responsibility**: Clean typographic greeting header rendered at the top of scrollback history (`Brain 1.1.0 — Relational Memory Engine`).
- **Responsive Layout**:
  - `columns >= 70`: Two-panel horizontal split (`LEFT_PANEL_MAX_WIDTH = 50` for greeting brand, right panel for `Getting Started` hints).
  - `columns < 70`: Compact single-column header.
- **Behavior**: Scrolls naturally out of view as conversation progresses. Zero pixel art, zero mascot avatars.
- **Claude Source Reference**: `components/LogoV2.tsx`.

### Primitive 5: `TimelineMessageStream`
- **Responsibility**: Ordered vertical stream of conversation messages.
- **Spacing**: Exactly 1 blank row between messages (`spacing.normal`).
- **Claude Source Reference**: `components/VirtualMessageList.js`.

### Primitive 6: `OverlayLayer`
- **Responsibility**: Floating viewport container for search dialogs, slash completion, and command palette.
- **Bounds & Geometry**: Positioned directly above `PromptComposer`, respecting `MODAL_TRANSCRIPT_PEEK = 2` lines of peek.
- **Claude Source Reference**: `context/overlayContext.tsx`, `components/design-system/FuzzyPicker.tsx`.

### Primitive 7: `PromptComposer`
- **Responsibility**: Primary user input editor with multiline expansion (`MIN_INPUT_VIEWPORT_LINES = 3` up to 8 visible rows) and Emacs/Vim navigation.
- **Visual Contract**: Boxed input with single-line `Rounded` borders. Focused border turns brand terracotta (`claude: rgb(215,119,87)` / `#D77757`).
- **Claude Source Reference**: `components/PromptInput/PromptInput.tsx`.

### Primitive 8: `StatusLine`
- **Responsibility**: Single-line borderless status and shortcut bar pinned at absolute bottom row (`y = height - 1`).
- **Content**: Left: working directory / session mode; Right: shortcut hints (`/ for commands, Ctrl+K for palette`).
- **Claude Source Reference**: `components/StatusLine.tsx`.

### Primitive 9: `UserMessageBlock`
- **Responsibility**: User query container with subtle prompt prefix (`❯`) and subtle background fill (`userMessageBackground: rgb(30,30,30)` / `#1E1E1E`).
- **Claude Source Reference**: `components/UserMessage.tsx`.

### Primitive 10: `AssistantMessageBlock`
- **Responsibility**: Assistant response container with rich markdown rendering and syntax-highlighted code blocks.
- **Claude Source Reference**: `components/AssistantMessage.tsx`.

### Primitive 11: `RecalledMemoryChip`
- **Responsibility**: Single-line inline provenance chip (`⟡ Recalled 4 memories · [Ctrl+O View Graph]`) for relational knowledge engines.
- **Visual Style**: Dim gray text (`subtle: rgb(80,80,80)` / `#505050`) with soft violet accent (`permission: rgb(177,185,249)` / `#B1B9F9`).

### Primitive 12: `ThinkingSpinnerBlock`
- **Responsibility**: Single-line Braille dot spinner (`⠋ Thinking (2.4s)...`). Auto-collapses to a single summary line on completion. Expandable via `Ctrl+O`.
- **Claude Source Reference**: `components/messages/AssistantThinkingMessage.tsx`, `components/ThinkingToggle.tsx`.

### Primitive 13: `ToolExecutionBlock`
- **Responsibility**: Single-line collapsed summary cards for tool activity (`✓ Read 42 lines from crates/brain-core/src/lib.rs`). Expandable via `Ctrl+O`.
- **Claude Source Reference**: `components/ToolProgress.tsx`.

### Primitive 14: `SlashCommandPopup`
- **Responsibility**: Dropdown autocomplete popup anchored above composer when `/` is typed.
- **Height Calculation**: `visibleCount = Math.max(3, Math.min(items.len(), rows - 5))`.
- **Claude Source Reference**: `components/design-system/FuzzyPicker.tsx`.

### Primitive 15: `CommandPaletteModal`
- **Responsibility**: Centered full modal overlay triggered by `Ctrl+K` for global action search and execution.
- **Claude Source Reference**: `components/GlobalSearchDialog.tsx`.

### Primitive 16: `HelpModal`
- **Responsibility**: Transient keybinding and command reference sheet.

### Primitive 17: `ErrorBanner`
- **Responsibility**: Inline alert box for failed commands or backend connection timeouts styled in coral red (`error: rgb(255,107,128)` / `#FF6B80`).

### Primitive 18: `EmptyStateCard`
- **Responsibility**: Informative container displayed when a search returns zero results or a session list is empty.

---

## 3. Z-Index Layering and Event Dispatching
- **Z-Index 0**: Canvas Floor (Background and borderless floor `Color::Reset`).
- **Z-Index 1**: Message History Stream (Conversations, Code Fences, Tool Cards, Memories).
- **Z-Index 2**: Pinned Input Region (`PromptComposer`, `StatusLine`).
- **Z-Index 3**: Floating Overlay Windows (`SlashCommandPopup`, `CommandPaletteModal`, `NewMessagesPill`).
