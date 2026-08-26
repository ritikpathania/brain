# CLAUDE ↔ BRAIN CAPABILITY MAP & ARCHITECTURAL MIGRATION AUTHORITY

**Document Version:** 1.1.0  
**Baseline Contract:** Phase 8A Certified (`v2.1.233` frozen vendor parity, 156/156 test suites passing, 0 diffs)  
**Status:** **ACTIVE / FROZEN ARCHITECTURAL AUTHORITY**  

---

## 1. Core Architectural Paradigm

### 1.1 The Golden Law of Ownership
> **"TypeScript owns product interaction. Rust owns Brain intelligence."**

The objective of Brain architecture is **not** a total rewrite of the product shell in Rust, nor is it permanent lock-in to Claude's backend implementation. Instead, we establish a clean, durable three-tier boundary:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          CLAUDE PRODUCT SHELL                               │
│                      (React 19 + Ink 5 + TypeScript)                        │
│                                                                             │
│  - Composer Subsystem (Input, History, Modes: prompt, !, &, @)              │
│  - Interactive Overlays (Help, Model Picker, Permissions, Dialogs)          │
│  - Streaming Presentation & Token Typewriter Queue                          │
│  - Thinking Block UI & Tool Execution Progress UX                           │
│  - Markdown & ANSI Formatting / Fullscreen Terminal Layout                  │
│                                                                             │
│                     WHAT WE NEED: PRESERVED & KEPT                          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                      Product Runtime Contract (TypeScript)
                                       │
┌──────────────────────────────────────▼──────────────────────────────────────┐
│                     BRAIN TS ADAPTER & IPC GATEWAY                          │
│                 (packages/brain-shell/src/adapter/)                         │
│                                                                             │
│  - QueryDeps.callModel Seam Translation Boundary                            │
│  - Protocol Envelope Framing & Monotonic Sequence Validation                │
│  - UDS Transport Client (/tmp/brain.sock)                                   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                          Stable IPC Protocol (UDS / JSON)
                                       │
┌──────────────────────────────────────▼──────────────────────────────────────┐
│                             BRAIN RUNTIME                                   │
│                        (Rust Domain & Services)                             │
│                                                                             │
│  crates/brain-domain:      Pure DDD Models, Events, & Invariants            │
│  crates/brain-services:    Knowledge Graph, Reflection, Ranking, Retrieval  │
│  crates/brain-application: ApplicationRuntime, Facades, & Workflows         │
│  crates/brain-daemon:      Async UDS Server & Streaming Daemon Bridge       │
│  crates/brain-persistence: SQLite WAL, Event Store, & Projections           │
│                                                                             │
│                  WHAT BRAIN OWNS: REASONING & MEMORY                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Classification & Runtime Status Values

Every capability is assigned an architectural decision and empirical runtime status:

#### Architectural Decision:
- **`KEEP`**: Mature, polished product-shell concern (UI/UX, keyboard, terminal layout). Maintained in TypeScript.
- **`ADAPT`**: Useful frontend orchestration whose data/execution is routed to Brain via TS Adapter.
- **`REPLACE`**: Core intelligence, memory, context, or reasoning capability superseded by Brain Rust.
- **`REMOVE_LATER`**: Redundant or orphaned legacy Claude cloud services retained in vendor until all dependencies are migrated.

#### Empirical Runtime Status:
- **`DIFFERENTIAL VERIFIED`**: Executed side-by-side against reference Claude binary and proven cell-equivalent.
- **`RUNTIME VERIFIED`**: Executed and proven across automated integration test suites.
- **`IMPLEMENTED`**: Code is written and passing local unit tests.
- **`PARTIAL`**: Baseline established, pending deeper subsystem migration.
- **`MISSING`**: Not yet implemented.
- **`INTENTIONALLY DIFFERENT`**: Deliberate architectural enhancement beyond Claude baseline.

---

## 2. Exhaustive Subsystem Inventory & Classification Matrix

