# Audit Specification — Claude Multiline Prompt Key Routing & Intra-Prompt Line Navigation

> **Document Status**: Complete Mechanical Interaction Audit & Gap Analysis  
> **Target Subsystem**: `crates/brain-tui` (Prompt Editor & Key Routing Subsystem)  
> **Target Scope**: Multiline Key Navigation, Intra-Prompt Line Movement, Wrapped vs Logical Line Boundaries, History Escalation  
> **Governing Foundations**: Native Rust/Ratatui Architecture (ADR-001), Two-Pass Content-Measurement Architecture, Locked `ScrollAnchor` Architecture, Locked `ThinkingBlockWidget`, Locked `NewMessagesPillWidget`  
> **Oracle Source Verification**: `/Users/ritikpathania/Developer/src/components/BaseTextInput.tsx`, `/Users/ritikpathania/Developer/src/hooks/useTextInput.ts`, `/Users/ritikpathania/Developer/src/utils/Cursor.ts`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Summary

This document presents a comprehensive, source-verified forensic audit comparing Claude Code's multiline prompt text editing implementation (`BaseTextInput.tsx`, `useTextInput.ts`, `Cursor.ts`) with Brain's native Ratatui frontend prompt editor (`crates/brain-tui/src/ui/widgets/prompt.rs`, `router.rs`, `state.rs`).

The investigation reveals a critical interaction gap:  
In Brain's current implementation, pressing the **`Up` arrow key** or **`Down` arrow key** in the prompt input field **unconditionally triggers prompt history recall** (`Action::RecallPrevious` / `Action::RecallNext`). When a prompt contains multiple lines (or wraps across multiple visual rows on the screen), pressing `Up` or `Down` immediately replaces the user's multiline draft with a previous history entry instead of navigating the cursor between prompt lines (`BRAIN-CONFIRMED`).

Claude Code solves this by maintaining a two-dimensional cursor model (`Cursor`) that calculates wrapped line boundaries. Up/Down arrow keys move the cursor vertically through wrapped visual lines first. History recall (`onHistoryUp` / `onHistoryDown`) is triggered **only when the cursor is at the top-most line (for Up) or bottom-most line (for Down)** (`SOURCE-CONFIRMED`).

---

## 2. Claude Input Architecture (`SOURCE-CONFIRMED`)

Extracted directly from source oracle files:

### 1. Component Hierarchy:
`PromptInput.tsx` $\rightarrow$ `TextInput.tsx` / `VimTextInput.tsx` $\rightarrow$ `BaseTextInput.tsx` $\rightarrow$ `useTextInput.ts` $\rightarrow$ `Cursor.ts`

### 2. State & Coordinate Model (`Cursor.ts` lines 151-200):
- Raw text is wrapped into `MeasuredText` based on terminal container width (`columns - 1`).
- The `Cursor` class maintains two position representations:
  - 1D Character Offset (`offset`): Byte/char index in the raw text string.
  - 2D Visual Coordinate (`Position { line, column }`): Line index and column offset within `wrappedLines`.
- Native Caret Parking (`useDeclaredCursor` in `BaseTextInput.tsx` lines 38-53): Parks the physical terminal cursor at `(line, column)` to enable OS-native IMEs (e.g. CJK character composition) and screen readers to track caret focus cleanly.

---

## 3. Claude Keyboard Contract (`SOURCE-CONFIRMED`)

Verified from `useTextInput.ts` (lines 224-267):

### 1. Arrow Key & Navigation Routing:
- `Up` / `Ctrl+P`: Invokes `upOrHistoryUp()`.
  - Step 1: Attempts `cursor.up()` (moves up 1 wrapped visual line).
  - Step 2: If `cursor.up()` equals current cursor position and input is `multiline`, attempts `cursor.upLogicalLine()` (moves up 1 paragraph line).
  - Step 3: If cursor cannot move up at all (already on line 0), triggers `onHistoryUp()`.
