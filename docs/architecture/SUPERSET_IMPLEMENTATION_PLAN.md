# Brain Superset Architecture: Layered Implementation Plan & Dependency Graph

**Document Version:** 1.0.0  
**Architectural North Star:** `BRAIN = CLAUDE SUPERSET`  
$$\text{Canonical Claude Frontend/UX} + \text{Authoritative Rust Brain Backend} + \text{Brain-Specific Extensions} = \mathbf{Brain\ Superset}$$  
**Status:** **READY FOR ARCHITECTURAL REVIEW (NO CODE WRITTEN YET)**  

---

## 1. Executive Overview & Phase Alignment

This implementation plan establishes the strict, dependency-ordered roadmap for constructing the **Brain Superset**. 

### 1.1 Alignment with Phase 8.2 (Knowledge Graph & Hybrid Retrieval)
> [!IMPORTANT]
> **Phase 8.2 is not a competing separate track.**  
> In this superset architecture, Phase 8.2's **Hybrid Retrieval (RRF $k=60.0$)**, **Knowledge Graph 1-hop expansion**, and **Authoritative Context Construction** are formally assigned to **Layer 3 (Brain Backend Replacements)**. Phase 8.2 provides the authoritative retrieval engine that replaces Claude's simplistic in-memory turn slicing behind the canonical `QueryDeps.callModel` seam.

---

## 2. Implementation Dependency Graph

