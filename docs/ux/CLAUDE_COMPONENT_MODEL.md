# Claude Code Reusable Component Model Specification (v2)

> **CANONICAL SPECIFICATION**: Reusable terminal component architecture reverse-engineered from Claude Code source code (`/Users/ritikpathania/Developer/src`).
> **PRINCIPLE**: UI modeled as composable, stateful component primitives, NOT static screen pages.
> **EVIDENCE TAGS**: All primitives explicitly tagged (`[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]`, `[INFERRED]`, `[PROPOSED_ADAPTATION]`).

---

## 1. Complete Taxonomy of 18 Reusable Component Primitives

Claude Code's terminal UI is built from 18 reusable component primitives:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                             1. AppShell Layer                               │
│                   (`FullscreenLayout.tsx`, `<AlternateScreen>`)             │
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
│ 4. LogoV2     │ │ 5. Timeline   │           │ 6. OverlayLayer │   │ 7. Prompt       │
│ (Header Head) │ │ Message Stream│           │ (Palette/Modal) │   │ Composer        │
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

### Primitive 1: `AppShell` (`FullscreenLayout.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Root viewport manager dividing terminal character cells into scrollable canvas vs. pinned bottom region.
- **Layout Contract**: `flexDirection: 'column'`, `width: '100%'`, `height: '100%'`.

### Primitive 2: `ScrollableCanvas` (`MessageHistory.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Houses conversation history, tool activity, and logo header. Handles auto-scroll tail locking.
- **Layout Contract**: `flexGrow: 1`, `flexShrink: 1`, `overflow: 'hidden'`, background `Color::Reset` (borderless).

### Primitive 3: `PinnedBottomRegion` (`FullscreenLayout.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Anchors input composer, spinners, overlays, and status line to terminal floor.
- **Layout Contract**: `flexShrink: 0`, `flexGrow: 0`.

### Primitive 4: `LogoV2` (`LogoV2.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Home branding header rendered at top of scrollback history.
- **Breakpoints**: `columns >= 70` → two-column (Clawd art left `max 50` | Feed right `min 30`); `columns < 70` → single column.

### Primitive 5: `TimelineMessageStream` (`MessageHistory.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Ordered vertical stream of conversation messages.
- **Spacing**: 1 blank row between messages (`spacing.normal`).

### Primitive 6: `OverlayLayer` (`FuzzyPicker.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Floating viewport container for search dialogs, slash completion, and command palette.
- **Positioning**: Positioned directly above `PromptComposer`.

### Primitive 7: `PromptComposer` (`PromptInput.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Primary user input editor (Emacs/Vim bindings).
- **Visual Contract**: Boxed input with `Rounded` borders. Focused border turns brand accent (`rgb(215,119,87)`).

### Primitive 8: `StatusLine` (`StatusLine.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Single-line borderless status and shortcut bar at absolute bottom row (`y = height - 1`).

### Primitive 9: `UserMessageBlock` (`UserMessage.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: User query container with left accent pillar (`▎`), prompt symbol (`❯`), and timestamp.

### Primitive 10: `AssistantMessageBlock` (`AssistantMessage.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Assistant response container with markdown rendering and `bgSecondary` code fences.

### Primitive 11: `RecalledMemoryChip` (`MemoryChip.tsx`) `[PROPOSED_ADAPTATION]`
- **Responsibility**: Single-line inline summary chip (`🧠 Recalled 3 memories · [View Graph]`) for relational knowledge engines.

### Primitive 12: `ThinkingSpinnerBlock` (`LoadingState.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Single-line Braille dot spinner (`⠋ Thinking...`) with alternating shimmer colors (~80ms cycle).

### Primitive 13: `ToolExecutionBlock` (`ToolProgress.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Single-line collapsible summary (`▶ Read lib.rs (142 lines)`). Expands inline on `Enter`.

### Primitive 14: `SlashCompletionPopup` (`PromptInput.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Borderless dropdown anchored above prompt input displaying command suggestions.

### Primitive 15: `CommandPaletteDropdown` (`GlobalSearchDialog.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Floating search and action dropdown overlay with single-line hint footer (`↑↓ navigate  tab select  esc close`).

### Primitive 16: `HelpOverlayModal` (`HelpDialog.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Modal dialog listing keyboard shortcuts and commands.

### Primitive 17: `ErrorBannerBlock` (`ErrorNotification.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Soft notification block for system errors and warnings.

### Primitive 18: `EmptyStateBlock` (`EmptyState.tsx`) `[VERIFIED_CLAUDE]`
- **Responsibility**: Borderless muted text block with next-step action hints for empty views.

---

*This document establishes the official v2 reverse-engineered Claude Code Component Model (All 18 Primitives).*