- `Down` / `Ctrl+N`: Invokes `downOrHistoryDown()`.
  - Step 1: Attempts `cursor.down()` (moves down 1 wrapped visual line).
  - Step 2: If `cursor.down()` equals current cursor position and input is `multiline`, attempts `cursor.downLogicalLine()`.
  - Step 3: If cursor cannot move down at all (already on last line), triggers `onHistoryDown()`.

### 2. Line Boundaries & Word Movements:
- `Ctrl+A` / `Home`: `cursor.startOfLine()` (moves to column 0 of current line; if already at col 0 and not at line 0, moves to col 0 of previous line).
- `Ctrl+E` / `End`: `cursor.endOfLine()` (moves to last column of current line).
- `Alt+B` / `Option+Left`: `cursor.prevWord()` (moves to start of previous word using Unicode `Intl.Segmenter`).
- `Alt+F` / `Option+Right`: `cursor.nextWord()` (moves to start of next word).

### 3. Deletion & Kill Ring Operations:
- `Ctrl+K`: `killToLineEnd()` (deletes text from cursor to end of current line; pushes killed text to global `killRing`).
- `Ctrl+U`: `killToLineStart()` (deletes text from cursor to start of line; prepends killed text to `killRing`).
- `Ctrl+W` / `Alt+Backspace`: `killWordBefore()` (deletes word before cursor into `killRing`).
- `Alt+D`: `deleteWordAfter()` (deletes word after cursor).
- `Ctrl+Y`: `yank()` (pastes last killed text from `killRing`).
- `Alt+Y`: `yankPop()` (cycles through previous `killRing` items after a yank).