### Domain 1: Terminal UI, Shell, & Layout

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Composer State Machine** | `components/PromptInput/PromptInput.tsx` | TS | — | **`KEEP`** | In-memory React state | None (Phase 7B Wave 1) | Never (Core Shell) | `composerContracts.test.ts` (States 02–07) |
| **Slash Command Palette** | `components/PromptInput/SlashCommandSelector.tsx` | TS | `brain-tools` registry metadata | **`ADAPT`** | TS Command Registry $\leftrightarrow$ Rust Tools | Phase 8.2 | None | `composerContracts.test.ts` (State 04) |
| **@ File Path Autocomplete** | `components/PromptInput/FileMentionSelector.tsx` | TS | — | **`KEEP`** | Local FS / `ripgrep` | None (Phase 7B Wave 1) | Never (Core Shell) | `composerContracts.test.ts` (State 05) |
| **! Bash Mode & Execution** | `components/PromptInput/inputModes.ts` | TS | `brain-tools::bash` | **`KEEP`** | Local Shell Execution | None | Never | `composerContracts.test.ts` (State 07) |
| **& Background Mode** | `components/PromptInput/PromptInput.tsx` | TS | `brain-application::AsyncTasks` | **`ADAPT`** | UDS Task Notification | Phase 8.4 | None | `composerContracts.test.ts` (State 06) |
| **Fullscreen Layout Engine** | `components/FullscreenLayout.tsx` | TS | — | **`KEEP`** | Ink Reconciler | None | Never | `visualCellParity.test.ts` |
| **Header & Logo V2** | `components/LogoV2/LogoV2.tsx` | TS | — | **`KEEP`** | Host State Initialization | None (Phase 7D) | Never | `parityClosureContracts.test.ts` (GAP-01) |
| **Footer & Status Indicators**| `components/PromptInput/PromptInputFooter.tsx` | TS | — | **`KEEP`** | `ToolPermissionContext` | None (Phase 7D) | Never | `parityClosureContracts.test.ts` (GAP-02) |
| **ANSI / Markdown Syntax Highlighting** | `components/Output.tsx`, `components/Markdown.tsx` | TS | — | **`KEEP`** | Ink Text Engine | None | Never | `visualCellParity.test.ts` |

---

### Domain 2: Interactive Overlays & Modals

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Help Menu Overlay** | `components/PromptInput/PromptInputHelpMenu.tsx` | TS | — | **`KEEP`** | React state | None (Phase 7B Wave 2) | Never | `overlayContracts.test.ts` (State 09) |
| **Model Picker Modal** | `components/ModelPicker.tsx` | TS | `brain-services::ModelGateway` | **`ADAPT`** | `getModelOptions()` dynamic discovery | Phase 8.1 | None | `overlayContracts.test.ts` (State 10) |
| **Permission Request Dialog** | `components/permissions/PermissionDialog.tsx` | TS | `brain-domain::specifications` | **`ADAPT`** | `ToolPermissionContext` Seam | Phase 8.3 | Brain Authorization Spec | `overlayContracts.test.ts` (State 11) |
| **Settings & Config Editor** | `components/Settings/Config.tsx` | TS | `brain-persistence::Settings` | **`ADAPT`** | Local Config JSON/TOML | Phase 8.5 | None | `overlayContracts.test.ts` |
| **Diff Viewer Modal** | `components/diff/DiffDialog.tsx` | TS | — | **`KEEP`** | Local Git diffing | None | Never | `overlayContracts.test.ts` |
| **Session Resume Picker** | `components/ResumeTask.tsx` | TS | `brain-application::SessionService` | **`ADAPT`** | `listSessions()` IPC API | Phase 8.6 | Brain Session Projection | `lifecycleContracts.test.ts` (State 15) |

---

