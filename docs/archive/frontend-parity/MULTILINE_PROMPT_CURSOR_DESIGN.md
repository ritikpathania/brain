# Design Specification — Claude-Parity Multiline Prompt Cursor & Line Navigation

> **Document Status**: Implementation-Grade Design Specification  
> **Target Subsystem**: `crates/brain-tui` (Prompt Editor & Key Routing Subsystem)  
> **Target Scope**: 2D Visual Cursor Navigation, Line Wrapping & Hard Newlines, History Escalation, Line-Bounded Home/End, Kill Line  
> **Governing Foundations**: Native Rust/Ratatui Architecture (ADR-001), Two-Pass Content-Measurement Architecture, Locked `ScrollAnchor` Architecture, Locked `ThinkingBlockWidget`, Locked `NewMessagesPillWidget`  
> **Oracle Source Verification**: `/Users/ritikpathania/Developer/src/components/BaseTextInput.tsx`, `/Users/ritikpathania/Developer/src/hooks/useTextInput.ts`, `/Users/ritikpathania/Developer/src/utils/Cursor.ts`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Summary

This document specifies the implementation-grade architecture for bringing Brain's prompt input editor into full interaction parity with Claude Code (`BaseTextInput.tsx`, `useTextInput.ts`, `Cursor.ts`).

### The Primary Defect Solved:
In Brain's existing TUI frontend, pressing the **`Up` arrow key** or **`Down` arrow key** in the prompt editor unconditionally dispatches prompt history recall (`Action::RecallPrevious` / `Action::RecallNext`). When a prompt contains multiple lines or wraps across multiple visual rows on narrow terminals, pressing `Up` or `Down` immediately replaces the user's draft with a previous history entry instead of navigating between prompt lines (`BRAIN-CONFIRMED`).

### The Solution:
Introduce a lightweight, 2D visual line & column calculation model (`PromptCursorModel`) encapsulated directly inside `PromptState` in `crates/brain-tui/src/ui/widgets/prompt.rs`. Up/Down arrow keys navigate between wrapped visual lines first; prompt history recall is escalated **only when the cursor is at the top-most visual line (for Up) or bottom-most visual line (for Down)** (`SOURCE-CONFIRMED`).

---

## 2. Cursor Coordinate Model & Line Wrapping

Rather than building an oversized generic text editor, Brain will encapsulate a deterministic 2D visual line calculator inside `PromptState`.

### Coordinate Planes:
1. **1D Character Offset (`cursor`)**: Raw character index within `buffer: String`.
2. **2D Visual Coordinate (`visual_line`, `visual_column`)**:
   - `visual_line`: Index of the wrapped visual row on screen ($0 \dots L-1$).
   - `visual_column`: Display column width from line start to cursor position ($0 \dots W$).

### Wrapped Line Layout Algorithm (`PromptState::calculate_visual_lines`):
```rust
/// Represents a single wrapped visual line within the prompt buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualLineRange {
    /// Start character index in buffer (inclusive).
    pub start_offset: usize,
    /// End character index in buffer (exclusive).
    pub end_offset: usize,
    /// Whether this line ends with a hard newline '\n'.
    pub is_hard_newline: bool,
}

impl PromptState {
    /// Computes wrapped visual lines for a given available prompt inner width.
    pub fn compute_visual_lines(&self, width: usize) -> Vec<VisualLineRange> {
        let width = width.max(1);
        let mut lines = Vec::new();
        let mut line_start = 0;

        for (idx, ch) in self.buffer.char_indices() {
            if ch == '\n' {
                lines.push(VisualLineRange {
                    start_offset: line_start,
                    end_offset: idx,
                    is_hard_newline: true,
                });
                line_start = idx + 1;
            } else if idx - line_start >= width {
                lines.push(VisualLineRange {
                    start_offset: line_start,
                    end_offset: idx,
                    is_hard_newline: false,
                });
                line_start = idx;
            }
        }

        lines.push(VisualLineRange {
            start_offset: line_start,
            end_offset: self.buffer.len(),
            is_hard_newline: false,
        });

        lines
    }
}
```

