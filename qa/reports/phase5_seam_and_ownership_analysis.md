# Phase 5 — Seam & Ownership Analysis Report

**Document Version**: 1.0.0  
**Status**: ARCHITECTURAL SPECIFICATION & SEAM DISCOVERY  
**Scope**: In-depth analysis of remaining capability boundaries before Phase 5.6 implementation  
**Governing Invariant**: `vendor/claude/` remains **100% frozen & immutable** (1,925/1,925 files SHA-256 identical).

---

## 1. Executive Summary & Certified Foundation

Phases 5.1 through 5.5 established that Claude's frozen upstream runtime can consume Brain model execution through a clean, conforming `CallModelFn` seam without modifying any vendor source files, presentation components, or REPL orchestration:

```text
┌────────────────────────────────────────────────────────────────────────┐
│ CLAUDE RUNTIME (FROZEN & IMMUTABLE)                                    │
│ • PromptInput & Interactive Multiline Composer                         │
│ • REPL & Query Loop Orchestration (query.ts)                           │
│ • Tool Schemas, Permissions & Local Execution (StreamingToolExecutor)  │
│ • Streaming Reducers & UI Viewport (VirtualMessageList, Markdown)      │
│ • Session State Management & Local JSONL Transcripts                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    │ QueryDeps.callModel(params)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ BRAIN CALLMODEL ADAPTER (TypeScript Boundary)                          │
│ • normalizeMessagesForBrain(messages) [User, Assistant, Tools, Thinking]│
│ • normalizeToolsForBrain(tools) [Authoritative Tool Schemas]           │
│ • normalizeThinkingConfig(thinkingConfig) [Adaptive, Budgets]          │
│ • Binds AbortSignal to BrainBackendClient                              │
│ • Normalizes Brain Stream Chunks → Claude StreamEvents                 │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    │ BrainGenerationRequest / BrainStreamChunk
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ BRAIN BACKEND CLIENT INTERFACE (Transport-Agnostic)                    │
│ • streamText(request): AsyncIterable<BrainStreamChunk>                 │
│ • Implementations: Mock (Hermetic Tests), UDS (Live Brain Daemon)      │
└────────────────────────────────────────────────────────────────────────┘
```

### Cumulative Certification Baseline

| Phase | Milestone | Tested Invariants | Status |
| :--- | :--- | :--- | :---: |
| **5.1** | Deterministic Contract Seam | Stream event sequence, delta accumulation, AssistantMessage payload, multi-turn accumulation, error propagation, cancellation | ✅ CERTIFIED |
| **5.2** | Brain Text Stream Adapter | Real `BrainBackendClient` text adapter, prompt serialization, error normalization | ✅ CERTIFIED |
| **5.3** | Adversarial Races | 8 race conditions: Ctrl+C before/during/after token, severed sockets, idempotent aborts, rapid 4-turn sequence, 0 orphan streams | ✅ CERTIFIED |
| **5.4** | Tool Execution Round-Trip | 8 tool scenarios: single tool, permissions approval/denial, execution failures, multiple tools, tool+streaming, cancellation | ✅ CERTIFIED |
| **5.5** | Thinking & Reasoning Blocks | 12 scenarios: thinking deltas, strict block ordering, multi-chunk accumulation, thinking+tool, redacted thinking, budgetTokens, 0 fake signatures | ✅ CERTIFIED |

---

## 2. Capability Area 1: Context Compaction

### 2.1 Codebase Discovery & Tracing

In Claude's upstream architecture, context management is handled at two distinct levels in `vendor/claude/query/deps.ts` and `vendor/claude/query.ts`:

1. **Microcompaction (`microcompactMessages` in `services/compact/microCompact.ts`)**:
   - **Mechanism**: In-memory inspection of historical `Message[]`.
   - **Operation**: Replaces old tool results with `[Old tool result content cleared]` for compactable tools (`FileRead`, `Bash`, `Grep`, `Glob`, `WebFetch`) after a time threshold or token threshold is exceeded.
   - **Model Dependency**: **Zero model calls**. Pure synchronous string replacement on in-memory message objects.
2. **Autocompaction (`autoCompactIfNeeded` in `services/compact/autoCompact.ts`)**:
   - **Mechanism**: Evaluates `tokenCountWithEstimation(messages)` against `getAutoCompactThreshold(model)`.
   - **Operation**: When threshold is exceeded, triggers `compactConversation()`, which forks a sub-query using `deps.callModel` to summarize earlier conversation turns into a `CompactSummaryMessage`.
   - **Model Dependency**: Executes through `deps.callModel` (our existing adapter seam).

### 2.2 Ownership & Responsibility Matrix

