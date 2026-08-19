# Forensic Source Audit — Inline Tool Execution Cards & Collapsible Result Drawers

> **Document Status**: Forensic Analysis & Architectural Audit  
> **Target Subsystem**: `crates/brain-tui` (Tool Execution Presentation & Interaction Layer)  
> **Scope**: Inline Tool Execution Cards, Tool Call Lifecycles, Collapsible Output Drawers, Truncation & Expansion Semantics  
> **Governing Foundations**: Native Rust/Ratatui Architecture (ADR-001), Locked Two-Pass Layout Engine, Locked `ThinkingBlockWidget`, Locked `NewMessagesPillWidget`, Locked Multiline Prompt Cursor  
> **Oracle Source Verification**:  
> - `/Users/ritikpathania/Developer/src/components/messages/AssistantToolUseMessage.tsx`  
> - `/Users/ritikpathania/Developer/src/components/messages/UserToolResultMessage/UserToolResultMessage.tsx`  
> - `/Users/ritikpathania/Developer/src/components/messages/UserToolResultMessage/UserToolSuccessMessage.tsx`  
> - `/Users/ritikpathania/Developer/src/components/messages/UserToolResultMessage/UserToolErrorMessage.tsx`  
> - `/Users/ritikpathania/Developer/src/components/messages/CollapsedReadSearchContent.tsx`  
> - `/Users/ritikpathania/Developer/src/Tool.ts`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

## 1. Executive Summary

This document presents a source-verified forensic audit of Claude Code's tool execution rendering architecture (`AssistantToolUseMessage.tsx`, `UserToolResultMessage.tsx`, `CollapsedReadSearchContent.tsx`) and compares it against Brain's native Ratatui frontend (`crates/brain-tui`).

### Primary Audit Conclusion:
Claude Code presents tool calls as **inline structured execution cards** with a two-tier presentation model:
1. **Header / Progress Line**: Compact bullet (`⏺`), bold tool name (e.g. `FileRead`, `Bash`), tool arguments (e.g. `(crates/brain-tui/src/state.rs)`), and live status indicator (`ToolUseLoader` spinner while running, `✔` on success, `✖` on error/rejection).
2. **Collapsible Result Drawer**: Renders tool output. Defaults to collapsed (showing a single summary line or preview) with a `(ctrl+o to expand)` hint. When expanded (`Ctrl+O`), it displays the full multiline output or code block (`SOURCE-CONFIRMED`).

Brain currently maintains tool execution data models (`ToolExecution`, `ToolApproval` in `crates/brain-tui/src/ui/command/tool.rs`), but lacks a dedicated **collapsible tool execution card widget** (`ToolExecutionCardWidget`) with `Ctrl+O` toggle integration, status icons, and output truncation boundaries (`BRAIN-CONFIRMED`).

Ratatui is **100% sufficient** to implement this capability without any backend, UDS, domain, or dependency changes.

---

## 2. Claude Component Architecture & Oracle Trace (`SOURCE-CONFIRMED`)

Source trace through `/Users/ritikpathania/Developer/src`:

```text
MessageList / Timeline
      │
      ├── AssistantToolUseMessage.tsx (Header, Tool Name, Arguments, Loader)
      │     └── ToolUseLoader.tsx (Animated Spinner / Status Bullet)
      │
      ├── UserToolResultMessage.tsx (Result Router)
      │     ├── UserToolSuccessMessage.tsx (Success Result Rendering)
      │     ├── UserToolErrorMessage.tsx (Error Result Rendering)
      │     ├── UserToolRejectMessage.tsx (User Rejection / Permission Denied)
      │     └── UserToolCanceledMessage.tsx (Execution Canceled)
      │
      └── CollapsedReadSearchContent.tsx (Grouped Tool Calls / Summary Lines)
            └── CtrlOToExpand.tsx (Ctrl+O Expansion Hint Component)
```

### Component Roles & State Ownership:
- **`AssistantToolUseMessage.tsx`**: Renders the tool invocation header. Determines `userFacingName` (bold text) and formats argument hints.
- **`UserToolResultMessage.tsx`**: Acts as a polymorphic router for completed tool outputs based on `param.is_error`, `CANCEL_MESSAGE`, and `REJECT_MESSAGE`.
- **`CollapsedReadSearchContent.tsx`**: Manages grouped read/search tool executions (e.g., `⏺ Searched for 13 patterns, read 6 files`), maintaining an `isExpanded` boolean state toggled via `Ctrl+O`.

---

## 3. Tool Lifecycle State Machine (`SOURCE-CONFIRMED`)

Claude Code establishes 6 explicit tool execution lifecycle states:

```text
  [1. Queued] ────────► [2. InProgress] ────────► [3. WaitingForPermission]
                             │                            │
                             ▼                            ▼
                      [4. Completed]              [5. Rejected / Denied]
                       /          \
                      /            \
             [4a. Success]     [4b. Error]
```

1. **Queued**: Tool call block emitted by Assistant; execution not yet initiated.
2. **InProgress**: Tool currently running in background (`inProgressToolUseIDs.has(id)`). Displays animated `ToolUseLoader` spinner.
3. **WaitingForPermission**: Execution paused awaiting user approval (`pendingWorkerRequest.toolUseId === id`).
4. **Completed (Success)**: Execution completed cleanly (`isResolved && !is_error`). Displays `✔` tick mark and tool output.
5. **Completed (Error)**: Tool failed (`is_error == true`). Displays `✖` cross mark and error message in error style.
6. **Rejected / Denied**: User denied execution (`REJECT_MESSAGE`). Displays `✖` and rejection badge.