### Domain 3: Runtime Execution & Streaming Presentation

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Model Streaming Turn** | `services/api/stream.ts` | TS | `brain-daemon::StreamBridge` | **`ADAPT`** | `QueryDeps.callModel` $\leftrightarrow$ UDS | Phase 7B Wave 3 | None (Seam locked) | `runtimeContracts.test.ts` (State 12) |
| **Thinking / Reasoning UI** | `components/ThinkingBlock.tsx` | TS | `brain-domain::ThinkingBlock` | **`KEEP`** | `thinking_start`/`thinking_delta` events | None (Phase 7B Wave 3) | Never (Presentation) | `runtimeContracts.test.ts` (State 12a) |
| **Tool Progress & Results UX** | `components/ToolResult.tsx` | TS | `brain-services::ExecutionMonitor` | **`KEEP`** | `Tool.call()` return presentation | None (Phase 7B Wave 3) | Never (Presentation) | `runtimeContracts.test.ts` (State 13) |
| **Cancellation UX (Ctrl+C)** | `utils/cancellation.ts` | TS | `brain-services::cancellation` | **`ADAPT`** | Socket Abort $\leftrightarrow$ Rust Task Cancel | None (Phase 7B Wave 3) | None | `runtimeContracts.test.ts` (State 13b) |
| **Typewriter Queue Drain** | `ink/terminal.ts` | TS | — | **`KEEP`** | Ink 2-Stage Queue Pipeline | None | Never | `performanceProfiling.test.ts` |

---

### Domain 4: Memory, Context, & Retrieval

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Short-Term Memory (STM)** | `services/SessionMemory/` | Claude TS | `brain-domain::stm`, `brain-services::SessionMemory` | **`REPLACE`** | `BrainGenerationRequest.context` | **Phase 8.1** | Brain STM Adapter | `productValidationE2E.test.ts` |
| **Long-Term Memory (LTM)** | `utils/claudemd.ts` | Claude TS | `brain-services::ltm_retrieval`, `brain-persistence` | **`REPLACE`** | Brain Retrieval Engine | **Phase 8.1** | Brain Persistence Graph | `retrieval_tests.rs` |
| **Vector & BM25 Hybrid Retrieval** | Basic file scanning / grep | Claude TS | `brain-services::hybrid_evaluator`, `brain-services::fts` | **`REPLACE`** | `brain-services::query_engine` | **Phase 8.2** | Brain FTS5 / Embedding Index | `hybrid_benchmark_tests.rs` |
| **Relational Knowledge Graph** | None (Flat memory) | — | `brain-domain::KnowledgeGraph`, `brain-services::graph` | **`REPLACE`** | Rust Domain Knowledge Graph | **Phase 8.2** | None (Brain Innovation) | `graph_tests.rs` |
| **Context Synthesis & Packing** | `utils/attachments.ts` | Claude TS | `brain-services::semantic_binder`, `context_synthesis` | **`REPLACE`** | Context Construction Engine | **Phase 8.3** | Brain Context Plan Engine | `batch_context_tests.rs` |
| **CLAUDE.md File Loader** | `utils/claudemd.ts` | Claude TS | `brain-services::ProductionCorpus` | **`ADAPT`** | Hook into Brain Ingestion Pipeline | Phase 8.3 | Brain Knowledge Ingest | `negativeDependencyVerification.test.ts` |

---

