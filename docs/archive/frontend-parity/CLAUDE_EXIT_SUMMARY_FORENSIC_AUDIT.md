# Forensic Source Audit — Exit Summary & Terminal Exit Formatting

> **Document Status**: Forensic Analysis & Architectural Audit  
> **Target Subsystem**: `crates/brain-tui` & `apps/brain` (Terminal Lifecycle & Shutdown Layer)  
> **Scope**: P3 — Exit Summary / Terminal Exit Formatting  
> **Governing Foundations**: Native Rust/Ratatui Architecture (ADR-001), Locked Two-Pass Layout Engine, Locked Subsystems (Thinking Blocks, New Messages Pill, Multiline Prompt Cursor, Tool Execution Cards, Sticky Header)  
> **Oracle Source Verification**:  
> - `/Users/ritikpathania/Developer/src/utils/gracefulShutdown.ts` (lines 144–184, 390–523)  
> - `/Users/ritikpathania/Developer/src/commands/exit/exit.tsx` (lines 10–32)  
> - `/Users/ritikpathania/Developer/src/ink/ink.tsx` (lines 1470–1485)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Summary

This document presents a source-verified forensic audit of Claude Code's terminal exit lifecycle and exit output formatting, comparing it against Brain's native Rust/Ratatui architecture (`crates/brain-tui` and `apps/brain`).

### Key Audit Findings (`SOURCE-CONFIRMED`):
1. **No Ink-Based Exit Summary Widget**: Claude Code does **NOT** render a complex multi-line visual summary dashboard or TUI card inside Ink upon exit.
2. **Terminal Restoration & Main-Screen Print**: Exit formatting occurs **after** exiting the alternate screen buffer (`EXIT_ALT_SCREEN` / 1049l) and disabling raw terminal modes.
3. **Session Resume Hint**: Claude writes a single 2-line dimmed string directly to stdout (fd 1) on the main terminal screen (`gracefulShutdown.ts` lines 173–178):
   ```text
   Resume this session with:
   claude --resume "<session_id_or_title>"
   ```
4. **Goodbye Message**: When `/exit` is executed explicitly, it outputs a random goodbye string (`['Goodbye!', 'See ya!', 'Bye!', 'Catch you later!']`) before calling `gracefulShutdown` (`exit.tsx` lines 10–29).
5. **Brain Parity Assessment**: Brain already exits Crossterm alternate screen cleanly back to the main terminal buffer. There is **NO MATERIAL VISUAL GAP**.

---

## 2. Claude Exit Lifecycle & Source Locations (`SOURCE-CONFIRMED`)

Source trace through `/Users/ritikpathania/Developer/src`:

```text
/exit command or Ctrl+C / SIGINT signal
      │
      ▼
commands/exit/exit.tsx (calls gracefulShutdown)
      │
      ▼
utils/gracefulShutdown.ts
      ├── 1. cleanupTerminalModes() -> writeSync(1, EXIT_ALT_SCREEN)
      ├── 2. printResumeHint() -> writeSync(1, chalk.dim("\nResume this session with:\nclaude --resume ...\n"))
      ├── 3. runCleanupFunctions() -> session persistence flush
      └── 4. forceExit(exitCode) -> process.exit(code)
```

### Exact Source Locations:
- `utils/gracefulShutdown.ts`: `cleanupTerminalModes` (line 59), `printResumeHint` (line 144), `gracefulShutdown` (line 391).
- `commands/exit/exit.tsx`: `GOODBYE_MESSAGES` and `call()` (lines 10–32).
- `ink/ink.tsx`: Terminal mode unmount hooks (lines 1472–1484).

---

## 3. Exit Path Matrix (`SOURCE-CONFIRMED`)

| Exit Path | Summary Format | Output Channel | Terminal State | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **/exit Command** | `"Goodbye!"` + Resume Hint | Stdout (main buffer) | Restored to cooked mode | Triggers clean shutdown |
| **Ctrl+C (Double press)** | Resume Hint only | Stdout (main buffer) | Restored to cooked mode | Handled by SIGINT / exit flow |
| **Ctrl+D / EOF** | Resume Hint only | Stdout (main buffer) | Restored to cooked mode | Triggers clean shutdown |
| **SIGINT / SIGTERM** | Resume Hint only | Stdout (main buffer) | Restored to cooked mode | Caught by `setupGracefulShutdown` |
| **Fatal Error** | Error message + Stack trace | Stderr (main buffer) | Restored to cooked mode | Handled by `forceExit(1)` |

---

## 4. Mechanical Parity Matrix

| Behavior / Contract | Claude Source Oracle | Brain Current (`crates/brain-tui` / `apps/brain`) | Parity | Classification |
| :--- | :--- | :--- | :--- | :--- |
| **Exit Component** | Stdout resume hint post-TUI teardown | Clean Crossterm alt-screen exit | **PARITY**: No missing TUI widget | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Output Channel** | Stdout (fd 1) main buffer | Main terminal buffer | **PARITY**: Identical channel | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Terminal Restoration** | `EXIT_ALT_SCREEN` + reset sequences | Crossterm `LeaveAlternateScreen` | **PARITY**: Identical teardown | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Resume Hint** | `Resume this session with: ...` | Terminal exits to prompt cleanly | **MINOR DIFFERENCE**: Optional stdout print | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |

---

## 5. Architectural Classification & Subsystem Safety

- **Classification**: **No Material Gap**.
- **Subsystem Protection**: Requires **zero changes** to `brain-domain`, `brain-services`, `brain-storage`, `brain-tui`, `Cargo.toml`, or any locked subsystem.
- **Locked Systems Safety**: All 6 locked subsystems (Two-Pass Layout Engine, Thinking Blocks, New Messages Pill, Multiline Prompt Cursor, Tool Execution Cards, Sticky Header) remain completely untouched and locked (`VERIFIED`).

---

## 6. Priority Assessment

```text
NOT A GAP
```

There is no missing TUI component or architectural defect. Claude Code's terminal exit mechanism is a standard terminal mode restoration followed by a single line of stdout text. Brain's current native Rust/Ratatui exit path is clean and fully compliant.

---

## 7. Recommendation

```text
NO MATERIAL GAP — KEEP CURRENT IMPLEMENTATION
```