---

## 3. Vertical Navigation Architecture (`move_cursor_up` & `move_cursor_down`)

Vertical cursor movement operates on wrapped visual lines while maintaining a `preferred_column` to preserve horizontal positioning across lines of varying lengths.

### `move_cursor_up(width)`:
```text
Pressed Up Arrow (or Ctrl+P)
             │
             ▼
Compute visual lines for `width`
             │
             ├── Is current cursor on visual_line > 0?
             │         │
             │         ├── YES ──► Move cursor to (visual_line - 1, preferred_column)
             │         │           Return true (Handled locally)
             │         │
             │         └── NO  ──► Return false (Escalate to Action::RecallPrevious)
```

### `move_cursor_down(width)`:
```text
Pressed Down Arrow (or Ctrl+N)
             │
             ▼
Compute visual lines for `width`
             │
             ├── Is current cursor on visual_line < (total_lines - 1)?
             │         │
             │         ├── YES ──► Move cursor to (visual_line + 1, preferred_column)
             │         │           Return true (Handled locally)
             │         │
             │         └── NO  ──► Return false (Escalate to Action::RecallNext)
```

---

## 4. Cursor Column Preservation & Sticky Column

When navigating vertically across lines of unequal lengths (e.g. line 1 has 40 chars, line 2 has 10 chars, line 3 has 40 chars):
1. `PromptState` maintains a `preferred_column: Option<usize>`.
2. When the user moves Left/Right or types text, `preferred_column` is updated to `None` (resetting to current column).
3. When `move_cursor_up` or `move_cursor_down` is invoked:
   - If `preferred_column` is `None`, record `preferred_column = Some(current_column)`.
   - Target line column is computed as `target_col = min(preferred_column, target_line_len)`.
   - This ensures moving down into a short line and then down into a long line restores the original horizontal column alignment (`SOURCE-CONFIRMED`).

---

## 5. Home / End Semantics (`Ctrl+A` & `Ctrl+E`)

In Claude Code, `Home` / `Ctrl+A` and `End` / `Ctrl+E` operate relative to the **current visual line**:

- **`Ctrl+A` / `Home` (`start_of_line`)**:
  - Moves cursor to `start_offset` of the current visual line.
  - If cursor is already at `start_offset` and `visual_line > 0`, moves to `start_offset` of the previous line (`SOURCE-CONFIRMED`).
- **`Ctrl+E` / `End` (`end_of_line`)**:
  - Moves cursor to `end_offset` of the current visual line.

---

## 6. Kill-Line Semantics (`Ctrl+K` & Kill Ring)

### `kill_to_line_end()`:
- Deletes text from `cursor` to `end_offset` of the current visual line or hard newline `\n`.
- If cursor is already at `end_offset` and followed by `\n`, deletes the `\n` character (joining lines).
- Killed text is stored in `PromptState.kill_ring: Vec<String>` (max capacity 10).
- `Ctrl+Y` (`yank`) pastes the most recently killed string from `kill_ring`.

---

## 7. Input Router Integration Flow

`InputRouter::route_key_event` delegates boundary decisions to `PromptState`:

```rust
// Priority 4: Active Prompt Editor Handling
if state.focus == FocusRegion::Editor {
    match (key.code, key.modifiers) {
        (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            let width = state.prompt_inner_width();
            if state.prompt.can_move_cursor_up(width) {
                return Some(Action::PromptMoveUp);
            } else {
                return Some(Action::RecallPrevious);
            }
        }
        (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            let width = state.prompt_inner_width();
            if state.prompt.can_move_cursor_down(width) {
                return Some(Action::PromptMoveDown);
            } else {
                return Some(Action::RecallNext);
            }
        }
        (KeyCode::Char('a'), KeyModifiers::CONTROL) | (KeyCode::Home, KeyModifiers::NONE) => {
            return Some(Action::PromptHome);
        }
        (KeyCode::Char('e'), KeyModifiers::CONTROL) | (KeyCode::End, KeyModifiers::NONE) => {
            return Some(Action::PromptEnd);
        }
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            return Some(Action::PromptKillLine);
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            return Some(Action::PromptYank);
        }
        _ => {}
    }
}
```

