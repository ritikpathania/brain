# Final Certification — New Messages / Scroll-to-Bottom Pill

```text
NEW MESSAGES PILL
STATUS: LOCKED
CERTIFICATION: PASS WITH NON-BLOCKING GAPS
```

---

## 1. Governing Artifacts

- **Governing Design**: [`docs/design/NEW_MESSAGES_PILL_DESIGN.md`](NEW_MESSAGES_PILL_DESIGN.md)
- **Implementation Report**: [`docs/design/NEW_MESSAGES_PILL_IMPLEMENTATION_REPORT.md`](NEW_MESSAGES_PILL_IMPLEMENTATION_REPORT.md)
- **Independent Final Audit**: [`docs/design/NEW_MESSAGES_PILL_FINAL_AUDIT.md`](NEW_MESSAGES_PILL_FINAL_AUDIT.md)
- **Oracle Source References**: `/Users/ritikpathania/Developer/src/components/FullscreenLayout.tsx` (`NewMessagesPill`, `countUnseenAssistantTurns`, `computeUnseenDivider`)

---

## 2. Final Certification Summary

- **Final Status**: `PASS WITH NON-BLOCKING GAPS`
- **Subsystem State**: **LOCKED**

The **Floating "Scroll to Bottom / New Messages" Pill Indicator** (`NewMessagesPillWidget`) subsystem is officially certified and locked.

---

## 3. Files Changed

- [`crates/brain-tui/src/ui/widgets/new_messages_pill.rs`](../../../crates/brain-tui/src/ui/widgets/new_messages_pill.rs) (`[NEW]`)
- [`crates/brain-tui/src/ui/widgets/mod.rs`](../../../crates/brain-tui/src/ui/widgets/mod.rs) (`[MODIFY]`)
- [`crates/brain-tui/src/state.rs`](../../../crates/brain-tui/src/state.rs) (`[MODIFY]`)
- [`crates/brain-tui/src/ui/renderer.rs`](../../../crates/brain-tui/src/ui/renderer.rs) (`[MODIFY]`)
- [`crates/brain-tui/tests/new_messages_pill_tests.rs`](../../../crates/brain-tui/tests/new_messages_pill_tests.rs) (`[NEW]`)

---

## 4. Architecture Boundaries

- **Frontend Subsystem**: All changes strictly confined to `crates/brain-tui`.
- **Backend Subsystems**: Zero changes to `brain-domain`, `brain-services`, `brain-storage`, `brain-core`, or `brain-events`.
- **Dependencies**: Zero changes to `Cargo.toml` or `Cargo.lock` (0 external dependencies added).
- **Core Systems**: Zero modifications to ADR-001 or the Two-Pass Layout engine architecture.
- **Previous Subsystems**: Inline Collapsible Thinking Blocks subsystem remains untouched and locked.

---

## 5. Verification & Test Results

- `cargo fmt --check`: Passed (0 formatting differences).
- `cargo test -p brain-tui`: **97 brain-tui test suites passed** (0 failed, 0 ignored).

---

## 6. Deferred Non-Blocking Gaps

1. **Mouse Click Activation**: Direct mouse click on the floating pill in terminal emulators requires TUI mouse event tracking enablement. Keyboard / action re-pinning (`Action::JumpToBottom`) is currently used. This is explicitly deferred as a non-blocking future enhancement.

---

## 7. Lock Statement

> The **Floating "Scroll to Bottom / New Messages" Pill Indicator** subsystem is **LOCKED**. No further code, test, or design modifications are permitted for this subsystem without a verified regression.
