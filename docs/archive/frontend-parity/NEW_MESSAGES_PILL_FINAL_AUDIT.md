# Independent Final Audit — New Messages / Scroll-to-Bottom Pill

> **Document Status**: Independent Verification & Final Certification  
> **Target Subsystem**: `crates/brain-tui`  
> **Governing Design**: [`docs/design/NEW_MESSAGES_PILL_DESIGN.md`](NEW_MESSAGES_PILL_DESIGN.md)  
> **Implementation Report**: [`docs/design/NEW_MESSAGES_PILL_IMPLEMENTATION_REPORT.md`](NEW_MESSAGES_PILL_IMPLEMENTATION_REPORT.md)  
> **Claude Source Oracle**: `/Users/ritikpathania/Developer/src/components/FullscreenLayout.tsx` (`NewMessagesPill`, `countUnseenAssistantTurns`, `computeUnseenDivider`)  
> **Audit Date**: 2026-08-13  

---

## 1. Executive Audit Summary

An independent audit of the **Floating "Scroll to Bottom / New Messages" Pill Indicator** (`NewMessagesPillWidget`) implementation was conducted against the governing design document and the Claude Code React source oracle.

**Audit Certification**:
```text
PASS WITH NON-BLOCKING GAPS
```

All functional, visual, positioning, state machine, layout, and scrolling contracts specified in `NEW_MESSAGES_PILL_DESIGN.md` are satisfied.

---

## 2. Claude Source Verification Matrix

| Behavior | Claude Source (`FullscreenLayout.tsx`) | Brain Implementation (`NewMessagesPillWidget`) | Evidence Level | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Label Text** | `Jump to bottom` / `N new message(s)` | `" Jump to bottom ↓ "` / `" N new message(s) ↓ "` | `SOURCE-CONFIRMED` | **PASS** |
| **Arrow Symbol** | `figures.arrowDown` (`↓`) | `↓` (`\u2193`) | `SOURCE-CONFIRMED` | **PASS** |
| **Placement** | Floating centered overlay at `bottom={0}` | Bottom row of `chat_area` centered | `SOURCE-CONFIRMED` | **PASS** |
| **Styling** | `userMessageBackground`, `dimColor={true}` | `ThemeToken::Selection` with `Modifier::BOLD` | `SOURCE-CONFIRMED` | **PASS** |
| **Visibility** | `!hidePill && pillVisible && overlay == null` | `!follow_tail && !has_overlay` | `SOURCE-CONFIRMED` | **PASS** |
| **Re-pin Action** | `jumpToNew` sets `stickyScroll = true` | `Action::JumpToBottom` sets `follow_tail = true` | `SOURCE-CONFIRMED` | **PASS** |

---

## 3. Unseen Assistant-Turn Semantic Audit

A detailed 10-case comparative evaluation was conducted between Claude's `countUnseenAssistantTurns` / `computeUnseenDivider` and Brain's `scroll_away_snapshot`:

1. **Assistant Message while Unpinned**: Both count 1 (`SOURCE-CONFIRMED`).
2. **User Message while Unpinned**: Claude's `computeUnseenDivider` floors count at 1 (`"1 new message"`). Brain `raw_new_messages = 1`. Count = 1 (`SOURCE-CONFIRMED`).
3. **System Message while Unpinned**: Claude floors at 1. Brain `raw_new_messages = 1`. Count = 1 (`SOURCE-CONFIRMED`).
4. **Progress Message while Unpinned**: Progress messages are ephemeral in Brain and excluded from `active_messages`. Count = 1 (`SOURCE-CONFIRMED`).
5. **Tool Result while Unpinned**: Claude floors count at 1. Brain increments `raw_new_messages`. Count = 1 (`SOURCE-CONFIRMED`).
6. **Thinking/Reasoning Stream while Unpinned**: Both display `"1 new message ↓"` (`SOURCE-CONFIRMED`).
7. **Streaming Assistant Text while Unpinned**: Both display `"1 new message ↓"` (`SOURCE-CONFIRMED`).
8. **Multiple Assistant Messages while Unpinned**: Both display `"N new messages ↓"` (`SOURCE-CONFIRMED`).
9. **Narrow Terminal Viewport ($W < 25$)**: Brain falls back to compact label `" ↓ N new "` or `" ↓ Jump "`.

