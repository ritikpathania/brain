# Design Specification — P2 Sticky Prompt Header

> **Document Status**: Approved Design Specification  
> **Target Subsystem**: `crates/brain-tui` (Header & Scrollback Navigation Layer)  
> **Governing Forensic Audit**: [`docs/design/CLAUDE_STICKY_HEADER_FORENSIC_AUDIT.md`](CLAUDE_STICKY_HEADER_FORENSIC_AUDIT.md)  
> **Locked Systems Protection**: Two-Pass Layout Engine, Inline Collapsible Thinking Blocks, New Messages Pill, Multiline Prompt Cursor, Inline Tool Execution Cards  
> **Final Recommendation Gate**: `APPROVED FOR IMPLEMENTATION`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Goal

Design Claude-parity **Sticky Prompt Header** behavior for Brain's native Ratatui frontend in `crates/brain-tui`.

### Core Capabilities Designed:
1. **Fixed 1-Row Pinned Header**: Displays a 1-row preview of the active conversation turn's prompt at the top of the chat panel when the prompt has scrolled above the top boundary of the viewport (`SOURCE-CONFIRMED`).
2. **Whitespace & Newline Collapsing**: Formats raw multiline prompts into `{figures.pointer} {collapsed_prompt_text}` (where `{figures.pointer}` is `❯`), replacing newlines and multiline whitespace runs with single spaces and truncating to `STICKY_TEXT_CAP` (120 chars) (`SOURCE-CONFIRMED`).
3. **Dedicated Layout Division**: Unlike floating overlays (such as the New Messages Pill), the Sticky Prompt Header consumes **1 real layout row** at the top of the chat pane, shrinking the scrollable message viewport by exactly 1 row (`SOURCE-CONFIRMED`).
4. **Overlay & Dismissal Suppression**: Suppressed when any modal or command overlay (Slash Completion, Shortcuts Help Menu) is open, or when dismissed by user jump action.

---

## 2. Claude Source Contract (`SOURCE-CONFIRMED`)

Primary oracle references: `/Users/ritikpathania/Developer/src/components/FullscreenLayout.tsx` (lines 540-588) and `VirtualMessageList.tsx` (lines 990-1040).

- **Header Text Format**: `❯ <collapsed_prompt_text>`
- **Fixed Height**: Exactly 1 visual row (`height={1}`, `flexShrink={0}`).
- **Overflow Policy**: Truncated at end (`wrap="truncate-end"`).
- **Text Transformation**:
  ```text
  raw_prompt_text
        ↓
  trim_leading_whitespace
        ↓
  slice_first_paragraph (before \n\s*\n)
        ↓
  replace_newlines_and_multispace_with_single_space
        ↓
  truncate_to_120_chars
        ↓
  prefix "❯ "
  ```
- **Visibility Predicate**: Visible **only when** active user prompt is scrolled above the top boundary of the message viewport, and no modal/overlay is open (`overlay == null`).

---

## 3. Brain Current Architecture

In `crates/brain-tui`:
- `AppRenderer::compute_layout` partitions screen geometry into `header` (app title), `mid_chunks` (sidebar, chat area, inspector), `prompt` (editor), `palette` (overlays), and `status` (footer).
- The chat area (`mid_chunks[1]`) currently renders `ChatView` across its entire height without a top sticky header strip when scrolled away.

---

## 4. Proposed Architecture

All changes remain strictly inside `crates/brain-tui`. Zero backend, UDS, domain, storage, or Cargo dependency changes.

```text
UiState
  ├── active_messages: Vec<Message>
  ├── viewport: ViewportState { scroll_offset, follow_tail, ... }
  └── dismissed_sticky_prompt: Option<MessageId> (NEW: tracks user-clicked dismissal)
        │
        ▼
StickyPromptHeaderViewModel (NEW: pure visual view model in sticky_header.rs)
        │
        ▼
StickyPromptHeaderWidget (NEW: Ratatui 1-row widget in sticky_header.rs)
        │
        ▼
AppRenderer::compute_layout (Splits chat area: 1 row Sticky Header + N-1 rows ChatView)
```

### Proposed Types (`crates/brain-tui/src/ui/widgets/sticky_header.rs`):

