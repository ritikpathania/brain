# Authoritative Baseline — Frozen React + Ink + Yoga Frontend Infrastructure

> **Document Status**: Authoritative Baseline & Infrastructure Freeze Certification  
> **Target Package**: `packages/brain-frontend` (React + Ink + Yoga Stack)  
> **Source Oracle**: Claude Code Frontend Source Oracle (`/Users/ritikpathania/Developer/src/**`)  
> **Infrastructure Status**: `🔒 FROZEN FRONTEND INFRASTRUCTURE`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
==================================================
FRONTEND INFRASTRUCTURE FREEZE
==================================================
STATUS: 🔒 FROZEN FRONTEND INFRASTRUCTURE
CANONICAL STACK: React 18 + Ink + Yoga Flexbox Layout Engine
SOURCE ORACLE: /Users/ritikpathania/Developer/src/**
ACCEPTED PACKAGE: packages/brain-frontend
FIRST-FRAME VERIFICATION: 100% Immediate (0 resize/SIGWINCH required)
PARITY AUDIT RESULT: PASS — VERIFIED VISUAL PARITY
FUTURE MODIFICATION RULE: Requires explicit regression justification & audit
```

---

## 1. Architectural Boundary & Contract Freeze

From this milestone forward, the frontend presentation architecture is formally locked as **frozen infrastructure**:

```text
┌────────────────────────────────────────────────────────┐
│           Claude UI / UX Visual Experience             │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│        🔒 Frozen React / Ink / Yoga Shell             │
│   (packages/brain-frontend/src/components/*)           │
└───────────────────────────┬────────────────────────────┘
                            │ Pure PresentationState Updates
                            ▼
┌────────────────────────────────────────────────────────┐
│              PresentationState Schema                  │
│       (packages/brain-frontend/src/types/*)            │
└───────────────────────────┬────────────────────────────┘
                            │ Data Translation Boundary
                            ▼
┌────────────────────────────────────────────────────────┐
│               BrainFrontendAdapter                     │
│      (packages/brain-frontend/src/adapter/*)           │
└───────────────────────────┬────────────────────────────┘
                            │ UDS Unix Domain Socket
                            ▼
┌────────────────────────────────────────────────────────┐
│               Brain Runtime Daemon                     │
└────────────────────────────────────────────────────────┘
```

---

## 2. Accepted Visual & Interaction Contracts

1. **3-Slot Container**: `FullscreenLayout.tsx` provides 1-row Header, scrollable Chat Viewport (`flexGrow: 1`), 3-row Prompt Editor, and 1-row Status Line.
2. **First-Frame Immediate Render**: Yoga flexbox layout solves geometry in 1 pass before frame emission without requiring `SIGWINCH` or redraw timers.
3. **Follow-Tail & Scrolling**: `followTail == true` auto-scrolls to stream tail. Scrolling away activates reading mode and preserves anchor position.
4. **Sticky Prompt Header**: Pinned 1-row header (`❯ <collapsed_prompt>`) appears at `y = chat_area.y` when prompt scrolls above viewport.
5. **New Messages Pill**: Appears at bottom row (`y = chat_area.y + height - 1`) when scrolled away: `↓ N new messages (Jump to bottom)`.
6. **Thinking Blocks**: Reasoning drawer with `⏺ Thinking... (duration)` status symbol and `Ctrl+O` toggle.
7. **Tool Execution Cards**: 5 states (`pending`, `running`, `completed`, `failed`, `denied`) with 20-line capped drawer and line numbering.
8. **Command Palette & Shortcuts**: `Ctrl+K` command overlay and `?` keybindings help modal.
9. **Status Line**: Clean Brain engine metrics (`● Brain v1.1.0 | daemon:connected | memory:active`).

---

## 3. Accepted Viewport Matrix

| Dimension | Category | Layout Bounds Allocation | First-Frame Status |
| :--- | :--- | :--- | :--- |
| **80x24** | Standard Terminal | Header: 1, Chat: 18, Prompt: 3, Status: 1 | `PASS` |
| **69x24** | Narrow Terminal | Header: 1, Chat: 18, Prompt: 3, Status: 1 | `PASS` |
| **70x40** | Medium Porting | Header: 1, Chat: 34, Prompt: 3, Status: 1 | `PASS` |
| **100x26** | Wide Terminal | Header: 1, Chat: 20, Prompt: 3, Status: 1 | `PASS` |
| **120x30** | Sticky Header Active | Header: 1, Sticky: 1, Chat: 23, Prompt: 3, Status: 1 | `PASS` |
| **120x40** | Large Monitor | Header: 1, Chat: 34, Prompt: 3, Status: 1 | `PASS` |
| **182x53** | Ultrawide Viewport | Header: 1, Chat: 47, Prompt: 3, Status: 1 | `PASS` |

---

## 4. Retained Non-Blocking Gaps Record

1. `Alt+Y` multi-item kill-ring rotation (`yankPop`) — Non-blocking gap.
2. Historic tool card keyboard selection (`Ctrl+O` targets active drawer) — Non-blocking gap.
3. Sticky prompt mouse click trigger — Non-blocking gap (requires terminal mouse router).

---

## 5. Strict Modification Guardrail

> **RULE**: No modifications to `packages/brain-frontend/src/components/*` or `types/*` are permitted unless an actual regression is identified. Any proposed change requires explicit regression justification and formal re-certification.
