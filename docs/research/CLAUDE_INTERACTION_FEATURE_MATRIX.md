# Claude Code Interaction Feature Matrix & Brain Adoption Guide

> Derived from Source Scanning + Real Interactive TUI Discovery · 2026-08-10

| Feature | Context | Keyboard / Trigger | Visual Surface | Source Evidence | Runtime Evidence | Recommendation |
|---|---|---|---|---|---|---|
| **Slash Command Autocomplete** | Prompt input | `ArrowUp / ArrowDown / Tab / Enter / Esc` | Borderless aligned list of command names & descriptions | `src/components/PromptInputFooterSuggestions.tsx` | VERIFIED (Session 20260810_032931_03_slash_completion_80x24) | **ADAPT (Reposition below prompt line as specified)** |
| **Global Command Discovery / Ctrl+K** | Global overlay | `Ctrl+K / ArrowUp / ArrowDown / Enter / Esc` | Search prompt input + filtered candidate list | `src/components/GlobalSearchDialog.tsx` | VERIFIED (Session 20260810_033020_05_ctrl_k_global_search_80x24) | **ADAPT (Align prompt-anchored palette reflow)** |
| **Quick Keyboard Help Surface** | Prompt input | `? / Shift+? / Esc` | Compact usage table & shortcut cheatsheet | `src/keybindings/defaultBindings.ts` | VERIFIED (Session 20260810_034751_kb_question_empty_prompt_80x24) | **ADAPT (Support '?' shortcut on empty prompt)** |
| **Unseen Message Stream Divider** | Workspace timeline | `PageUp / ArrowUp` | Subtle horizontal line with 'Unread' label | `src/components/UnreadDivider.tsx` | VERIFIED (Session 20260810_033116_13_unseen_message_state_80x24) | **ADOPT (Add unread divider when scrolled up during stream)** |
| **Responsive Multi-Column Breakpoints** | Home / Startup | `SIGWINCH resize signal` | Left panel Clawd mascot (<=50 cols), right panel feed (>=30 cols) | `src/utils/logoV2Utils.ts` | VERIFIED (Sessions across 80x24, 96x24, 120x30, 156x52, 182x53) | **BRAIN-NATIVE EQUIVALENT (Already implemented in renderer.rs)** |
| **Scriptable Shell Status Line Command** | Status bar | `N/A` | Custom text status bar item | `src/components/StatusLine.tsx` | SOURCE-CONFIRMED ONLY | **DO_NOT_ADOPT (Preserve Brain's internal domain metrics)** |