---

## 4. Visual Contract & Formatting (`SOURCE-CONFIRMED`)

### Visual Elements:
- **Bullet / Status Symbol**:
  - Running / Progress: `BLACK_CIRCLE` (`⏺`, `\u25CF`) or `ToolUseLoader` spinner.
  - Success: `figures.tick` (`✔`, `\u2714`) in `success` color.
  - Error / Rejection: `figures.cross` (`✖`, `\u2716`) in `error` color.
- **Tool Header Line**:
  - Tool Name: Bold text (e.g. `FileEdit`).
  - Tool Arguments: Enclosed in parentheses with `dimColor` (e.g. `(crates/brain-tui/src/state.rs)`).
- **Expansion Hint**:
  - `(ctrl+o to expand)` rendered in `dimColor` right of summary text when collapsed (`CtrlOToExpand.tsx`).

---

## 5. Collapsed vs Expanded Result Semantics (`SOURCE-CONFIRMED`)

### Collapsed Mode (Default):
- Occupies **1 visual row** in the message list.
- Displays summary line (e.g. `⏺ Read 1 file (ctrl+o to expand)` or `⏺ Bash(cargo check) → success`).
- Truncates large multiline outputs to 1 preview line.

### Expanded Mode (`Ctrl+O` Toggled):
- Expands to show full multiline output or code block below tool header.
- Height grows dynamically to fit output lines (up to maximum scrollback limit).
- Participates in Two-Pass layout measurement as intrinsic message content height.

---

## 6. Interaction Contract & Key Routing (`SOURCE-CONFIRMED`)

- **`Ctrl+O` / `Alt+T`**: Toggles expansion state (`isExpanded = !isExpanded`) for the active or focused tool execution block.
- **Auto-Expansion during Active Execution**: While a tool is `InProgress`, its status loader is active. Upon completion, the result defaults to collapsed mode with `Ctrl+O` affordance.

---

## 7. Error Handling & Truncation Semantics (`SOURCE-CONFIRMED`)

- **Error Formatting**: Rendered with `is_error` flag, displaying `✖` and error details in `ThemeToken::TextError` style.
- **Output Truncation**: Tool outputs exceeding terminal viewport height are capped at `maxVisibleLines` (default 20 lines) with a `... (N lines truncated)` indicator when expanded.

---

## 8. Brain Current Architecture vs Claude Parity Matrix

| Feature / Capability | Claude Oracle (`UserToolResultMessage.tsx`) | Brain Current (`tool.rs` / `chat.rs`) | Status / Gap | Evidence Level |
| :--- | :--- | :--- | :--- | :--- |
| **Tool Execution Data Model** | `ToolUseBlockParam`, `ToolResultBlockParam` | `ToolExecution`, `ToolApproval` | **MATCH** | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Status Bullet Icons** | `⏺` (Progress), `✔` (Success), `✖` (Error) | Plain text labels | **GAP**: Missing status icons | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Tool Header Formatting** | Bold Tool Name + Dim Arguments | Raw text block | **GAP**: Missing tool header formatting | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Collapsible Output Drawer** | 1-row collapsed vs full expanded (`Ctrl+O`) | Always expanded raw text | **GAP**: Missing collapsible result drawer | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Expansion Hint** | `(ctrl+o to expand)` hint | None | **GAP**: Missing expansion hint | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |
| **Output Truncation** | Capped at 20 lines when expanded | Capped at viewport height | **GAP**: Missing line truncation bound | `SOURCE-CONFIRMED` / `BRAIN-CONFIRMED` |

---

## 9. Two-Pass Layout Engine & Measurement Integration (`BRAIN-CONFIRMED`)

- **Pass 1 (Measurement)**: `LayoutEngine::measure_chat` measures message block heights.
  - When tool card is **collapsed**: Intrinsic height $= 1$ row.
  - When tool card is **expanded**: Intrinsic height $= 1 \text{ (header)} + \min(output\_lines, 20) \text{ rows}$.
- **Pass 2 (Geometry Allocation)**: Allocates chat viewport geometry based on intrinsic timeline height. Zero layout engine refactoring required.

---

## 10. Locked-Subsystem Safety Audit (`BRAIN-CONFIRMED`)

- **Two-Pass Layout Engine**: Untouched (reads intrinsic height during Pass 1).
- **Inline Collapsible Thinking Blocks**: Untouched (shares `Ctrl+O` toggle router pattern without collision).
- **New Messages Pill**: Untouched.
- **Multiline Prompt Cursor**: Untouched.

---

## 11. Architectural Evaluation & Final Recommendation

1. **Ratatui Sufficiency**: Ratatui `Paragraph` and `Line`/`Span` widgets are 100% sufficient to render tool execution cards.
2. **Backend/UDS Boundary**: Zero backend changes required (`active_tool_calls` in `UiState` already contains necessary tool call fields).
3. **Target Subsystem**: Scope strictly confined to `crates/brain-tui` (`src/ui/widgets/tool_card.rs`, `chat.rs`, `state.rs`, `router.rs`).

---

## 12. Final Recommendation Gate

```text
APPROVED FOR DESIGN SPECIFICATION
```