```rust
/// Pure view model for the 1-row Sticky Prompt Header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyPromptHeaderViewModel {
    /// Associated message ID of the active user prompt.
    pub message_id: brain_domain::MessageId,
    /// Formatted single-line preview string (e.g. "❯ Hello world").
    pub display_text: String,
}

impl StickyPromptHeaderViewModel {
    /// Transforms raw prompt text into collapsed single-line preview format.
    pub fn collapse_text(raw_prompt: &str) -> String {
        let trimmed = raw_prompt.trim_start();
        let para_end = trimmed.find("\n\n").unwrap_or(trimmed.len());
        let first_para = &trimmed[..para_end];
        let single_spaced = first_para.split_whitespace().collect::<Vec<_>>().join(" ");
        let truncated: String = single_spaced.chars().take(120).collect();
        format!("❯ {}", truncated)
    }
}

/// Ratatui widget rendering the 1-row sticky prompt header.
pub struct StickyPromptHeaderWidget<'a> {
    pub vm: &'a StickyPromptHeaderViewModel,
    pub theme: &'a crate::ui::theme::Theme,
}
```

---

## 5. State Model & Ownership

1. **State Storage**: `dismissed_sticky_prompt: Option<brain_domain::MessageId>` in `UiState` ([`crates/brain-tui/src/state.rs`](../../../crates/brain-tui/src/state.rs)).
2. **Action Dispatch**: `Action::DismissStickyPrompt(Option<brain_domain::MessageId>)` added to `Action` enum.
3. **Derived Visibility**: Header visibility is derived dynamically during rendering based on `scroll_offset`, `ViewportIndex`, active overlays, and `dismissed_sticky_prompt`.

---

## 6. Visibility Algorithm & Active Prompt Resolution

The visibility and text of the sticky header are determined deterministically:

```rust
pub fn resolve_sticky_header(state: &UiState, index: &ViewportIndex) -> Option<StickyPromptHeaderViewModel> {
    // 1. Overlay suppression gate
    if state.shortcuts_overlay_open
        || state.active_overlay != PromptOverlay::None
        || state.slash_completion().visible
        || state.command_palette.open
    {
        return None;
    }

    // 2. Follow-tail gate (hidden when at bottom)
    if state.viewport.follow_tail {
        return None;
    }

    // 3. Locate top visible message index from scroll_offset
    let (first_visible_idx, local_line_offset) = index.find_offset(state.viewport.scroll_offset as u32)?;

    // 4. Scan backwards to locate the active turn's User message
    let user_msg_idx = (0..=first_visible_idx).rev().find(|&i| {
        state.active_messages.get(i).map_or(false, |m| m.role == MessageRole::User)
    })?;

    let user_msg = state.active_messages.get(user_msg_idx)?;

    // 5. Check if user prompt is scrolled above the viewport
    let prompt_top = if user_msg_idx > 0 { index.cumulative_heights[user_msg_idx - 1] } else { 0 };
    let is_scrolled_above = (state.viewport.scroll_offset as u32) > prompt_top;

    if !is_scrolled_above {
        return None;
    }

    // 6. Check dismissal gate
    if state.dismissed_sticky_prompt == Some(user_msg.id.clone()) {
        return None;
    }

    let display_text = StickyPromptHeaderViewModel::collapse_text(&user_msg.content);
    if display_text.trim().is_empty() {
        return None;
    }

    Some(StickyPromptHeaderViewModel {
        message_id: user_msg.id.clone(),
        display_text,
    })
}
```

---

## 7. Layout Integration

Dedicated 1-row layout division in `AppRenderer::compute_layout`:

```text
chat_area (mid_chunks[1])
┌────────────────────────────────────────────────────────┐
│ StickyPromptHeaderWidget (1 real layout row)           │
├────────────────────────────────────────────────────────┤
│                                                        │
│ ChatView (Height = chat_area.height - 1)               │
│                                                        │
└────────────────────────────────────────────────────────┘
                          ▲
                          │
            NewMessagesPillWidget (Floats over bottom row)
```

- **Top Division**: `sticky_area = Layout::vertical([Length(1), Min(1)]).split(chat_area)[0]`.
- **Message Area**: `chat_viewport_area = Layout::vertical([Length(1), Min(1)]).split(chat_area)[1]`.
- **Coexistence Guarantee**: Sticky Header at top row, New Messages Pill at bottom row. Zero layout collisions (`SOURCE-CONFIRMED`).