```text
┌────────────────────────────────────────────────────────────────────────┐
│             LAYER 1: CLAUDE-LOCAL FUNCTIONALITY (COMPLETE & FROZEN)     │
│   • Multiline Composer (`\`), Shortcuts (`?`), Modal Vim (`/vim`)       │
│   • Autocomplete (`@` file, `/` command), Shell Mode (`!`)              │
│   • 17-Step Theme Picker (`/theme`), Local Diffs (`/diff`), 50+ Cmds    │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│             LAYER 2: BRAIN ADAPTER & SEAMS (IPC FOUNDATION)            │
│   • UDS Protocol Normalizer (`udsClient.ts`, `brainCallModel.ts`)      │
│   • Dynamic Model Picker Gateway (`/model`, `Alt+P`)                   │
│   • Local Engine Diagnostic Health Probes (`Doctor.tsx`, `/doctor`)    │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│             LAYER 3: BRAIN BACKEND REPLACEMENTS (RUST ENGINE & RETRIEVAL)│
│   • Phase 8.2 Hybrid Retrieval: Mathematical RRF ($k=60.0$) + Graph     │
│   • Structured Relational Memory (SQLite WAL STM/LTM) (`/memory`)       │
│   • Domain Knowledge Consolidation (`autoDream` -> `Edge::strengthen`)  │
│   • Authoritative Session Compaction (`/compact`) & Rollback (`/rewind`)│
│   • Unified Session Repository & Resume (`/resume` from SQLite)        │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│             LAYER 4: CROSS-SUBSYSTEM INTEGRATIONS                      │
│   • Swarm Coordination & Agent-to-Agent Memory Bridge (`brain-a2a`)    │
│   • Dynamic MCP Server Manager Synchronization (`brain-mcp-adapter`)   │
│   • Team Workspace Database Partitioning (`brain-storage`)             │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│             LAYER 5: BRAIN-SPECIFIC EXTENSIONS (CLAUDE UI PROJECTIONS) │
│   • `/graph`: 1-Hop Knowledge Graph Visualizer in Claude `Pane`        │
│   • `/memory-debug`: Working Memory vs SQLite Inspector in `ThemedBox` │
│   • `/retrieval-debug`: RRF Fusion Score Breakdown in `StructuredDiff` │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│             LAYER 6: EXTERNAL & QUARANTINED CAPABILITIES BOUNDARY      │
│   • Anthropic OAuth & Teleport Quarantine Boundary Module              │
│   • Explicit Local Offline Notices for Cloud SaaS Commands             │
│   • Local Speech-to-Text Adapter (`/voice` via Local Whisper)          │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Detailed Layer-by-Layer Implementation Specifications

---

### Layer 1 — Claude-Local Functionality

*Local UI and client-side state behaviors that execute 100% in TypeScript without backend replacement.*

- **Target Capabilities (99 total):**
  - Composer: Multiline `\`, history `Ctrl+R`, `@` file completion, `/` slash autocomplete, `?` help menu, `!` shell mode, `/vim` modal input.
  - Display: `StructuredDiff.tsx`, `VirtualMessageList.tsx`, `ThinkingToggle.tsx`, `LogoV2`.
  - Settings & Theme: `/theme` (17-step state machine), `/config`, `/keybindings`, `/permissions`, `/privacy-settings`.
  - Local Tools: `FileReadTool`, `FileWriteTool`, `FileEditTool`, `GlobTool`, `GrepTool`, `BashTool`, `PowerShellTool`, `NotebookEditTool`, `LSPTool`.
  - Local Commands: `/diff`, `/commit`, `/branch`, `/review`, `/copy`, `/brief`, `/effort`, `/passes`, `/fast`, `/color`, `/statusline`, `/status`, `/help`, `/exit`, `/env`, `/stats`, `/cost`, `/export`, `/tag`, `/rename`, `/session`.
- **Prerequisites:** None (Certified under Contract A Parity Gate: 13/13 EXACT_PARITY).
- **Affected Modules:** `packages/brain-shell/src/`, `packages/brain-shell/vendor/claude/components/`, `packages/brain-shell/vendor/claude/commands/`.
- **Claude Reference:** `vendor/claude/commands/`, `vendor/claude/components/PromptInput/`, `vendor/claude/tools/`.
- **Brain Implementation:** Local execution in `packages/brain-shell` utilizing the frozen vendor Claude components unmodified.
- **API / Seam Changes:** None.
- **Migration Strategy:** Maintain current frozen vendor code; enforce zero unintended mutations via hermetic build.
- **Tests & Verification:**
  - Automated PTY behavioral test suite: `packages/brain-shell/src/test/product_parity_gate.py`.
- **Acceptance Criteria:** 100% pass rate across all 13 certified parity gate contracts and local command execution.
- **Rollback Strategy:** Git revert of any shell packaging changes; vendor directory remains write-protected.

---

### Layer 2 — Brain Adapter & Seams

*The foundational communication boundary bridging the canonical TypeScript Claude shell with the native Rust Brain daemon.*

- **Target Capabilities (10 total):**
  - Token & Stream Normalizer (`conv_streaming`, `conv_thinking`, `conv_cancel`)
  - Dynamic Model Picker Gateway (`mode_model_picker`, `/model`, `Alt+P`)
  - Local Engine Diagnostic Health Probes (`Doctor.tsx`, `/doctor`)
  - Shell Lifecycle & IPC Bridge (`bootstrap/state.ts`, `udsClient.ts`)
- **Prerequisites:** Layer 1.
- **Affected Modules:**
  - `packages/brain-shell/src/adapter/brainCallModel.ts`
  - `packages/brain-shell/src/adapter/udsClient.ts`
  - `packages/brain-shell/src/adapter/modelGateway.ts` [NEW]
  - `packages/brain-shell/src/adapter/doctorProbe.ts` [NEW]
- **Claude Reference:** `vendor/claude/query.ts`, `vendor/claude/components/ModelPicker.tsx`, `vendor/claude/screens/Doctor.tsx`.
- **Brain Implementation:**
  - `udsClient.ts`: Strongly-typed JSON-RPC socket client handling monotonic `StreamEvent` frames (`stream_start`, `stream_chunk`, `stream_end`).
  - `brainCallModel.ts`: Bridges `QueryDeps.callModel` to Rust UDS stream, converting reasoning chunks into native `thinking` blocks without signature fabrication.
  - `modelGateway.ts`: Intercepts `ModelPicker` list requests to query available local Ollama/vLLM models and gateway backends from Rust daemon.
  - `doctorProbe.ts`: Replaces remote Anthropic network checks in `Doctor.tsx` with local Rust daemon ping, IPC roundtrip latency test, and SQLite integrity validation.
- **API / Seam Changes:**
  - Introduce `BrainBackendClient::list_models()` and `BrainBackendClient::health_check()` endpoints over UDS socket.
- **Migration Strategy:** Implement adapter modules in `packages/brain-shell/src/adapter/` without modifying `vendor/claude/`. Inject via `QueryDeps` and state context.
- **Tests & Verification:**
  - Unit tests for stream chunk deserialization and reasoning block parsing.
  - IPC mock tests for daemon disconnection, reconnection, and timeout handling.
  - PTY tests for `Alt+P` model switching and `/doctor` diagnostic output.
- **Acceptance Criteria:**
  - Zero dropped stream tokens or corrupted UTF-8 boundaries.
  - `Alt+P` displays both standard model tiers and active Brain local backends.
  - `/doctor` reports green health status for local Rust daemon and SQLite database.
- **Rollback Strategy:** Fall back to mock stream generator in `brainCallModel.ts`.

---

### Layer 3 — Brain Backend Replacements (Rust Engine & Phase 8.2)

*Replacing Claude's simplistic in-memory or text-file backend implementations with the authoritative Rust Brain domain engine.*

- **Target Capabilities (19 total):**
  - Phase 8.2 Hybrid Retrieval & Context Engine: Mathematical RRF ($k=60.0$) + 1-hop Graph expansion
  - Structured Relational Memory: `svc_session_memory`, `/memory` (SQLite STM/LTM)
  - Domain Knowledge Consolidation: `svc_auto_dream` (Domain graph consolidation)
  - Session Compaction & Checkpoint Rollback: `svc_compaction`, `/compact`, `/rewind`
  - Session Persistence & Resume: `conv_resume`, `/resume` (SQLite session event log)
  - Context Telemetry & Inspection: `/context`, `/ctx_viz`, `/usage`
  - Project Initialization: `/init`, `init-verifiers`, `/project_areas`
- **Prerequisites:** Layer 2 (UDS IPC Seam).
- **Affected Modules:**
  - `crates/brain-services/src/retrieval.rs` (Phase 8.2 RRF fusion & 1-hop expansion)
  - `crates/brain-storage/src/` (SQLite tables, WAL configuration, checkpoint store)
  - `crates/brain-domain/src/graph/` (KnowledgeGraph entity & edge strengthening invariants)
  - `crates/brain-session/src/` (SessionContext compaction & turn lifecycle)
  - `packages/brain-shell/src/adapter/sessionStorageAdapter.ts` [NEW]
  - `packages/brain-shell/src/adapter/memoryAdapter.ts` [NEW]
- **Claude Reference:** `vendor/claude/services/SessionMemory/`, `vendor/claude/services/autoDream/`, `vendor/claude/services/compact/`, `vendor/claude/commands/resume/`.
- **Brain Implementation:**
  - **Hybrid Retrieval (Phase 8.2):** Rust `RetrievalService` executes parallel BM25 search, vector cosine search, and STM memory scan; calculates fused score:
    $$RRF(d) = \sum_{m \in \{BM25, Vec, STM\}} \frac{1}{60.0 + r_m(d)}$$
    Applies 1-hop bounded graph expansion for top-ranked entities and packs prompt context within model token budget.
  - **Memory Storage:** Rust SQLite tables (`entities`, `relations`, `observations`) replace Claude `CLAUDE.md` text appends.
  - **Compaction:** Rust `SessionContext::compact` condenses conversation history into a structured checkpoint snapshot in WAL log.
  - **Session Resume:** `sessionStorageAdapter.ts` adapts Claude's `LogSelector` in `ResumeConversation.tsx` to read session turns from Brain SQLite database.
- **API / Seam Changes:**
  - UDS endpoints: `session.resume`, `session.compact`, `session.rewind`, `memory.query`, `memory.store`, `retrieval.context`.
- **Migration Strategy:**
  - Implement and verify Rust engine logic in `crates/brain-*` with 100% unit test coverage.
  - Wire UDS dispatch handlers in daemon.
  - Connect TypeScript adapters to intercept `/memory`, `/compact`, `/rewind`, `/resume` commands.
- **Tests & Verification:**
  - Rust engine test suite (`cargo test --workspace`).
  - Mathematical assertion test for RRF ranking ($k=60.0$) with synthetic score distributions.
  - SQLite WAL concurrency and transaction rollback tests.
  - End-to-end PTY test for session resume and `/compact` invocation.
- **Acceptance Criteria:**
  - Rust context construction replaces in-memory turn slicing with zero context loss.
  - `/memory` queries and renders structured facts from SQLite in Claude UI.
  - `/compact` reduces session token count while preserving memory entities.
  - `/resume` successfully lists and restores past sessions from SQLite.
- **Rollback Strategy:** Isolate Rust domain changes; revert TS session adapters to fallback memory state.

---

### Layer 4 — Cross-Subsystem Integrations

*Coordinating multi-agent swarms, background task pipelines, and external protocol bridges.*

- **Target Capabilities (14 total):**
  - Multi-Agent Swarm: `AgentTool`, `SendMessageTool`, `/agents`, `tool_ship_audit`
  - Background Tasks: `TaskCreateTool`, `TaskGetTool`, `TaskListTool`, `TaskOutputTool`, `TaskStopTool`, `TaskUpdateTool`, `/tasks`
  - Team Workspace Synchronization: `TeamCreateTool`, `TeamDeleteTool`, `services/teamMemorySync`
  - Model Context Protocol: `MCPTool`, `ListMcpResourcesTool`, `ReadMcpResourceTool`, `/mcp`
  - Plugins: `/plugin`, `/reload-plugins`, `createMovedToPluginCommand`
- **Prerequisites:** Layer 3.
- **Affected Modules:**
  - `crates/brain-a2a-adapter/` (Agent-to-Agent protocol & memory sharing)
  - `crates/brain-mcp-adapter/` (Rust background MCP client manager)
  - `packages/brain-shell/src/adapter/swarmCoordinator.ts` [NEW]
  - `packages/brain-shell/src/adapter/mcpSync.ts` [NEW]
- **Claude Reference:** `vendor/claude/tools/AgentTool/`, `vendor/claude/tasks/`, `vendor/claude/services/mcp/`.
- **Brain Implementation:**
  - `swarmCoordinator.ts`: Bridges Claude's `LocalAgentTask` with Rust `brain-a2a-adapter` to allow subagents to share memory graphs and LTM facts.
  - `mcpSync.ts`: Synchronizes MCP server configurations from `.claude.json` / `/mcp` to Rust `brain-mcp-adapter` background daemon.
- **API / Seam Changes:**
  - UDS endpoints: `swarm.spawn_agent`, `swarm.send_message`, `mcp.sync_servers`.
- **Migration Strategy:** Build adapter bridge connecting TS task manager to Rust A2A adapter without altering subagent UI components.
- **Tests & Verification:**
  - Subagent spawning and message exchange integration tests.
  - MCP stdio server registration and tool execution verification.
- **Acceptance Criteria:**
  - Subagents inherit root conversation memory context.
  - MCP servers configured via `/mcp` execute tools through Rust MCP adapter.
- **Rollback Strategy:** Decouple A2A bridge; run subagents in isolated TS shell processes.

---

### Layer 5 — Brain-Specific Extensions (Claude UI Projections)

*Exposing Brain's relational memory, knowledge graph, and retrieval fusion capabilities through canonical Claude design system primitives.*

- **Target Capabilities (3 total):**
  - `/graph`: 1-Hop Knowledge Graph Visualizer & Relationship Inspector
  - `/memory-debug`: STM Working Memory Cache vs LTM SQLite Store Inspector
  - `/retrieval-debug`: Mathematical RRF Fusion Score & Proximity Breakdown
- **Prerequisites:** Layer 3 & Layer 4.
- **Affected Modules:**
  - `packages/brain-shell/src/commands/graph.tsx` [NEW]
  - `packages/brain-shell/src/commands/memoryDebug.tsx` [NEW]
  - `packages/brain-shell/src/commands/retrievalDebug.tsx` [NEW]
- **Claude UI Primitives Used:**
  - `/graph`: `components/design-system/Pane.tsx`, `components/CustomSelect/CustomSelect.tsx`, `components/design-system/ThemedText.tsx`.
  - `/memory-debug`: `components/design-system/ThemedBox.tsx`, `components/design-system/ListItem.tsx`.
  - `/retrieval-debug`: `components/StructuredDiff.tsx`, `components/MarkdownTable.tsx`.
- **Brain Implementation:**
  - Commands query Rust engine over UDS and format payloads into Claude React/Ink components.
  - Zero custom or foreign UI styling; 100% theme token inheritance.
- **API / Seam Changes:**
  - UDS endpoints: `debug.get_graph_snapshot`, `debug.get_memory_tables`, `debug.get_retrieval_telemetry`.
- **Migration Strategy:** Register commands in `commands/index.ts` alongside standard Claude commands.
- **Tests & Verification:**
  - PTY rendering tests for `/graph`, `/memory-debug`, `/retrieval-debug`.
  - Theme compatibility verification (Dark, Light, High Contrast).
- **Acceptance Criteria:**
  - All 3 commands render seamlessly within the Claude TUI layout without visual jitter or column wrapping bugs.
  - Edge weights, fusion ranks, and memory facts reflect live Rust engine state.
- **Rollback Strategy:** Unregister slash command entrypoints.

---

### Layer 6 — External & Quarantined Capabilities Boundary

*Managing remote cloud, SaaS webhooks, Anthropic OAuth, and offline isolation.*

- **Target Capabilities (18 total):**
  - Authentication: `/login`, `/logout`, `/oauth-refresh`, `McpAuthTool`
  - Remote Teleport & Bridges: `/teleport`, `/remote-env`, `/remote-setup`, `/remote-control`, `/bridge`, `/bridge-kick`, `RemoteTriggerTool`
  - SaaS & Remote PRs: `/install-github-app`, `/install-slack-app`, `/chrome`, `/mobile`, `/issue`, `/pr_comments`, `/autofix-pr`
  - Cloud Billing & Limits: `/extra-usage`, `/rate-limit-options`, `/feedback`, `/perf-issue`
  - Speech-to-Text: `/voice`, `useVoice.ts` (Local Whisper adapter option)
- **Prerequisites:** Layer 1–5.
- **Affected Modules:**
  - `packages/brain-shell/src/quarantine/quarantineNotice.tsx` [NEW]
  - `packages/brain-shell/src/adapter/voiceAdapter.ts` [NEW]
- **Claude Reference:** `vendor/claude/services/oauth/`, `vendor/claude/commands/teleport/`, `vendor/claude/services/voiceStreamSTT.ts`.
- **Brain Implementation:**
  - When user triggers a quarantined cloud command, shell renders a clean Claude `ThemedBox` notice:
    ```text
    ┌──────────────────────────────────────────────────────────────┐
    │  Capability Notice: /login                                   │
    │  Brain is operating in Autonomous Local Mode.                │
    │  Anthropic Cloud Authentication is quarantined.             │
    └──────────────────────────────────────────────────────────────┘
    ```
  - For `/voice`, provide optional bridge to local Whisper STT engine while quarantining Anthropic WebSocket endpoint.
- **API / Seam Changes:** None (Purely defensive client-side isolation).
- **Migration Strategy:** Route external command triggers through `quarantineNotice.tsx`.
- **Tests & Verification:**
  - Network isolation tests: verify zero outbound HTTP/WebSocket requests when executing quarantined commands.
- **Acceptance Criteria:**
  - Zero crashes or hangs on cloud commands; clear user-facing notice presented.
- **Rollback Strategy:** No impact on local system capabilities.

---

## 4. Verification & Certification Pipeline

Every implemented capability across all 6 layers must pass the 4-stage verification gate before certification:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        VERIFICATION PIPELINE                           │
├────────────────────────────────────────────────────────────────────────┤
│  Stage 1: Frontend Behavior Test (PTY Component & Interaction)        │
│  Stage 2: Backend Behavior Test (Rust Unit / SQLite Invariants)       │
│  Stage 3: Integration & Seam Test (UDS Serialization & Roundtrip)      │
│  Stage 4: Regression Test Against Frozen Claude Reference (Gate Lock)  │
└────────────────────────────────────────────────────────────────────────┘
```

