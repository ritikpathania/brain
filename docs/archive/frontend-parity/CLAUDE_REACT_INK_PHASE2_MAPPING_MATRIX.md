# Specification — Phase 2 Mapping Matrix: Brainification & Feature Pruning

> **Document Status**: Approved Phase 2 Mapping Matrix & Architectural Specification  
> **Target Package**: `packages/brain-frontend` (React + Ink + Yoga Stack)  
> **Core Strategy**: **Brainification & Feature Pruning** (Prune Claude product placeholders; map Brain engine semantics)  
> **Authoritative Baseline Reference**: [`docs/design/CLAUDE_REACT_INK_PHASE1_BRAIN_ADAPTER_IMPLEMENTATION_REPORT.md`](CLAUDE_REACT_INK_PHASE1_BRAIN_ADAPTER_IMPLEMENTATION_REPORT.md)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
==================================================
PHASE 2 MAPPING MATRIX — BRAINIFICATION & PRUNING
==================================================
1. Remove Claude Product Placeholders (/model, /effort, cost counters)
2. Map Brain Engine Semantics (Brain Session, Relational Memory, Graph Retrieval)
3. Preserve Visual & Interaction Shell (100% Layout & Ink Integrity)
4. Retain Deferred Gaps Separately (Alt+Y, Historic Selection, Mouse Click)
```

---

## 1. Master Phase 2 Mapping Matrix

| Claude / UI Surface | Phase 0 State | Phase 1 State | Phase 2 Action & Decision | Brain Engine Mapping & Justification |
| :--- | :--- | :--- | :--- | :--- |
| **Header Title** | Mock Title | Adapter Live | **Map** $\rightarrow$ Brain Session | Shows `Brain Engine (session_title / working_dir)`. |
| **User Query** | Mock Query | Adapter Live | **Keep** $\rightarrow$ Brain Query | Maps `UserTextMessage` to Brain relational graph query. |
| **Assistant Response** | Mock Text | Adapter Live | **Keep** $\rightarrow$ Streamed Response | Renders streamed markdown response tokens. |
| **Thinking Blocks** | Mock Reasoning | Adapter Live | **Map** $\rightarrow$ Brain Stage Trace | Maps `AssistantThinkingMessage` to reasoning & retrieval stages. |
| **Tool Execution Cards** | Mock Tools | Adapter Live | **Map** $\rightarrow$ Brain Tool Calls | Maps `AssistantToolUseMessage` to Brain tool executions (`run_command`, `view_file`, etc.). |
| **Tool Output Drawer** | Mock Output | Adapter Live | **Map** $\rightarrow$ Brain Tool Result | Maps `UserToolResultMessage` with 20-line drawer cap and line numbers. |
| **`/model` Command** | Mock Command | Mock Command | **Remove** | Non-applicable. Brain uses local daemon configuration. |
| **`/effort` Command** | Mock Command | Mock Command | **Remove** | Non-applicable. Brain manages reasoning pipeline internally. |
| **Token Counter** | Mock `1420` | Mock `1420` | **Remove** | Non-applicable. Brain operates local relational graph engine. |
| **Cost Counter** | Mock `$0.0042` | Mock `$0.0042` | **Remove** | Non-applicable. Local engine without Anthropic API cost. |
| **Status Bar Footer** | Mock Status | Mock Status | **Map** $\rightarrow$ Brain Status Line | Shows `● Brain v1.1.0 | daemon:connected | memory:active`. |
| **Command Palette (`Ctrl+K`)** | Mock Palette | UI Modal | **Map** $\rightarrow$ Brain Commands | Maps commands (`/help`, `/config`, `/status`, `/clear`, `/exit`). |
| **Shortcuts Modal (`?`)** | Mock Modal | UI Modal | **Keep** $\rightarrow$ Keybindings Matrix | Retains full keybinding reference matrix. |
| **Sticky Prompt Header** | Mock Sticky | Adapter Live | **Keep** $\rightarrow$ Sticky Prompt Header | Retains 1-row top header when prompt scrolls off-screen. |
| **New Messages Pill** | Mock Pill | Adapter Live | **Keep** $\rightarrow$ New Messages Pill | Retains bottom-row `↓ N new messages` jump indicator. |

---

## 2. Component Cleanup & Refactoring Strategy

1. **`StatusLine.tsx`**:
   - Prune `modelName`, `effortTier`, `totalTokens`, and `totalCostUsd` props.
   - Introduce `version` (`Brain v1.1.0`), `daemonStatus` (`connected`), and `memoryStatus` (`active`).
   - Clean status line format: `● Brain v1.1.0 | daemon:connected | memory:active`.

2. **`GlobalSearchDialog.tsx`**:
   - Remove `/model` and `/effort` from command palette results.
   - Include official Brain CLI commands: `/help`, `/config`, `/status`, `/clear`, `/exit`.

3. **`types/presentation.ts`**:
   - Update `footer` schema to reflect Brain engine status indicators instead of Anthropic API metrics:
     ```typescript
     footer: {
       engineVersion: string; // "Brain v1.1.0"
       daemonStatus: 'connected' | 'connecting' | 'disconnected';
       memoryStatus: string;  // "active"
     };
     ```

4. **`BrainFrontendAdapter.ts`**:
   - Update default `baseFixture` and state initialization to emit Brain engine status.
   - Stream event mappings for `Token`, `Stage`, `ToolCallRequest`, `ToolCallResult`, `Finished`, `Error`, and `Cancelled` remain 100% intact.

---

## 3. Retained Deferred Non-Blocking Gaps Record

The following 3 deferred non-blocking gaps remain explicitly isolated and separate from Phase 2 Brainification:
1. `Alt+Y` multi-item kill-ring rotation (`yankPop`).
2. Historic tool card keyboard selection (`Ctrl+O` targets active drawer).
3. Sticky prompt mouse click trigger (requires terminal mouse router).

---

## 4. Execution Roadmap

```text
Phase 2 Mapping Matrix Approved (Current)
      ↓
Component Refactoring (StatusLine, Types, Adapter, Fixtures)
      ↓
Automated Test Verification (bun test)
      ↓
Phase 2 Implementation Report (CLAUDE_REACT_INK_PHASE2_IMPLEMENTATION_REPORT.md)
```
