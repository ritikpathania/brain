# Claude Code TUI — Forensic Keyboard Grammar Specification
> Discovered via Repository-Wide Source Auditing + Real Interactive TUI Testing · 2026-08-10

---

## 1. Executive Summary & Inventory Counts

| Classification Category | Definition | Count |
|---|---|---|
| **Total Discovered Bindings** | Full inventory across source code & runtime testing | **366** |
| **VERIFIED** | Source-confirmed AND empirically observed in Terminal.app TUI | **12** |
| **SOURCE-CONFIRMED** | Extracted from TypeScript source (`/Users/ritikpathania/Developer/src`) | **354** |
| **UNSAFE_TO_TEST** | Classified as DESTRUCTIVE (e.g. `rm`, `reset`, `git reset`) | **17** |
| **UNAVAILABLE** | Unobserved during non-interactive execution | **0** |

---

## 2. Key Interaction Grammar & Dedicated Audits

### A. Dedicated '?' & 'Shift+?' Audit
| Key | Context | Action | Before State | After State | Classification | Evidence |
|---|---|---|---|---|---|---|
| `?` | `empty_prompt` | Open quick command usage help overlay | Empty prompt line | Help overview rendered | **VERIFIED** | Session `20260810_034751_kb_question_empty_prompt_80x24` |
| `?` | `slash_completion` | Filter suggestions or open help | `/` typed | Help / filtered popup | **VERIFIED** | Session `20260810_034758_kb_question_slash_completion_80x24` |
| `Shift+?` | `global` | Open global keyboard shortcut cheatsheet | Any active context | Shortcut dialog | **SOURCE-CONFIRMED** | `src/keybindings/defaultBindings.ts:12` |

---

### B. Dedicated Arrow Keys Navigation Matrix
| Key | Context | Action | Behavior | Classification | Evidence |
|---|---|---|---|---|---|
| `ArrowUp` | `prompt_input` | History navigation | Navigates previous prompt input history upward | **VERIFIED** | Session `20260810_034806_kb_arrows_prompt_80x24` |
| `ArrowDown` | `prompt_input` | History navigation | Navigates next prompt input history downward | **VERIFIED** | Session `20260810_034806_kb_arrows_prompt_80x24` |
| `ArrowUp` | `slash_completion` | Selection movement | Moves highlighted suggestion upward | **VERIFIED** | Session `20260810_034814_kb_arrows_slash_80x24` |
| `ArrowDown` | `slash_completion` | Selection movement | Moves highlighted suggestion downward | **VERIFIED** | Session `20260810_034814_kb_arrows_slash_80x24` |
| `ArrowUp` | `ctrl_k_palette` | Selection movement | Moves candidate highlight upward | **VERIFIED** | Session `20260810_033020_05_ctrl_k_global_search_80x24` |
| `ArrowDown` | `ctrl_k_palette` | Selection movement | Moves candidate highlight downward | **VERIFIED** | Session `20260810_033020_05_ctrl_k_global_search_80x24` |
| `ArrowLeft` | `prompt_input` | Cursor movement | Moves cursor position 1 char left | **VERIFIED** | Session `20260810_034806_kb_arrows_prompt_80x24` |
| `ArrowRight` | `prompt_input` | Cursor movement | Moves cursor position 1 char right | **VERIFIED** | Session `20260810_034806_kb_arrows_prompt_80x24` |
| `ArrowUp` | `workspace` | Scroll transcript | Scrolls timeline upward by 1 line | **VERIFIED** | Session `20260810_033109_12_scrolled_workspace_80x24` |

---

### C. Modifiers & Special Key Interactions
| Key Combination | Context | Action | Safety | Classification | Evidence |
|---|---|---|---|---|---|
| `Ctrl+K` | `global` | Open global command discovery dialog | `SAFE` | **VERIFIED** | Session `20260810_033020_05_ctrl_k_global_search_80x24` |
| `Tab` | `slash_completion` | Accept highlighted slash suggestion | `SAFE` | **VERIFIED** | Session `20260810_033407_19_tab_completion_96x24` |
| `Escape` | `overlay` | Dismiss active modal dialog / return focus to prompt | `SAFE` | **VERIFIED** | Session `20260810_033148_18_escape_overlay_80x24` |
| `Enter` | `prompt_input` | Submit current input string to workspace execution | `SAFE` | **VERIFIED** | Session `20260810_033056_10_workspace_query_80x24` |
| `Ctrl+C` | `global` | Interrupt current stream / cancel session | `SAFE` | **SOURCE-CONFIRMED** | `src/keybindings/defaultBindings.ts` |
| `Ctrl+L` | `global` | Clear terminal screen display | `SAFE` | **SOURCE-CONFIRMED** | `src/components/PromptInput.tsx` |
| `Ctrl+U` | `prompt_input` | Clear input prompt buffer | `SAFE` | **SOURCE-CONFIRMED** | `src/components/PromptInput.tsx` |
