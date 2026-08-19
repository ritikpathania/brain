# Interaction Model & State Machine Specification

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`.
> **CANONICAL DESIGN AUTHORITY**: All interaction states, focus transitions, and keyboard grammar are strictly governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).

---

## 1. Prompt-First Interaction Architecture

Brain operates on a **prompt-first conversational interaction model**:
1. **Default State**: Focus is immediately positioned within the boxed `PromptComposer` at launch.
2. **Natural Language by Default**: Users type queries directly without requiring explicit command mode switching.
3. **Floating Slash Autocomplete (`/`)**: Typing `/` at the start of a prompt anchors a floating autocomplete dropdown directly above the composer, displaying available local commands (`/help`, `/session`, `/memory`, `/doctor`).
4. **Command Palette (`Ctrl+K`)**: Opens a centered floating modal overlay with fuzzy matching across sessions, commands, and settings.
5. **Card Expansion (`Ctrl+O`)**: Toggles expanded detail viewports for the active thinking block or tool execution card.
6. **Session Drawer (`Ctrl+S`)**: Toggles the collapsible session history drawer.

---

## 2. Interaction Lifecycle State Machine

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 INTERACTION STATE MATRIX                                    │
├───────────────────┬───────────────────┬────────────────────┬─────────────────┬──────────────┤
│ State             │ Prompt Composer   │ Conversation Canvas│ Floating Overlay│ Status Line  │
├───────────────────┼───────────────────┼────────────────────┼─────────────────┼──────────────┤
│ Idle / Focused    │ Active (Terracotta│ Scrollable         │ None            │ Hints active │
│                   │ Border)           │                    │                 │              │
│ Slash Autocomplete│ Active (Typing /) │ Scrollable         │ Slash Popup     │ Popup hints  │
│ Command Palette   │ Inactive          │ Dimmed Floor       │ Ctrl+K Modal    │ Modal hints  │
│ Generating/Stream │ Shimmering Border │ Follow-Tail Lock   │ None            │ Ctrl+C Cancel│
│ Review / Scrolled │ Inactive / Dim    │ Manually Scrolled  │ New Msgs Pill   │ Scroll hints │
│ Tool Permission   │ Locked            │ Dimmed Floor       │ [y/n/always]    │ Review hints │
└───────────────────┴───────────────────┴────────────────────┴─────────────────┴──────────────┘
```

---

## 3. Keyboard Grammar & Focus Traversal

* **Prompt Editing**: Standard Emacs/Vim line editing (`Ctrl+A`, `Ctrl+E`, `Ctrl+W`, `Ctrl+U`, `Alt+F`, `Alt+B`), `Up`/`Down` history navigation.
* **Multiline Editing**: `Shift+Enter` or `Alt+Enter` inserts newline; standard `Enter` submits query.
* **Modal Dismissal**: `Esc` dismisses active floating overlays and restores primary focus to `PromptComposer`.
* **Streaming Interruption**: `Ctrl+C` sends immediate cancellation signal to background daemon over UDS stream socket.
