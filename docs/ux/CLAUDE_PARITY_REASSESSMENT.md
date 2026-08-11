# Phase 6 Acceptance Audit Reassessment & Deconstruction (v2)

> **CANONICAL REASSESSMENT**: Critical reassessment of the certified Phase 6 Acceptance Audit (`PHASE6_ACCEPTANCE_REPORT.md`).
> **OBJECTIVE**: Uncover why Phase 6 certified Brain as `MATCH` across all areas while the user experience still feels noticeably different from Claude Code.
> **EVIDENCE TAGS**: `[VERIFIED_CLAUDE]`, `[VERIFIED_BRAIN]`, `[INFERRED]`, `[PROPOSED_ADAPTATION]`.

---

## 1. Executive Reassessment Summary

The Phase 6 Acceptance Audit certified Brain TUI with **100% MATCH** across Home UX, Workspace UX, Conversation Streaming, Visual Language, Persistence/Lifecycle, and End-to-End User Flow.

However, a mechanical reassessment against the **Claude Code Visual Contract** reveals that Phase 6 evaluated **FUNCTIONAL CAPABILITY AND LAYOUT EXISTENCE**, not **OPTICAL WEIGHT, CONTAINER HIERARCHY, OR TRANSITION PHILOSOPHY**.

```text
Phase 6 Audit Scope                       Actual User Visual Experience
───────────────────                       ─────────────────────────────
• "Does Home have logo & prompt?"  → YES  • But Home prompt is boxed & status bar is heavy.
• "Does Workspace show stream?"    → YES  • But Workspace locks a permanent 22-col sidebar.
• "Does Ctrl+K show commands?"     → YES  • But Ctrl+K is a heavy centered modal box.
• "Does status bar show daemon?"   → YES  • But status bar uses 4 heavy boxed metric slots.
• "Do tests pass on 80x24?"        → YES  • But heavy borders consume 30% of character cells!
```

---

## 2. Reassessment Statement & Classification

> **"Brain has strong functional and architectural parity coverage, but that coverage does not currently constitute a sufficiently strict visual/interaction parity contract."**

| Phase 6 Area | Previous Result | Reassessment Classification | Detailed Evidence & Cause of Mismatch |
|---|---|---|---|
| **Home UX** | `MATCH` | **STRUCTURAL DELTA** | Phase 6 verified ASCII logo rendering and prompt presence (`[VERIFIED_BRAIN]`). It missed that Claude's logo is head-of-stream content that scrolls up (`[VERIFIED_CLAUDE]`), whereas Brain treats Home as a distinct screen mode with custom prompt anchoring. |
| **Workspace UX** | `MATCH` | **STRUCTURAL DELTA** | Phase 6 verified session list and chat view (`[VERIFIED_BRAIN]`). It missed that Claude has NO permanent sidebar (`[VERIFIED_CLAUDE]`); Claude allocates 100% width to the conversation stream by default. |
| **Conversation & Streaming** | `MATCH` | **CLOSE** | Functional stream delivery matches byte-for-byte (`[VERIFIED_BRAIN]`). However, Brain wraps the chat stream in a heavy `Rounded` border box, consuming terminal padding. |
| **Visual Language** | `MATCH` | **STRUCTURAL DELTA** | Phase 6 verified color token resolution (`[VERIFIED_BRAIN]`). It missed container hierarchy: Claude uses borderless canvas floor with whitespace (`[VERIFIED_CLAUDE]`), while Brain draws boxes around every component. |
| **Status Bar & Hints** | `MATCH` | **STRUCTURAL DELTA** | Phase 6 verified status text visibility (`[VERIFIED_BRAIN]`). It missed that Claude uses a single-line borderless hint bar (`[VERIFIED_CLAUDE]`), whereas Brain uses a 4-slot boxed status panel. |
| **Command Palette (`Ctrl+K`)** | `MATCH` | **STRUCTURAL DELTA** | Phase 6 verified fuzzy filtering and dispatch (`[VERIFIED_BRAIN]`). It missed overlay composition: Claude uses a floating dropdown (`[VERIFIED_CLAUDE]`), whereas Brain uses a heavy centered modal dialog box. |
| **Reasoning & Evidence** | `MATCH` | **STRUCTURAL DELTA** | Phase 6 verified stage steps and evidence cards (`[VERIFIED_BRAIN]`). It missed progressive disclosure: Claude uses single-line collapsible chips (`🧠 3 memories recalled`), whereas Brain renders multi-row checklists and full evidence cards. |

---

## 3. Why Previous Audits Missed These Deltas

1. **Test-Driven Blind Spots**: Unit and snapshot tests verified that text content appeared inside `Rect` bounds, but did NOT evaluate whether `Block::borders(Borders::ALL)` added unnecessary visual noise.
2. **Feature Existence vs. Visual Philosophy**: The audit checked "Is there a status bar?" rather than "Is the status bar quiet, borderless, and non-intrusive?"
3. **Screen-Based Mental Model**: Previous audits modeled Brain as distinct screens (Home Screen vs. Workspace Screen), whereas Claude Code is a single continuous borderless canvas.

---

*This document establishes the critical reassessment of previous audit findings.*
