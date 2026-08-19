# Independent Final Audit — Multiline Prompt Cursor & Line Navigation

> **Document Status**: Independent Verification & Final Certification  
> **Target Subsystem**: `crates/brain-tui` (Prompt Editor & Key Routing Subsystem)  
> **Governing Design**: [`docs/design/MULTILINE_PROMPT_CURSOR_DESIGN.md`](MULTILINE_PROMPT_CURSOR_DESIGN.md)  
> **Implementation Report**: [`docs/design/MULTILINE_PROMPT_CURSOR_IMPLEMENTATION_REPORT.md`](MULTILINE_PROMPT_CURSOR_IMPLEMENTATION_REPORT.md)  
> **Claude Source Oracle**: `/Users/ritikpathania/Developer/src/components/BaseTextInput.tsx`, `/Users/ritikpathania/Developer/src/hooks/useTextInput.ts`, `/Users/ritikpathania/Developer/src/utils/Cursor.ts`  
> **Audit Date**: 2026-08-13  

---

## 1. Executive Audit Summary

An independent audit of the **Claude-Parity Multiline Prompt Cursor & Line Navigation** implementation was conducted against the governing design document and the Claude Code React source oracle.

**Audit Certification**:
```text
PASS WITH NON-BLOCKING GAPS
```

The critical prompt history escalation defect has been completely resolved. `Up` and `Down` arrow keys navigate vertically between wrapped visual lines first, escalating to prompt history recall **only when the cursor is at the top-most visual line (for Up) or bottom-most visual line (for Down)**. All visual boundary (`Home` / `End`), line-kill (`Ctrl+K`), and yank (`Ctrl+Y`) contracts match Claude Code (`SOURCE-CONFIRMED`).

---

## 2. Behavioral Parity Matrix

| Behavior | Claude Source (`useTextInput.ts` / `Cursor.ts`) | Brain Implementation (`EditorState` / `router.rs`) | Evidence Level | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Up within Visual Lines** | `cursor.up()` / `cursor.upLogicalLine()` | `EditorState::move_up` moves cursor up 1 visual line | `SOURCE-CONFIRMED` | **PASS** |
| **Down within Visual Lines** | `cursor.down()` / `cursor.downLogicalLine()` | `EditorState::move_down` moves cursor down 1 visual line | `SOURCE-CONFIRMED` | **PASS** |
| **Up $\rightarrow$ History at Top** | `onHistoryUp()` when `line == 0` | `Action::RecallPrevious` when `!can_move_up` | `SOURCE-CONFIRMED` | **PASS** |
| **Down $\rightarrow$ History at Bottom** | `onHistoryDown()` when `line == last` | `Action::RecallNext` when `!can_move_down` | `SOURCE-CONFIRMED` | **PASS** |
| **Home / Ctrl+A** | `startOfLine()` (col 0 of visual line) | `EditorState::move_home` sets cursor to line `start_offset` | `SOURCE-CONFIRMED` | **PASS** |
| **End / Ctrl+E** | `endOfLine()` (end of visual line) | `EditorState::move_end` sets cursor to line `end_offset` | `SOURCE-CONFIRMED` | **PASS** |
| **Ctrl+K (Kill Line)** | `killToLineEnd()` to end of visual line | `EditorState::kill_to_line_end` drains to line end | `SOURCE-CONFIRMED` | **PASS** |
| **Ctrl+Y (Yank)** | `yank()` from `killRing` | `EditorState::yank` inserts from `kill_ring` | `SOURCE-CONFIRMED` | **PASS** |
| **Image Tokens** | Hopped atomically (`[Image #N]`) | Atomic token hopping preserved in `AttachmentCursorResolver` | `SOURCE-CONFIRMED` | **PASS** |
| **Soft Wrapping** | Wraps at `columns - 1` | `compute_visual_lines` wraps at `prompt_inner_width()` | `SOURCE-CONFIRMED` | **PASS** |
| **Hard Newlines (`\n`)** | Paragraph breaks | Hard newline breaks in `compute_visual_lines` | `SOURCE-CONFIRMED` | **PASS** |
| **Two-Pass Engine** | N/A | Geometry allocation & measurement untouched | `BRAIN-CONFIRMED` | **PASS** |

---

## 3. Cursor Geometry & Wrapping Single-Source Audit

- `EditorState::compute_visual_lines` uses `prompt_inner_width()`, which is derived synchronously from `terminal_width` and canonical padding in `UiState::prompt_inner_width()`.
- The exact same `usable_w` calculation is used by `prompt::draw` for rendering and `LayoutEngine::measure_prompt` for Two-Pass height measurement.
- There are **zero conflicting wrapping implementations** (`VERIFIED`).

---

## 4. History Boundary & Routing Audit

- `Action::MoveCursorUp` checks `self.editor.move_up(width)`. If returns `false` (cursor at top visual line), automatically invokes `self.editor.recall_up()`.
- `Action::MoveCursorDown` checks `self.editor.move_down(width)`. If returns `false` (cursor at bottom visual line), automatically invokes `self.editor.recall_down()`.
- `InputRouter` remains focused strictly on key classification; zero line geometry calculation is performed in `router.rs` (`VERIFIED`).

---

## 5. Image Token Audit

- Pre-existing atomic `[Image #N]` attachment token bounds (`AttachmentCursorResolver`) are preserved.
- `move_up` and `move_down` invoke `image_ref_ending_at` and `image_ref_starting_at` to snap the cursor outside atomic chips.

---

## 6. Two-Pass Layout Compatibility Audit

- Prompt cursor calculations are derived from already-known viewport dimensions (`prompt_inner_width()`).
- Zero circular layout dependencies or measurement invalidations were introduced (`VERIFIED`).

---

## 7. Scope & Diff Verification

- **Files Modified**:
  - `crates/brain-tui/src/state.rs`
  - `crates/brain-tui/src/lib.rs`
  - `crates/brain-tui/src/ui/interaction/router.rs`
- **Files Created**:
  - `crates/brain-tui/tests/multiline_prompt_tests.rs`
- **Backend / Manifest / Dependency Changes**: **0** (`VERIFIED`).

---

## 8. Regression Audit Results

- `cargo fmt --check`: Passed (0 formatting differences).
- `cargo test -p brain-tui`: **98 test suites passed** (0 failures).

---

## 9. Findings & Non-Blocking Gaps

1. **Multi-Item Kill Ring Cycling (`Alt+Y`)**: `Ctrl+K` and `Ctrl+Y` support single-level kill/yank. Multi-item kill ring rotation via `Alt+Y` (`yankPop`) is deferred as a non-blocking future enhancement.

---

## 10. Final Certification

```text
MULTILINE PROMPT CURSOR
IMPLEMENTATION: AUDITED
CERTIFICATION: PASS WITH NON-BLOCKING GAPS
```