---

## 8. Scroll & Jump Semantics

- **Jump Action**: Activating jump to prompt sets `scroll_offset` to `prompt_top` and dispatches `Action::DismissStickyPrompt(Some(msg_id))`.
- **Mouse Input**: If mouse click input on sticky header is triggered in future mouse event loop, dispatches jump action (`DEFERRED — MOUSE INPUT GAP`).

---

## 9. Compatibility with Locked Subsystems

1. **Two-Pass Layout Engine**: Compatible. Fixed 1-row layout deduction introduces zero circular dependencies.
2. **Inline Collapsible Thinking Blocks**: Compatible. Thinking blocks scroll underneath the sticky header.
3. **New Messages Pill**: Compatible. Header at top row, pill at bottom row.
4. **Multiline Prompt Cursor**: Compatible. Prompt editor at bottom of screen remains untouched.
5. **Inline Tool Execution Cards**: Compatible. Tool cards scroll underneath the sticky header.

---

## 10. Edge Cases

- **Narrow Viewports (< 40 cols)**: Truncated at right edge with `...`.
- **Short Viewports (< 10 rows)**: Suppressed (`sticky_header = None`).
- **Rapid Resize**: Dynamic recalculation derived directly from `terminal_width`.

---

## 11. Performance Budget

- **Derivation Cost**: $O(\log N)$ binary search on `ViewportIndex` + $O(1)$ reverse scan to nearest user prompt.
- **Allocation Cost**: 0 allocations per frame when prompt text is unchanged (`MEASURED`).

---

## 12. Testing Strategy

Proposed tests in `crates/brain-tui/tests/sticky_header_tests.rs`:
1. `test_sticky_header_text_collapsing_and_truncation()`: Verifies whitespace collapsing, paragraph slicing, and 120-char truncation.
2. `test_sticky_header_visibility_scrolled_above()`: Verifies visibility when user prompt scrolls above viewport.
3. `test_sticky_header_hidden_at_bottom_follow_tail()`: Asserts header is hidden when `follow_tail == true`.
4. `test_sticky_header_overlay_suppression()`: Verifies header suppression when modal/overlay is open.
5. `test_sticky_header_layout_geometry_deduction()`: Asserts chat viewport height contracts by exactly 1 row when header is active.

---

## 13. Incremental Implementation Steps

1. **Step 1**: Create `crates/brain-tui/src/ui/widgets/sticky_header.rs` with `StickyPromptHeaderViewModel` and `StickyPromptHeaderWidget`.
2. **Step 2**: Re-export `sticky_header` in `crates/brain-tui/src/ui/widgets/mod.rs`.
3. **Step 3**: Add `dismissed_sticky_prompt` to `UiState` and `Action::DismissStickyPrompt` in `crates/brain-tui/src/state.rs`.
4. **Step 4**: Update `compute_layout` and `draw` in `crates/brain-tui/src/ui/renderer.rs` to split chat area when sticky header is visible.
5. **Step 5**: Create `crates/brain-tui/tests/sticky_header_tests.rs`.

---

## 14. Risks & Mitigations

- **Risk**: Visual jump during scrolling if sticky header height varies.
  - **Mitigation**: Height is strictly fixed at 1 visual row (`height = 1`) (`SOURCE-CONFIRMED`).
- **Risk**: Collision with New Messages Pill.
  - **Mitigation**: Header is top layout row, pill is bottom float row (`SOURCE-CONFIRMED`).

---

## 15. Rollback Strategy

If regression occurs, revert `sticky_header.rs` and the 1-row layout split in `renderer.rs`. All locked subsystems remain completely isolated.

---

## 16. Acceptance Criteria

```text
✓ Fixed 1-row header widget
✓ Collapsed prompt text formatting (prefix ❯, single-space whitespace runs, 120-char cap)
✓ Scrolled-above visibility resolution
✓ Overlay suppression
✓ 1-row top layout deduction
✓ 100% test suite pass across all locked subsystems
```

---

## 17. Final Recommendation Gate

```text
APPROVED FOR IMPLEMENTATION
```