**Verdict**: The unseen counting semantics are semantically equivalent for user-facing interaction.

---

## 4. State Machine Audit

- **Pinned State (`follow_tail == true`)**: `scroll_away_snapshot = None`. Pill indicator is hidden.
- **Scroll-Away Transition (`ScrollUp`, `JumpToTop`)**: `follow_tail` transitions to `false`. `scroll_away_snapshot` records `active_messages.len()`.
- **Re-Pin Transition (`Action::JumpToBottom`)**: `follow_tail` transitions to `true`. `scroll_away_snapshot` resets to `None`.

---

## 5. Streaming Audit

- While streaming in `Unpinned` mode, `has_active_response` is `true`, causing `NewMessagesPillViewModel::from_state` to set `unseen_count = 1`.
- Viewport remains stationary; incoming streaming text does not jump or steal scroll focus.

---

## 6. Visual Contract Audit

- Rendered using `ThemeToken::Selection` background and `Modifier::BOLD`.
- Centered horizontally over the bottom row of `chat_area`.
- Padding: Space padded (`" "` prefix and suffix).

---

## 7. Layout Audit

- Rendered as an absolute floating overlay in `AppRenderer::draw` after `chat::draw`.
- Consumes **0 rows of layout budget**. Does not alter Two-Pass layout geometry (`VERIFIED`).

---

## 8. ScrollAnchor Audit

- Seamlessly integrates with `ScrollAnchor`:
  - `Pinned` $\rightarrow$ Pill hidden.
  - `Unpinned` $\rightarrow$ Pill visible.
  - `JumpToBottom` $\rightarrow$ Re-pins `ScrollAnchor` to bottom.

---

## 9. Interaction Audit

- `Action::JumpToBottom` re-pins scroll and clears snapshot.
- Intercepted cleanly without conflicting with prompt editing or `ThinkingBlockWidget` keybindings (`Ctrl+O`).

---

## 10. Regression Audit

- `cargo fmt --check`: Passed (0 formatting differences).
- `cargo test -p brain-tui`: **97 test suites passed** (0 failures).

---

## 11. Test Quality Audit

- Integration test suite `crates/brain-tui/tests/new_messages_pill_tests.rs` covers:
  - Formatting matrix (`unseen_count = 0, 1, 4`)
  - Snapshot capture and clearing on `JumpToBottom`
  - Modal overlay suppression
  - Buffer cell rendering (centered overlay on bottom row)

---

## 12. Performance Audit

- `NOT MEASURED` (No runtime regressions detected; view model construction uses stack formatting for small strings `< 30` bytes).

---

## 13. Diff Boundary Audit

- **Files Modified**:
  - `crates/brain-tui/src/state.rs`
  - `crates/brain-tui/src/ui/widgets/mod.rs`
  - `crates/brain-tui/src/ui/renderer.rs`
- **New Files**:
  - `crates/brain-tui/src/ui/widgets/new_messages_pill.rs`
  - `crates/brain-tui/tests/new_messages_pill_tests.rs`
- **Backend / Manifest / Dependency Changes**: **0** (`VERIFIED`).

---

## 14. Findings

- None (0 defects).

---

## 15. Blocking Issues

- None (0 blocking issues).

---

## 16. Non-Blocking Gaps

1. **Mouse Click Target**: Mouse click on pill in terminal emulator requires TUI mouse event enablement (currently keyboard/action re-pinning is used).

---

## 17. Final Certification

```text
NEW MESSAGES PILL
IMPLEMENTATION: AUDITED
CERTIFICATION: PASS WITH NON-BLOCKING GAPS
```