### Domain 5: Reasoning, Planning, & Subagents

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Microcompaction (0 model calls)** | `utils/microcompaction.ts` | Claude TS | `brain-services::log_compaction` | **`ADAPT`** | Local tool-pruning rule | Phase 8.4 | None | `lifecycleContracts.test.ts` (State 14a) |
| **Autocompaction Summarization** | `utils/autocompaction.ts` | Claude TS | `brain-services::reflection_pipeline` | **`REPLACE`** | Brain Consolidation Planner | **Phase 8.4** | Brain Reflection Engine v2 | `lifecycleContracts.test.ts` (State 14b) |
| **Task Planning & DAG Execution** | None (Single-threaded loop) | — | `brain-services::execution_planner`, `pass_dag` | **`REPLACE`** | Rust Execution Engine | **Phase 8.5** | None (Brain Innovation) | `plan_optimizer_tests.rs` |
| **Subagents & Swarms** | `tools/AgentTool/` | Claude TS | `brain-services::agent_tests`, `supervision_events` | **`ADAPT`** | TS Agent UI $\leftrightarrow$ Rust Worker Runtime | Phase 8.5 | Brain Subagent Supervisor | `r26_worker_runtime_tests.rs` |
| **Knowledge Evolution & Reflection** | None | — | `brain-services::evolution_planner_v2` | **`REPLACE`** | Rust Evolution Runtime | **Phase 8.6** | None (Brain Innovation) | `evolution_planner_v2_tests.rs` |

---

### Domain 6: Session Management & Persistence

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **JSONL Session Persistence** | `utils/sessionStorage.ts` | Claude TS | `brain-persistence::sqlite_store` | **`ADAPT`** | Session Bridge Adapter | Phase 8.6 | Brain SQLite Event Store | `lifecycleContracts.test.ts` (State 15) |
| **`/resume` Dialogue Reconstruction** | `utils/resume.ts` | Claude TS | `brain-application::SessionService` | **`ADAPT`** | Brain Session Store $\leftrightarrow$ TS History | Phase 8.6 | Brain Replay Engine | `lifecycleContracts.test.ts` (State 15) |
| **Diagnostics & Doctor Tooling** | `commands/doctor/` | Claude TS | `brain-services::observability_diagnostics` | **`KEEP`** | TS Doctor UI $\leftrightarrow$ Brain Health IPC | None (Phase 7B Wave 4) | Never | `lifecycleContracts.test.ts` (State 16) |

---

### Domain 7: Outbound Network, Telemetry, & Auth (Dead-Code Retirement)

| Capability / Feature | Vendored Claude Path | Current Owner | Brain Target Equivalent | Decision | Boundary / Seam | Migration Order | Removal Pre-req | Regression Contract |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Anthropic HTTP Client** | `services/api/client.ts` | Claude TS | None (Zero Outbound Network) | **`REMOVE_LATER`** | Bypassed via `QueryDeps.callModel` | Phase 9.0 | Complete Brain Model Gateway | `negativeDependencyVerification.test.ts` (Inv 1) |
| **Statsig / Growthbook Telemetry** | `services/analytics/` | Claude TS | None (Zero External Telemetry) | **`REMOVE_LATER`** | Sinks unattached in `main.tsx` | Phase 9.0 | Standalone Brain Shell build | `negativeDependencyVerification.test.ts` (Inv 3) |
| **Anthropic OAuth Auth Flow** | `utils/auth.ts` | Claude TS | Brain Local Credentials | **`REMOVE_LATER`** | Mock token in test, local key in prod | Phase 9.0 | Standalone Brain Shell build | `negativeDependencyVerification.test.ts` (Inv 2) |
| **Claude Auto-Updater** | `utils/autoUpdater.ts` | Claude TS | None (Managed via Cargo/Bun) | **`REMOVE_LATER`** | Disabled via `DISABLE_AUTOUPDATER=1` | Phase 9.0 | Standalone Brain Shell build | `negativeDependencyVerification.test.ts` (Inv 4) |

---

## 3. Deep-Dive: Key Seams and Architectural Boundaries

### 3.1 Memory & Context Synthesis Seam (Phase 8.1 – 8.3)
In standard Claude Code, user prompts are concatenated naively with local transcript turns and static file attachments:
```
Standard Claude:
User Prompt ──► Array of JSONL Messages ──► Anthropic API
```