| Sub-Capability | Claude-Owned | Brain-Owned | Seam Classification |
| :--- | :---: | :---: | :---: |
| **Microcompaction** | ✅ In-memory tool result replacement | ❌ None | **`KEEP`** (Native Claude) |
| **Token Threshold Evaluation** | ✅ Context window limits & warning math | ❌ None | **`KEEP`** (Native Claude) |
| **Compaction Triggering** | ✅ Query loop autocompact check | ❌ None | **`KEEP`** (Native Claude) |
| **Compaction Summarization Turn** | ❌ None | ✅ Generates summary via `callModel` | **`ADAPT`** (via `QueryDeps.callModel`) |
| **Context Collapse (Tier-C)** | ✅ Strips signatures on fallback | ❌ None | **`KEEP`** (Native Claude) |

### 2.3 Boundary Decision

> **Verdict: `KEEP` Claude's native compaction orchestration; `ADAPT` the summarization turn through `QueryDeps.callModel`.**
> 
> Because `QueryDeps` already exposes `microcompact` and `autocompact`, and because `compactConversation` delegates its model call directly through `deps.callModel`, Claude's existing compaction machinery works out of the box with zero custom compaction code.

---

## 3. Capability Area 2: Session Persistence & Resume

### 3.1 Codebase Discovery & Tracing

Session management in Claude is implemented in `vendor/claude/utils/sessionStorage.ts` and `vendor/claude/utils/sessionRestore.ts`:

1. **Transcript Persistence (`saveMessageToDisk`)**:
   - **Location**: `~/.claude/projects/<slug>/<session-id>.jsonl`.
   - **Format**: Append-only JSONL log containing `SerializedMessage` records (`user`, `assistant`, `attachment`, `tool_result`).
   - **Role**: Powers instant terminal resume (`/resume`), session switching (`/session`), and terminal scrollback rehydration.
2. **Session Rehydration (`loadSession` / `sessionRestoreStateFromLog`)**:
   - Reads the JSONL transcript sequentially, reconstructs in-memory `messages: Message[]`, restores cost tracking, and resets file state caches.
3. **Brain Relational Memory vs Claude Session Transcripts**:
   - **Brain Responsibility**: Long-term relational knowledge graph, cross-session memory consolidation, semantic node search, DDD entities (`crates/brain-domain/`, `crates/brain-storage/`).
   - **Claude Responsibility**: Interactive terminal conversation transcript, UI scrollback buffer, local turn replay.

### 3.2 Ownership & Responsibility Matrix

| Sub-Capability | Claude-Owned | Brain-Owned | Seam Classification |
| :--- | :---: | :---: | :---: |
| **UI Transcript Logging (JSONL)** | ✅ Client turn persistence | ❌ None | **`KEEP`** (Native Claude) |
| **Session Resume UI (`/resume`, `/history`)** | ✅ Transcript rehydration | ❌ None | **`KEEP`** (Native Claude) |
| **Cross-Session Relational Memory** | ❌ None | ✅ Knowledge Graph / SQLite | **`REPLACE`** (Brain Domain) |
| **Semantic Entity Extraction** | ❌ None | ✅ Entity node & edge mutations | **`REPLACE`** (Brain Services) |

### 3.3 Boundary Decision

> **Verdict: `KEEP` Claude's local JSONL transcript storage for frontend session resume; `REPLACE` memory/knowledge extraction with Brain's native relational engine.**
> 
> Claude continues logging turn JSONL files to preserve native `/resume`, session history, and UI replay without friction. Brain asynchronously ingests conversation turns into the relational knowledge graph via its service layer.

---

## 4. Capability Area 3: UDS Transport Binding

### 4.1 Codebase Discovery & Tracing

Brain's daemon architecture communicates via a Unix Domain Socket (UDS) using newline-delimited JSON streaming:
- **Daemon Socket**: `/tmp/brain.sock` or `~/.brain/daemon.sock`.
- **Rust Client Engine**: `crates/brain-sdk-rs/src/client.rs`.
- **Protocol Handshake**:
  ```json
  {"action": "handshake", "payload": "{\"protocol_version\":\"1.0\",\"capabilities\":[\"ConversationMessages\"]}"}
  ```
- **Stream Event Framing**:
  ```text
  Client -> {"action": "query", "payload": {...}}
  Daemon -> {"type": "token", "token": "..."}\n
  Daemon -> {"type": "tool_use", "toolUse": {...}}\n
  Daemon -> {"type": "finished"}\n
  ```

### 4.2 Ownership & Responsibility Matrix

| Sub-Capability | Claude-Owned | Brain-Owned | Seam Classification |
| :--- | :---: | :---: | :---: |
| **Model Invocations** | ❌ None | ✅ Rust Daemon Engine | **`REPLACE`** (Brain Daemon) |
| **Transport Lifecycle (Connect, Reconnect)** | ❌ None | ✅ UDS Client Adapter | **`ADAPT`** (`BrainBackendClient`) |
| **Frame Serialization & Parsing** | ❌ None | ✅ Line-delimited JSON parser | **`ADAPT`** (`BrainBackendClient`) |
| **Stream Event Mapping** | ❌ None | ✅ `brainCallModel` adapter | **`ADAPT`** (`brainCallModel.ts`) |

### 4.3 Smallest Stable Boundary

We implement a dedicated `UdsBrainBackendClient` that implements the existing `BrainBackendClient` interface:

```typescript
export class UdsBrainBackendClient implements BrainBackendClient {
  constructor(private socketPath: string = '/tmp/brain.sock') {}

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    const socket = net.createConnection(this.socketPath);
    // 1. Handshake & query dispatch
    // 2. Stream chunk parsing with readline
    // 3. AbortSignal listener -> emit cancel action & socket.destroy()
  }
}
```

---

## 5. Capability Area 4: Authentication & Configuration

### 5.1 Codebase Discovery & Tracing

1. **Anthropic API Authentication (`utils/auth.ts`, `constants/oauth.ts`)**:
   - Claude natively looks for `ANTHROPIC_API_KEY`, Anthropic OAuth tokens, or setup tokens.
   - When hosted by Brain, `QueryDeps.callModel` directs all model requests to the Brain adapter and daemon.
   - **Result**: Anthropic API keys are **completely unnecessary** for model execution.
2. **Claude Client Configuration (`utils/settings/settings.ts`)**:
   - Manages user theme (dark/light/auto), editor preferences, verbose logging, output style, and tool permission rules (`~/.claude/settings.json`).
   - These settings are purely local client preferences and operate completely offline.

### 5.2 Ownership & Responsibility Matrix

| Sub-Capability | Claude-Owned | Brain-Owned | Seam Classification |
| :--- | :---: | :---: | :---: |
| **Model API Authentication** | ❌ Bypassed | ✅ Daemon UDS permissions | **`REPLACE`** (Local Daemon) |
| **User Settings (`settings.json`)** | ✅ Theme, editor, output styles | ❌ None | **`KEEP`** (Native Claude) |
| **Tool Permission Rules** | ✅ Always-allow rules & modes | ❌ None | **`KEEP`** (Native Claude) |

---

## 6. Comprehensive Seam & Capability Classification Matrix

| Domain / Capability | Upstream Claude Location | Boundary / Seam | Strategy | Rationale |
| :--- | :--- | :--- | :---: | :--- |
| **Prompt Input & Multiline Composer** | `src/components/PromptInput.tsx` | UI presentation layer | **`KEEP`** | Claude owns 100% of interactive typing, history, cursor math |
| **REPL & Query Loop** | `vendor/claude/query.ts` | Orchestration | **`KEEP`** | Claude manages turn loop, permissions, and reducers |
| **Tool Schemas & Permissions** | `vendor/claude/Tool.ts`, `toolExecution.ts` | Execution engine | **`KEEP`** | Claude owns tool schemas, validation, execution, and results |
| **Model Execution** | `vendor/claude/services/api/claude.ts` | `QueryDeps.callModel` | **`REPLACE`** | Brain provides model intelligence & reasoning stream |
| **Reasoning / Thinking Stream** | `vendor/claude/services/api/claude.ts` | `thinking_delta` chunks | **`ADAPT`** | Brain streams reasoning; Claude reduces to `ThinkingBlock` |
| **Microcompaction** | `vendor/claude/services/compact/microCompact.ts` | `QueryDeps.microcompact` | **`KEEP`** | Pure in-memory tool output trimming |
| **Autocompaction** | `vendor/claude/services/compact/autoCompact.ts` | `QueryDeps.autocompact` | **`ADAPT`** | Claude triggers threshold; Brain executes summary turn |
| **JSONL Session Transcripts** | `vendor/claude/utils/sessionStorage.ts` | File system logging | **`KEEP`** | Enables native `/resume`, `/history`, and UI rehydration |
| **Relational Knowledge Memory** | `crates/brain-domain/`, `crates/brain-storage/` | Asynchronous service ingest | **`REPLACE`** | Brain relational graph replaces Claude SessionMemory |
| **UDS Daemon Transport** | `crates/brain-sdk-rs/` | `BrainBackendClient` | **`ADAPT`** | Line-delimited socket streaming between Node and Rust |
| **Anthropic Cloud Auth** | `vendor/claude/utils/auth.ts` | Bypassed by `callModel` | **`REPLACE`** | Brain daemon manages local execution permissions |
| **User Settings & Preferences** | `vendor/claude/utils/settings/settings.ts` | Local file settings | **`KEEP`** | Client preferences (theme, verbose) preserved locally |

---

## 7. Recommended Implementation Sequence for Remaining Phases

```text
Phase 5.6: Live UDS Transport Adapter
  ├── Implement UdsBrainBackendClient (Node net socket -> Brain daemon)
  ├── Framing & reconnect verification
  └── Live token stream through Claude runtime

Phase 5.7: Autocompaction & Multi-Turn Integration
  ├── Exercise autocompact summarization via Brain callModel
  └── Verify context threshold behavior and token estimations

Phase 5.8: End-to-End Regression & Gate Certification
  ├── 100% Green Test Suite (Contract, Text, Races, Tools, Thinking, UDS, Compaction)
  ├── 7/7 PTY Interactive Verification
  └── 1,925 / 1,925 Vendor Integrity Gate A
```
