# Ratatui TUI Client Migration (Milestone 7: Parity Validation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Perform comprehensive parity checks, behavioral audits, stress testing, and legacy removal audits for the native Ratatui TUI client.

**Categories:**
1. **Feature Parity**: Verify every feature from the legacy client exists and operates correctly (scrolling, multiline editing, streaming, cancellation, history, sessions, resizing, markdown, shortcuts).
2. **Behavioral Parity**: Verify operational behavior behaves matching user expectations (typing latency, resize anchors, scroll anchoring, typewriter pacing, cursor movements, submission timing).
3. **Stress Testing**: Run extreme scenarios to verify safety under pressure (long threads, massive streams, repeated switches, resize spam, cancel spam, huge histories).
4. **Legacy Removal & Dependency Graph Audit**: Check that no active runtime paths depend on Node/Bun/Ink/React, and verify that no Rust workspace crates retain dependencies on legacy compatibility layers or deprecated client modules.

---

## Exit Checklist (Completion Criteria)
- [ ] **Criteria 1**: All parity tests pass cleanly.
- [ ] **Criteria 2**: All stress tests pass cleanly.
- [ ] **Criteria 3**: Zero Clippy warnings or Rust lints in presentation crates.
- [ ] **Criteria 4**: No active runtime dependencies on Bun, Node, Ink, or React.
- [ ] **Criteria 5**: No Rust workspace crates retain dependencies on legacy compatibility wrappers.
- [ ] **Criteria 6**: All user documentation is updated to reference the new TUI client.

---

## Baseline Performance Metrics
We will measure and log the following baseline statistics inside our final report:
- **Cold Startup Time**: Time elapsed from binary execution to Alternate Screen rendering.
- **Idle Memory Footprint**: RSS memory usage of the client process while idle.
- **Render Latency**: Frame redraw commit time (measured via profiler/logs).
- **Streaming Latency**: Input token delivery and typewriter queue pacing latency.
- **Session Switch Latency**: Database query and history reload completion time.

---

### Task 1: Comprehensive Parity and Stress Testing

**Files:**
- Modify: `crates/brain-tui/tests/parity_tests.rs` (new integration test file)

**Interfaces:**
- Validates: Layout dimensions, keyboard inputs, and rendering transitions under stress.

- [ ] **Step 1: Write Parity Test Suite**
  Create `crates/brain-tui/tests/parity_tests.rs` with automated tests that:
  - Verify layout partitions (sidebar, header, chat, prompt, status) at varied boundaries.
  - Verify scroll offset limits and anchor properties.
  - Stress test repeated fast-frequency session switching (100 switches).
  - Stress test rapid size updates (100 resizes).
  - Stress test cancellation spam (sending tokens and cancels in rapid succession).
- [ ] **Step 2: Execute stress tests**
  Run `cargo test -p brain-tui --test parity_tests` and ensure all tests pass.
- [ ] **Step 3: Run clippy checks**
  Verify clean clippy checks on the test package.
- [ ] **Step 4: Commit**
  Commit Task 1: `git add . && git commit -m "test(tui): add comprehensive parity and stress test suite"`

---

### Task 2: Manual Parity Checklist & Legacy Removal Audit

**Files:**
- Create: `docs/migration/parity_audit_report.md`

- [ ] **Step 1: Conduct Manual Parity Checklist Audit**
  Perform manual checks for typing latency, scrolling widgets, cursor boundaries, and markdown rendering. Log the explicit pass/fail checklist in `parity_audit_report.md`.
- [ ] **Step 2: Conduct Legacy Removal Pre-Audit**
  Scan workspace to ensure no runtime execution flows require Bun/Node/Ink/React, verifying that all cargo dependencies on native Rust crates are intact.
- [ ] **Step 3: Update documentation**
  Add walkthrough details and final migration notes.
- [ ] **Step 4: Commit**
  Commit Task 2: `git add . && git commit -m "docs(tui): complete manual parity check list and legacy removal audit report"`
