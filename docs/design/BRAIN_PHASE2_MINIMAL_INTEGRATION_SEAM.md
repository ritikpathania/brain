# Phase 2 Architecture: Minimal Brain Integration Seam Specification

> **Document Status**: Integration Seam Architecture Specification (Pre-Implementation Audit)  
> **Presentation Ground Truth**: 100% Immutable Phase 1 Literal Claude Baseline (`packages/brain-frontend/src/components/`)  
> **Backend Ground Truth**: 100% Immutable Rust Daemon & UDS Protocol (`crates/`, `daemon/`)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 2 — MINIMAL BRAIN INTEGRATION SEAM
================================================================================

ONE-WAY PROP FLOW:
    Brain Rust Daemon (Unix Domain Socket: ~/.brain/daemon.sock)
               │
               │ (JSONL IPC: stream_chunk, stream_end, tool_call, sessions)
               ▼
         BrainUdsClient (Transport-only layer)
               │
               │ (Direct state reduction into Claude-shaped props)
               ▼
     BrainRuntimeStateContainer (Zero middleman layers, zero UI inventions)
               │
               │ Standard Claude Props:
               │   • Messages: PresentationMessage[]
               │   • Streaming: { activeText, isStreaming }
               │   • Prompt: { buffer, cursorOffset, multiline }
               │   • Modals: { activeModal, searchQuery }
               ▼
       UNCHANGED CLAUDE PRESENTATION BASELINE
       (FullscreenLayout, Messages, PromptInput, StatusLine, FuzzyPicker, HelpV2)
================================================================================
```

---

## 1. Trace of Runtime Pipelines

### A. Brain Backend Pipeline
1. **Entrypoint**: Rust `brain daemon` listening on `~/.brain/daemon.sock`.
2. **Wire Protocol**: Line-delimited JSON (JSONL).
   - Event Stream: `stream_start`, `stream_progress`, `stream_chunk`, `stream_end`, `stream_cancelled`.
   - RPC Actions: `v1/sessions/get`, `list_sessions`, `v1/search`, `v1/reflect`, `v1/compile`, `v1/inspect_node`.
3. **Transport**: `BrainUdsClient` connects via Node/Bun `net.Socket`. Emits lines and connection status events.

### B. Claude Frontend Presentation Pipeline
1. **Root Screen**: `App.tsx` mounts `FullscreenLayout`.
2. **Top Canvas**: `Messages` renders `LogoV2` (header) + sequential `MessageRow` items.
3. **Turn Hierarchy**:
   - User: `UserPromptMessage` (`#1E1E1E` card with `❯ ` in `#D77757`).
   - Assistant: `AssistantThinkingMessage` (`∴ Thinking`), `AssistantToolUseMessage` (`● tool(args)`), `UserToolResultMessage` (` 1 │ `), and `AssistantTextMessage` (Markdown AST).
4. **Bottom Region**:
   - Autocomplete: `FuzzyPicker` (mounted when buffer starts with `/`).
   - Composer: `PromptInput` (rounded box with `❯ `, block cursor `▌`, auto-expanding 1-8 rows) + `PromptInputFooter` (`? for shortcuts`).
   - Footer: `StatusLine` (borderless single line).
5. **Modal Layer**: `GlobalSearchDialog` (`Ctrl+K` palette) and `HelpV2` (`?` / `/help`).

---

## 2. Identified Minimal Integration Seam

The integration seam connects `BrainUdsClient` to Claude's presentation props with zero superfluous intermediary abstractions:

```text
================================================================================
MINIMAL INTEGRATION RESPONSIBILITY MATRIX
================================================================================
Layer / Component            Concrete Responsibility & Justification
───────────────────────────  ───────────────────────────────────────────────────
1. BrainUdsClient            • Pure transport: socket management, auto-reconnect,
                               JSONL framing, one-shot RPC requests.
                             • ZERO presentation logic, ZERO state storage.

2. BrainRuntimeCoordinator   • Orchestrates runtime event loop:
                               - Ingests user prompt submissions from PromptInput.
                               - Dispatches queries to daemon over UDS.
                               - Reduces wire events into standard message array.
                               - Handles tool approvals (y/n) and dispatches response.
                               - Dispatches slash commands to RPC endpoints.
                             • Feeds props directly to App.tsx.

3. Canonical Claude Shell    • 100% Phase 1 baseline (IMMUTABLE & UNTOUCHED).
                             • Renders pure Claude Code UI.
================================================================================
```

---

## 3. Capability Mapping Onto Existing Claude Surfaces

Every Brain capability is mapped strictly onto an existing Claude Code interaction surface without creating custom UI chrome:

| Brain Backend Capability | Target Claude UI Surface | Mapping Specification |
|---|---|---|
| **Streaming Responses** | `AssistantTextMessage` | Incoming `stream_chunk` tokens append to `activeText`, rendering markdown with live trailing `▌`. |
| **Thinking / Reasoning** | `AssistantThinkingMessage` | Thinking traces render with `∴ Thinking (N.Ns)...` and expand/collapse via `Ctrl+O`. |
| **Tool Execution** | `AssistantToolUseMessage` | Invoked tools render `● tool_name(args)` with lifecycle badges (`[RUNNING]`, `[COMPLETED]`). |
| **Tool Permissions** | `AssistantToolUseMessage` | Permission prompts render `❯ Permission required: [y/Enter, n/Esc]` and capture `y`/`n`. |
| **Tool Output Drawers** | `UserToolResultMessage` | Output renders inside rounded line-numbered drawer (` 1 │ `) capped at 20 rows with `[Ctrl+O]`. |
| **Session Restoration** | `Messages` | Restored session history populates standard `messages: PresentationMessage[]` array. |
| **Command Autocomplete** | `FuzzyPicker` | Slash command typing triggers `FuzzyPicker` popup anchored above the prompt composer. |
| **Memory / Graph Search** | `GlobalSearchDialog` | `Ctrl+K` opens `GlobalSearchDialog`, routing queries to daemon hybrid search over UDS. |
| **Help & Reference** | `HelpV2` | `/help` or `?` opens `HelpV2` keybinding reference manual. |

---

## 4. Invariant & Non-Regression Guarantees

1. **Zero Presentation Modifications**: All 21 files in `packages/brain-frontend/src/components/` remain bit-for-bit identical to Phase 1.
2. **Zero Invented UI Chrome**: No custom status bars, no memory chips, no daemon health widgets, no branding overrides.
3. **No Redundant Abstractions**: Eliminated artificial middleman classes; the runtime coordinator directly produces standard Claude props.
4. **Backend & Protocol Immutability**: Zero changes to the Rust backend (`crates/`, `daemon/`) and zero changes to the UDS wire protocol.