### Certification Status Levels
1. **`IMPLEMENTED`**: Code written and wired through appropriate layer seam.
2. **`FUNCTIONAL`**: Passes standalone component and backend unit tests.
3. **`QUARANTINED`**: Verified to be defensively isolated without network leaks (Layer 6).
4. **`CERTIFIED`**: Validated end-to-end through automated PTY assertion suite with 0 regressions.

---

## 5. Summary Execution Schedule & Checkpoints

| Layer | Focus Area | Capabilities | Key Milestone | Verification Artifact |
| :--- | :--- | :---: | :--- | :--- |
| **Layer 1** | Claude-Local Functionality | 99 | Contract A Parity Frozen | `packages/brain-shell/src/test/product_parity_gate_results.json` |
| **Layer 2** | Brain Adapter & Seams | 10 | UDS Socket & Model Gateway Operational | `packages/brain-shell/src/test/uds_adapter_suite.ts` |
| **Layer 3** | Brain Backend Replacements (Phase 8.2) | 19 | RRF $k=60.0$, SQLite WAL, Compaction | `crates/brain-services/tests/retrieval_fusion_test.rs` |
| **Layer 4** | Cross-Subsystem Integrations | 14 | Swarm A2A & MCP Synchronization | `packages/brain-shell/src/test/swarm_integration_test.ts` |
| **Layer 5** | Brain-Specific Extensions | 3 | `/graph`, `/memory-debug`, `/retrieval-debug` | `packages/brain-shell/src/test/brain_extensions_test.py` |
| **Layer 6** | External Quarantine Boundary | 18 | Cloud Isolation & Local Whisper STT | `packages/brain-shell/src/test/quarantine_isolation_test.ts` |
| **Final** | **Brain Superset Complete** | **148** | **0 Gaps, 0 Divergence, Certified** | `docs/architecture/SUPERSET_CERTIFICATION_REPORT.md` |