In Brain, memory is an **active relational retrieval & ranking process**:
```
Brain Architecture:
User Prompt
     │
     ├──► [1] Short-Term Working Memory (STM)
     ├──► [2] Long-Term Episodic Memory (LTM SQLite)
     ├──► [3] BM25 Full-Text Search (FTS5)
     ├──► [4] Semantic Vector Embeddings
     └──► [5] Relational Knowledge Graph (Adjacency & Traversal)
           │
           ▼
     [Brain Hybrid Evaluator & LambdaMART Ranking]
           │
           ▼
     [Brain Context Synthesis Engine]
           │
           ▼
     Dynamic Context Payload ──► Model Gateway (via UDS) ──► Monotonic Stream
```
**Contract Invariant**: The Claude UI is completely decoupled from this pipeline. It simply provides the user prompt to `QueryDeps.callModel` and renders the resulting streamed tokens via the typewriter queue.

---

### 3.2 Tool Execution & Supervision Seam (Phase 8.3)
In the Brain architecture, the tool ownership boundary verified in Phase 7B remains strictly enforced:
- **Brain Emits**: `tool_use` event containing tool name, ID, and input arguments.
- **Claude Owns**: Tool permission evaluation (`canUseTool`), user approval dialog rendering, `Tool.call()` execution on the local host, and error presentation.
- **Claude Feeds Back**: Tool result block to Brain via the next turn of `QueryDeps.callModel`.
- **Brain Ingests**: Tool results into the Knowledge Graph and Fact Store for reflection and learning.

---

### 3.3 Session & Compaction Seam (Phase 8.4 & 8.6)
- **Microcompaction**: Retained entirely on the TypeScript side for immediate, zero-latency pruning of redundant historical tool output.
- **Autocompaction & Consolidation**: When the conversation reaches threshold tokens, Brain's **Reflection Engine v2** takes over summarization, extracting permanent facts and relationship changes into the Knowledge Graph before emitting the condensed dialogue context.

---

## 4. Phased Migration Roadmap

```
Phase 7D (Complete) ──► Phase 8.1 (Memory) ──► Phase 8.2 (Graph & Ranking) ──► Phase 8.3 (Context)
                             │
                             ▼
Phase 9.0 (Retirement) ◄── Phase 8.6 (Sessions) ◄── Phase 8.5 (Planning) ◄── Phase 8.4 (Reflection)
```

### Phase 8 Execution Order:
1. **Phase 8.1 — Memory Seam Integration**: Connect `brain-domain::stm` and `brain-services::SessionMemory` to `QueryDeps.callModel`, replacing Claude's flat memory.
2. **Phase 8.2 — Knowledge Graph & Hybrid Ranking**: Enable relational graph retrieval and BM25 search in context preparation.
3. **Phase 8.3 — Context Synthesis & Tool Ingestion**: Route tool execution results into Brain's fact extraction pipeline.
4. **Phase 8.4 — Reflection & Autocompaction**: Replace Claude autocompaction with Brain Reflection Engine v2.
5. **Phase 8.5 — Task DAG Planning & Subagents**: Connect Brain execution planner to TS background mode and agent swarms.
6. **Phase 8.6 — Durable Session Persistence**: Unify JSONL transcript storage with Brain SQLite Event Store.
7. **Phase 9.0 — Dead-Code Retirement**: Isolate and prune dead Claude telemetry, auto-updater, and Anthropic HTTP client files with zero regressions.

---

## 5. Migration Guardrails & Regression Defense

Every incremental migration step must satisfy the following invariant checklist:

1. **Zero Vendor Mutations During Phase 8**: `vendor/claude/` remains frozen at 1,925 files.
2. **Double-Seam Verification**: Every adapted capability must pass both:
   - TypeScript frontend contract test (e.g., `src/test/frontend-contract/*.test.ts`)
   - Rust backend unit/integration test (e.g., `cargo test -p brain-services`)
3. **Differential Regression Hard Gate**: The live differential runner (`differentialAuditRunner.py`) must maintain 0 unclassified differences across all 20 categories.
4. **Negative Dependency Preservation**: Zero Anthropic network calls, zero telemetry, and zero React state leakage into domain DTOs.
