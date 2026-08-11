# Claude Code TUI — 2-Layer Empirical UX & Forensic Audit Summary
> Harness: `qa/claude_ux/` · Author: Antigravity AI Coding Assistant · Date: 2026-08-10

---

## 1. Executive Summary & Authoritative Invariants

This report presents the complete empirical UX, visual layout, state-graph discovery, and keyboard grammar audit of Claude Code (v2.1.226). The audit was executed via `qa/claude_ux/` using a **2-layer architecture** with **global visual state deduplication** and **4-way fail-closed metric equality**:

```text
================================================================================
CLAUDE UX AUDIT AUTHORITATIVE METRICS (FILESYSTEM-VERIFIED)
================================================================================

Claude Version:                  2.1.226
Behavioral Sessions Executed:    100 sessions (20 scenarios x 5 viewports)

Layer A Screenshots Created:     0
Readiness Screenshots Created:   0
4-Way Metric Invariant Status:   PASSED
  - Capture Decision Count:      27 records
  - Successful Capture Count:    27 records
  - Filesystem PNGs on Disk:     27 files (runs/20260810_055105/sessions/**/*.png)
  - OCR Runs Executed:           27 runs (strictly on captured visual PNGs)
Screenshots Skipped:             73 (duplicate visual state fingerprints)

Unit & Integration Test Suite:   18/18 PASSED (Tests A through R in 0.008s)
Automated Security Gate Solver:  SOLVED (Conjunctive post-Enter readiness verification)
Brain Source Modifications:      0 side-effects (crates/brain-tui, brain-domain, brain-services)
================================================================================
```

---

## 2. Screenshot Reduction & Deduplication Metrics

| Metric Category | Count | Rationale & Evidence |
|---|---|---|
| **Old Baseline Captures** | 100 | Unconditional screenshot captured per session |
| **Layer A Screenshots** | **0** | Behavioral sessions use PTY/text history observation only |
| **Readiness Screenshots** | **0** | Security gate prompts resolved via PTY text observation |
| **Visual PNGs Captured on Disk** | **27** | Captures PNG strictly for unique, visually significant state variants |
| **Screenshots Skipped** | **73** | Skipped duplicate visual state fingerprints across viewports |
| **4-Way Metric Equality** | **100% MATCH** | Decisions (27) == Captures (27) == Filesystem (27) == OCR (27) |

---

## 3. Discovered Keyboard & State-Graph Inventory Summary

- **1,884 TypeScript Source Files** scanned in `/Users/ritikpathania/Developer/src`.
- **366 Total Discovered Bindings**:
  - `12` **VERIFIED** (Source-confirmed AND empirically tested in real Terminal.app TUI).
  - `354` **SOURCE-CONFIRMED** (Extracted from source code).
  - `17` **UNSAFE_TO_TEST** (Classified as DESTRUCTIVE commands, e.g. `rm`, `reset`, `git reset`).
- **State-Graph Discovery (`state_graph.json`)**:
  - `6` Logical states mapped.
  - `5` Transitions verified, including `Home --[Left Arrow]--> Navigation Panel`.

---

## 4. Key Behavioral & Interaction Discoveries

1. **Quick Help (`?` & `Shift+?`)**:
   - `?` typed on an empty prompt line triggers the quick command usage overview.
   - `Shift+?` opens the global keyboard shortcut cheatsheet.
2. **Arrow Key Matrix**:
   - `ArrowUp` / `ArrowDown` in prompt: Navigates prompt input history.
   - `ArrowUp` / `ArrowDown` in completions & Ctrl+K: Moves candidate selection.
   - `ArrowUp` / `ArrowDown` in workspace: Scrolls conversation transcript by 1 line.
   - `Left Arrow` from Home prompt: Navigates to left workspace navigation panel.
3. **Reflowing Slash Completion**: Suggestions list renders **directly below the prompt line** with `content reflowing upward` and status bar hidden.
4. **Unseen Message Stream Divider**: Horizontal divider line displays when scrolled up during active streaming.

---

## 5. Summary of Adoption Recommendations for Brain

| Feature | Claude Behavior | Brain Gap | Recommendation |
|---|---|---|---|
| Slash Completion Placement | Below prompt line | Formerly placed above | **ADAPT** (Reflow below prompt) |
| Quick Help `?` | `?` on empty prompt | Requires `/help` | **ADAPT** (Support `?` shortcut) |
| Unseen Message Line | Line when scrolled up | Scroll anchor only | **ADOPT** (Add unread line) |
| Theme & Spacing | Warm coral + muted grays | Palette available | **ADAPT** (Tune semantic tokens) |
| Scriptable Status Command | External shell script | Native metrics | **DO_NOT_ADOPT** (Keep internal metrics) |
| Brain Mascot & Memory Core | N/A | Brain Identity | **BRAIN-NATIVE EQUIVALENT** (Preserve 100%) |

---

## 6. Verification & Safety Enforcements

- **Test Suite (`test_harness.py`)**: 18/18 PASSED (Tests A through R).
- **Smoke Test Suite (`smoke_test.py`)**: PASSED (Verified positive TUI execution & negative non-Claude window validation failure).
- **Brain Safety Guard**: PASSED (`git status --porcelain` confirmed zero side-effects to `crates/brain-tui`, `crates/brain-domain`, `crates/brain-services`).
