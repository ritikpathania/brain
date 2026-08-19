# Implementation Report — P2 Sticky Prompt Header

> **Document Status**: Complete Implementation Report  
> **Target Subsystem**: `crates/brain-tui` (Header & Scrollback Navigation Layer)  
> **Governing Design**: [`docs/design/STICKY_HEADER_DESIGN.md`](STICKY_HEADER_DESIGN.md)  
> **Forensic Audit Reference**: [`docs/design/CLAUDE_STICKY_HEADER_FORENSIC_AUDIT.md`](CLAUDE_STICKY_HEADER_FORENSIC_AUDIT.md)  
> **Implementation Result**: `COMPLETE AND VERIFIED`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Summary

The **P2 Sticky Prompt Header** feature has been successfully implemented in `crates/brain-tui`.

### Core Capabilities Implemented:
1. **`StickyPromptHeaderWidget` & Presentation View Model**:
   - Created `StickyPromptHeaderViewModel` and `StickyPromptHeaderWidget` in [`crates/brain-tui/src/ui/widgets/sticky_header.rs`](../../../crates/brain-tui/src/ui/widgets/sticky_header.rs).
   - Formats raw multiline prompts into `{figures.pointer} {collapsed_prompt_text}` (where `{figures.pointer}` is `❯`), replacing newlines and multiline whitespace runs with single spaces and truncating to `STICKY_TEXT_CAP` (120 chars) (`SOURCE-CONFIRMED`).
2. **Deterministic Visibility Resolver**:
   - Implemented `AppRenderer::resolve_sticky_header(state, index)` in [`crates/brain-tui/src/ui/renderer.rs`](../../../crates/brain-tui/src/ui/renderer.rs).
   - Resolves active prompt position via $O(\log N)$ binary search on `ViewportIndex` and reverse scan for the active `MessageRole::User` message.
   - Header is visible **only when** `scroll_offset > prompt_top` and `follow_tail == false` (`SOURCE-CONFIRMED`).
3. **Dedicated 1-Row Layout Division**:
   - Unlike floating overlays, the Sticky Prompt Header consumes **1 real layout row** at the top of the chat panel (`mid_chunks[1]`), reducing the scrollable `ChatView` height by exactly 1 row (`SOURCE-CONFIRMED`).
4. **State & Dismissal Action**:
   - Added `dismissed_sticky_prompt: Option<MessageId>` to `UiState` and `Action::DismissStickyPrompt` reducer in [`crates/brain-tui/src/state.rs`](../../../crates/brain-tui/src/state.rs).

---

## 2. Files Changed

- [`crates/brain-tui/src/ui/widgets/sticky_header.rs`](../../../crates/brain-tui/src/ui/widgets/sticky_header.rs) (`[NEW]`): Created view model and widget for Sticky Prompt Header.
- [`crates/brain-tui/src/ui/widgets/mod.rs`](../../../crates/brain-tui/src/ui/widgets/mod.rs) (`[MODIFY]`): Re-exported `sticky_header`.
- [`crates/brain-tui/src/state.rs`](../../../crates/brain-tui/src/state.rs) (`[MODIFY]`): Added `dismissed_sticky_prompt` to `UiState` and `Action::DismissStickyPrompt` to `Action` enum.
- [`crates/brain-tui/src/ui/renderer.rs`](../../../crates/brain-tui/src/ui/renderer.rs) (`[MODIFY]`): Implemented `resolve_sticky_header` and 1-row layout split in `AppRenderer::draw`.
- [`crates/brain-tui/tests/sticky_header_tests.rs`](../../../crates/brain-tui/tests/sticky_header_tests.rs) (`[NEW]`): Added 6 unit & integration tests.

---

## 3. Architecture & Data Flow

```text
UiState (scroll_offset, active_messages, dismissed_sticky_prompt)
        │
        ▼
ViewportIndex (cumulative_heights of message blocks)
        │
        ▼
AppRenderer::resolve_sticky_header
        ├── 1. Overlay / Palette Check (Suppressed if open)
        ├── 2. Follow-tail Check (Suppressed if true)
        ├── 3. ViewportIndex::find_offset(scroll_offset)
        ├── 4. Backwards scan for MessageRole::User
        ├── 5. Check if scroll_offset > prompt_top
        └── 6. Check dismissal state
        │
        ▼
Some(StickyPromptHeaderViewModel)
        │
        ▼
AppRenderer::draw (Splits chat_area vertically: 1 row Header + N-1 rows ChatView)
```

---

## 4. Claude Source Contracts Implemented (`SOURCE-CONFIRMED`)

- **Format**: `❯ <collapsed_prompt_text>` (`SOURCE-CONFIRMED`).
- **Fixed Height**: Fixed at 1 visual row (`height = 1`) (`SOURCE-CONFIRMED`).
- **Whitespace Collapsing**: Multiline prompts, newlines, and space runs collapsed to single space runs (`SOURCE-CONFIRMED`).
- **Text Truncation**: Truncated to `STICKY_TEXT_CAP` (120 chars) (`SOURCE-CONFIRMED`).
- **Visibility Gate**: Visible only when user prompt has scrolled off-screen above viewport (`SOURCE-CONFIRMED`).
- **Overlay Suppression**: Suppressed when command palette, slash menu, or shortcuts help overlay is open (`SOURCE-CONFIRMED`).

---

## 5. Compatibility with Locked Subsystems

1. **Two-Pass Layout Engine**: 100% untouched and locked (`VERIFIED`). Fixed 1-row layout deduction introduces zero circular dependencies.
2. **Inline Collapsible Thinking Blocks**: 100% untouched and locked (`VERIFIED`). Thinking blocks scroll underneath the sticky header.
3. **New Messages Pill**: 100% untouched and locked (`VERIFIED`). Header is at the **top row** of the chat viewport; New Messages Pill is anchored at the **bottom row**. Zero spatial or z-order collisions.
4. **Multiline Prompt Cursor**: 100% untouched and locked (`VERIFIED`). Bottom prompt editor remains completely independent.
5. **Inline Tool Execution Cards**: 100% untouched and locked (`VERIFIED`). Tool cards scroll underneath the sticky header.

---

## 6. Verification & Test Results

### 1. Formatting Audit:
```bash
cargo fmt --check
# Exit code 0 (Passes cleanly)
```

### 2. New Test Suite (`sticky_header_tests.rs`):
```text
running 6 tests
test test_sticky_header_dismissal_action ... ok
test test_sticky_header_hidden_at_bottom_or_prompt_visible ... ok
test test_sticky_header_overlay_suppression ... ok
test test_sticky_header_text_collapsing_and_truncation ... ok
test test_sticky_header_visibility_resolution_scrolled_above ... ok
test test_sticky_header_widget_rendering ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 3. Full `brain-tui` Test Suite:
```text
test result: ok. 100 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 7. Scope & Diff Audit

- Backend crates (`brain-domain`, `brain-services`, `brain-storage`): **0 changes** (`VERIFIED`).
- Cargo manifests / dependencies (`Cargo.toml`, `Cargo.lock`): **0 changes** (`VERIFIED`).
- Locked subsystems: **0 changes / untouched** (`VERIFIED`).

---

## 8. Remaining Non-Blocking Gaps

1. **Mouse Click Handler**: Mouse click input on sticky header (to jump viewport back to prompt) is deferred until a unified terminal mouse event router is introduced (`DEFERRED — MOUSE INPUT GAP`).

---

## 9. Implementation Status

```text
COMPLETE AND VERIFIED
```