### 4. Enter & Hard Newline Semantics:
- `Enter`: Triggers `onSubmit(value)`.
- `Shift+Enter` / `Meta+Enter` / `Alt+Enter`: Inserts hard newline `\n`.
- `\` + `Enter`: Removes trailing `\` and inserts `\n` (`markBackslashReturnUsed`).

---

## 4. Brain Current Prompt Architecture (`BRAIN-CONFIRMED`)

Inspected within `crates/brain-tui`:

### 1. Current State Representation ([`crates/brain-tui/src/ui/widgets/prompt.rs`](../../../crates/brain-tui/src/ui/widgets/prompt.rs)):
- `PromptState` stores:
  - `buffer: String`
  - `cursor: usize` (1D character offset index in `buffer`).
  - `history: Vec<String>`, `history_index: Option<usize>`.

### 2. Current Router Behavior ([`crates/brain-tui/src/ui/interaction/router.rs`](../../../crates/brain-tui/src/ui/interaction/router.rs)):
- `KeyCode::Up`: Immediately returns `Action::RecallPrevious` (`BRAIN-CONFIRMED`).
- `KeyCode::Down`: Immediately returns `Action::RecallNext` (`BRAIN-CONFIRMED`).
- `KeyCode::Enter`: Returns `Action::SubmitPrompt`.
- `Alt+Enter` / `Shift+Enter`: Returns `Action::PromptInsert('\n')`.

---

## 5. Mechanical Behavioral Diff Matrix

| Capability / Interaction | Claude Oracle (`useTextInput.ts` / `Cursor.ts`) | Brain Current Behavior (`prompt.rs` / `router.rs`) | Mechanical Gap Classification | Evidence Level |
| :--- | :--- | :--- | :--- | :--- |
| **`Up` Key on Line 0** | Triggers history recall (`onHistoryUp`) | Triggers `Action::RecallPrevious` | **Identical behavior** | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Up` Key on Line > 0** | Moves cursor up 1 line | Triggers `Action::RecallPrevious` (erases multiline draft!) | **CRITICAL DEFECT**: Replaces draft with history | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Down` Key on Last Line** | Triggers history recall (`onHistoryDown`) | Triggers `Action::RecallNext` | **Identical behavior** | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Down` Key on Line < Last** | Moves cursor down 1 line | Triggers `Action::RecallNext` | **CRITICAL DEFECT**: Replaces draft with history | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Shift+Enter` / `Alt+Enter`** | Inserts `\n` into prompt buffer | Inserts `\n` into prompt buffer | **Matching capability exists** | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Ctrl+A` / `Home`** | Moves cursor to line start | Moves cursor to offset 0 | **Minor Gap**: 1D vs 2D line start | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Ctrl+E` / `End`** | Moves cursor to line end | Moves cursor to text end | **Minor Gap**: 1D vs 2D line end | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **`Ctrl+K` (Kill Line)** | Deletes to line end into `killRing` | Deletes forward 1 character | **Gap**: Missing kill-line action | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Image Attachment Chips** | Atomic `[Image #N]` cursor hopping | Atomic `[Image #N]` token hopping | **Matching capability exists** (`image_paste_tests.rs`) | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |

---

## 6. Recommended Minimal Design Architecture

To resolve the multiline prompt navigation defect while preserving Brain's native Rust/Ratatui architecture and zero-dependency constraint:

### 1. 2D Wrapped Line Calculator in `PromptState`:
Extend `PromptState` with line calculation helpers:
```rust
impl PromptState {
    /// Calculates line and column for current cursor offset given available width.
    pub fn cursor_line_column(&self, width: usize) -> (usize, usize, usize) {
        // Returns (current_line, current_column, total_lines)
    }

    /// Attempts to move cursor up 1 line. Returns true if cursor moved.
    pub fn move_cursor_up(&mut self, width: usize) -> bool {
        // Returns false if cursor is already on line 0
    }

    /// Attempts to move cursor down 1 line. Returns true if cursor moved.
    pub fn move_cursor_down(&mut self, width: usize) -> bool {
        // Returns false if cursor is already on the bottom-most line
    }
}
```

### 2. Context-Aware Input Router:
Update `InputRouter::route_key_event` to inspect `prompt_state.is_multiline()` or call `move_cursor_up()` / `move_cursor_down()`:
- If `Up` key is pressed:
  - If cursor can move up $\rightarrow$ dispatch `Action::PromptMoveUp`.
  - If cursor is at line 0 $\rightarrow$ dispatch `Action::RecallPrevious`.
- If `Down` key is pressed:
  - If cursor can move down $\rightarrow$ dispatch `Action::PromptMoveDown`.
  - If cursor is at bottom line $\rightarrow$ dispatch `Action::RecallNext`.

---

## 7. Test Strategy Matrix

### Unit Tests (`crates/brain-tui/src/ui/widgets/prompt.rs`):
1. `test_multiline_cursor_up_down_boundaries`: Verify `move_cursor_up` returns `false` on line 0 and `true` on line 1.
2. `test_multiline_cursor_column_clamping`: Verify cursor column is clamped when moving to a shorter line.

### Integration Tests (`crates/brain-tui/tests/multiline_prompt_tests.rs`):
1. `test_up_key_moves_up_when_multiline`: Verify `Up` key navigates lines before triggering history.
2. `test_up_key_recalls_history_when_at_top`: Verify `Up` key on line 0 recalls history.
3. `test_down_key_moves_down_when_multiline`: Verify `Down` key navigates lines before history down.

---

## 8. Architecture Impact & Constraints Audit

- `crates/brain-domain`: **UNCHANGED**
- `crates/brain-services`: **UNCHANGED**
- `crates/brain-storage`: **UNCHANGED**
- `Cargo.toml` / `Cargo.lock`: **UNCHANGED (0 external dependencies added)**
- Two-Pass Layout Engine: **UNCHANGED**
- Thinking Blocks & New Messages Pill: **UNCHANGED / UNTOUCHED**

---

## 9. Evidence Classification

- Claude `BaseTextInput.tsx`, `useTextInput.ts`, `Cursor.ts` contract: `SOURCE-CONFIRMED`.
- Brain `prompt.rs`, `router.rs` current behavior & multiline defect: `BRAIN-CONFIRMED`.
- Proposed 2D cursor line navigation architecture: `INFERRED`.

---

## 10. Final Recommendation Gate

```text
APPROVED FOR DESIGN SPECIFICATION
```