---

## 8. Width Source of Truth & Two-Pass Integration

- **Authoritative Prompt Inner Width**: Derived from `state.terminal_width` in `UiState::prompt_inner_width()`:
  - Total terminal width minus sidebar (if visible) minus prompt container borders/padding (`2` cells).
- **Zero Circular Layout Dependency**: `prompt_inner_width()` depends solely on current viewport allocation; prompt measurement reads `prompt_inner_width()` synchronously without triggering layout cycles.

---

## 9. Atomic Image Attachment Tokens (`[Image #N]`)

Preserve existing atomic chip navigation ([`crates/brain-tui/tests/image_paste_tests.rs`](../../../crates/brain-tui/tests/image_paste_tests.rs)):
- When moving cursor vertically or horizontally, if cursor lands inside `[Image #N]`, automatically snap to token boundary (`image_ref_starting_at` / `image_ref_ending_at`).

---

## 10. Comprehensive Test Strategy Matrix

### Unit Tests (`crates/brain-tui/src/ui/widgets/prompt.rs`):
- `test_wrapped_lines_calculation_multiline_text`: Verify wrapped line offset boundaries for mixed soft wrap + hard `\n`.
- `test_vertical_cursor_navigation_and_clamping`: Verify vertical movement and `preferred_column` retention.
- `test_home_and_end_visual_line_boundaries`: Verify `Ctrl+A` and `Ctrl+E` on line 0 vs line 1.
- `test_kill_line_and_yank`: Verify `Ctrl+K` deletes to visual line end and `Ctrl+Y` yanks.

### Integration Tests (`crates/brain-tui/tests/multiline_prompt_tests.rs`):
- `test_up_key_navigates_lines_before_history_recall`: Verify `Up` key moves cursor up on line 1 and recalls history on line 0.
- `test_down_key_navigates_lines_before_history_next`: Verify `Down` key moves cursor down on line 0 and recalls next on last line.
- `test_multiline_prompt_image_chip_atomic_hopping`: Verify `[Image #N]` tokens are hopped atomically during vertical movement.

---

## 11. Incremental Implementation Plan

- **Step 1**: Implement `VisualLineRange` and `compute_visual_lines` in `crates/brain-tui/src/ui/widgets/prompt.rs`.
- **Step 2**: Add `move_cursor_up`, `move_cursor_down`, `preferred_column` to `PromptState`.
- **Step 3**: Update `Action` enum in `crates/brain-tui/src/state.rs` (`PromptMoveUp`, `PromptMoveDown`, `PromptHome`, `PromptEnd`, `PromptKillLine`, `PromptYank`).
- **Step 4**: Update `InputRouter` key routing in `crates/brain-tui/src/ui/interaction/router.rs`.
- **Step 5**: Implement `Ctrl+A`, `Ctrl+E`, `Ctrl+K`, `Ctrl+Y` in `PromptState`.
- **Step 6**: Add test suite `crates/brain-tui/tests/multiline_prompt_tests.rs`.
- **Step 7**: Validate with `cargo fmt --check` and `cargo test -p brain-tui`.

---

## 12. Architectural Alternatives Analysis

- **Option A (Chosen)**: Minimal 2D Visual Cursor Helpers in `PromptState`. (Cleanest, zero dependencies, exact Claude parity).
- **Option B**: Full external text editor library (e.g. `tui-textarea`). Rejected: Violates zero-dependency guardrail and breaks custom image token rendering.
- **Option C**: Duplicate 2D state in `InputRouter`. Rejected: Pollutes router with rendering concerns.

---

## 13. Evidence Classification

- Claude `BaseTextInput.tsx`, `useTextInput.ts`, `Cursor.ts`: `SOURCE-CONFIRMED`.
- Brain `prompt.rs`, `router.rs` multiline defect: `BRAIN-CONFIRMED`.
- 2D `VisualLineRange` architecture: `INFERRED`.

---

## 14. Final Decision Gate

```text
APPROVED FOR IMPLEMENTATION
```
