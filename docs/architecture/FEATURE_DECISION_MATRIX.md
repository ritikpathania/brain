# Feature Decision Matrix Grouped by Capability Family

**Document Version:** 1.0.0  
**Architectural Objective:** `Claude Frontend/UX + Brain Backend/Domain + Brain Extensions = Brain Superset`  
**Status:** **AWAITING USER REVIEW & SIGN-OFF**  

---

## 1. Architectural Principles & Decision Framework

Every capability in the frozen Claude v2.1.233 codebase is explicitly decoupled across two independent decision axes:

1. **Frontend Action (`KEEP | MODIFY | REMOVE`):**
   - **`KEEP`**: Retain the canonical Claude visual representation, keyboard shortcuts, animations, and interaction flow.
   - **`MODIFY`**: Adapt UI elements (e.g. model dropdown choices, diagnostic items) to expose Brain capabilities using Claude-native primitives.
   - **`REMOVE`**: Exclude only obsolete, internal employee telemetry, or non-functional novelty features.

2. **Backend Action (`KEEP | MODIFY | REPLACE | REMOVE | EXTERNAL`):**
   - **`KEEP`**: Preserve local TypeScript implementation (e.g., config file writes, theme persistence, vim modal logic, local diffing).
   - **`MODIFY`**: Adapt TypeScript handlers to synchronize state or bridge requests with the Rust Brain daemon.
   - **`REPLACE`**: Replace Claude's implementation with the authoritative Rust Brain backend (e.g., SQLite WAL storage, hybrid RRF retrieval, domain knowledge graph, session compaction).
   - **`EXTERNAL`**: Quarantine capabilities requiring Anthropic cloud infrastructure, remote OAuth, or SaaS webhooks.
   - **`REMOVE`**: Remove unreachable or internal backend handlers.

---

## 2. Capability Family Summary Dashboard

| Family | Name | Capabilities | Frontend Action Distribution | Backend Action Distribution |
| :--- | :--- | :--- | :--- | :--- |
| `fam_conv` | **Family 1: Core Conversational REPL & Streaming UX** | 15 | `KEEP: 13, MOD: 2, REM: 0` | `KEEP: 10, MOD: 0, REP: 5, EXT: 0, REM: 0` |
| `fam_composer` | **Family 2: Composer & Input Interaction** | 10 | `KEEP: 8, MOD: 2, REM: 0` | `KEEP: 8, MOD: 0, REP: 0, EXT: 2, REM: 0` |
| `fam_modes` | **Family 3: Modes, Preferences & Presentation Styling** | 16 | `KEEP: 14, MOD: 2, REM: 0` | `KEEP: 14, MOD: 2, REP: 0, EXT: 0, REM: 0` |
| `fam_memory` | **Family 4: Memory, Knowledge & Context Management** | 11 | `KEEP: 0, MOD: 11, REM: 0` | `KEEP: 0, MOD: 0, REP: 11, EXT: 0, REM: 0` |
| `fam_devtools` | **Family 5: Local Developer & Filesystem Tools** | 20 | `KEEP: 20, MOD: 0, REM: 0` | `KEEP: 20, MOD: 0, REP: 0, EXT: 0, REM: 0` |
| `fam_swarm` | **Family 6: Multi-Agent Swarm, Tasks & Coordination** | 14 | `KEEP: 14, MOD: 0, REM: 0` | `KEEP: 11, MOD: 3, REP: 0, EXT: 0, REM: 0` |
| `fam_plugins_mcp` | **Family 7: Extensibility, Plugins & Model Context Protocol (MCP)** | 14 | `KEEP: 14, MOD: 0, REM: 0` | `KEEP: 10, MOD: 4, REP: 0, EXT: 0, REM: 0` |
| `fam_system` | **Family 8: Diagnostic, Configuration & System Lifecycle** | 27 | `KEEP: 26, MOD: 1, REM: 0` | `KEEP: 26, MOD: 1, REP: 0, EXT: 0, REM: 0` |
| `fam_external` | **Family 9: External Cloud, Auth & Quarantined Integrations** | 16 | `KEEP: 16, MOD: 0, REM: 0` | `KEEP: 0, MOD: 0, REP: 0, EXT: 16, REM: 0` |
| `fam_deprecated` | **Family 10: Claude-Specific Internal / Deprecated Utilities** | 2 | `KEEP: 0, MOD: 0, REM: 2` | `KEEP: 0, MOD: 0, REP: 0, EXT: 0, REM: 2` |
| `fam_brain_ext` | **Family 11: Brain-Specific Extensions (Exposed through Claude-Native UI)** | 3 | `KEEP: 3, MOD: 0, REM: 0` | `KEEP: 0, MOD: 0, REP: 3, EXT: 0, REM: 0` |

---

## 3. Detailed Decision Matrix by Capability Family

### Family 1: Core Conversational REPL & Streaming UX
> **Description:** Lifecycle of conversational turns, streaming token buffers, typewriter rendering, thinking/reasoning blocks, structured diff previews, interruption, and transcript management.  
> **Total Capabilities in Family:** 15

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Session Creation & Initialization`** | App mounts -> generates session UUID -> loads... | `bootstrap/state.ts:initSession(), utils/...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Resume Past Session (/resume)`** | User triggers /resume -> reads project sessio... | `commands/resume/index.ts, utils/sessionS...` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-storage SQLite session even...` | Adapt LogSelector to discover and load stored... |
| **`Token Streaming & Typewriter Drain`** | Query executes -> receives stream events -> b... | `query.ts:query(), query/deps.ts:callMode...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`KEEP`** | **`REPLACE`** | `crates/brain-services UDS monotonic Stre...` | Preserve Claude streaming UI and typewriter q... |
| **`Reasoning & Thinking Blocks (ThinkingConfig)`** | Receives `thinking` stream events -> renders ... | `utils/thinking.ts, query.ts (Model Requi...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`KEEP`** | **`REPLACE`** | `packages/brain-shell/src/adapter/brainCa...` | Preserve collapsible thinking box UI while re... |
| **`Structured Diff Rendering`** | Computes file patch delta -> tokenizes line c... | `native-ts/color-diff/index.ts, component...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Response Interruption & Cancellation (Ctrl+C / Escape)`** | User presses cancel key -> CancelRequestHandl... | `hooks/useCancelRequest.ts, utils/message...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`KEEP`** | **`REPLACE`** | `AbortSignal propagation over UDS socket ...` | Propagate AbortController cancellation from C... |
| **`/copy`** | User invokes `/copy` -> resolves command defi... | `commands/copy/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/diff`** | User invokes `/diff` -> resolves command defi... | `commands/diff/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/export`** | User invokes `/export` -> resolves command de... | `commands/export/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/release-notes`** | User invokes `/release-notes` -> resolves com... | `commands/release-notes/index.ts (No Mode...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/rename`** | User invokes `/rename` -> resolves command de... | `commands/rename/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/resume`** | User invokes `/resume` -> resolves command de... | `commands/resume/index.ts (No Model)` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-storage SQLite session even...` | Adapt LogSelector to discover and load stored... |
| **`/session`** | User invokes `/session` -> resolves command d... | `commands/session/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/tag`** | User invokes `/tag` -> resolves command defin... | `commands/tag/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/version`** | User invokes `/version` -> resolves command d... | `commands/version.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |

---

### Family 2: Composer & Input Interaction
> **Description:** Multiline input buffer handling, suggestions popovers (@ file and / command autocomplete), modal Vim input, shell execution mode (!), shortcut help overlay (?), and audio voice streaming.  
> **Total Capabilities in Family:** 10

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Multiline Input (`\` + `Enter`)`** | User types `\` and presses Enter -> PromptInp... | `components/PromptInput/PromptInput.tsx (...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`@ File Path Autocompletion`** | Typing `@` triggers fuzzy file search -> quer... | `utils/suggestions/fileSuggestions.ts, na...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/ Slash Command Autocompletion`** | Typing `/` lists registered commands matching... | `commands/index.ts, utils/suggestions/com...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Keyboard Shortcut Help Menu (?)`** | Pressing `?` on empty input sets `helpOpen=tr... | `components/PromptInput/PromptInput.tsx:o...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Shell / Bash Execution Mode (!)`** | Typing `!` toggles `bashBorder` (`#DC2626`) -... | `tools/BashTool/index.ts, components/Prom...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Modal Vim Editing Mode (/vim)`** | Toggling vim mode switches input renderer to ... | `components/VimTextInput.tsx, commands/vi...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Push-to-Talk Voice Streaming (/voice)`** | User holds voice key -> records audio via nat... | `services/voiceStreamSTT.ts, services/voi...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`MODIFY`** | **`EXTERNAL`** | `Local Whisper / STT engine adapter or ex...` | Claude voice streaming relies on Anthropic We... |
| **`/clear`** | User invokes `/clear` -> resolves command def... | `commands/clear/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/vim`** | User invokes `/vim` -> resolves command defin... | `commands/vim/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/voice`** | User invokes `/voice` -> resolves command def... | `commands/voice/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`MODIFY`** | **`EXTERNAL`** | `Local Whisper / STT engine adapter or ex...` | Claude voice streaming relies on Anthropic We... |

---

### Family 3: Modes, Preferences & Presentation Styling
> **Description:** Interactive permission mode cycling (Shift+Tab), 17-step theme selection with live color diffs, model picker dialog (Alt+P), architect/plan mode, output style configuration, and reasoning token budget limits.  
> **Total Capabilities in Family:** 16

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Permission Mode Cycling (Shift+Tab)`** | Shift+Tab cycles Normal -> Auto-accept -> Byp... | `utils/permissions/permissionSetup.ts, ho...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Theme Selection & Live Diff Preview (/theme)`** | User triggers /theme -> mounts 17-step state ... | `commands/theme/index.ts, utils/theme.ts,...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Model Selection Dialog (/model, Alt+P)`** | User triggers Alt+P -> renders ModelPicker li... | `commands/model/index.ts, utils/model.ts ...` | `ADAPTED` | `OPERATIONAL_RUST` | **`MODIFY`** | **`MODIFY`** | `packages/brain-shell/src/adapter/brainCa...` | Preserve Claude ModelPicker UI but extend sel... |
| **`Architect / Plan Mode (/plan)`** | Plan mode entered -> sets border to `planMode... | `tools/EnterPlanModeTool/, tools/ExitPlan...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/brief`** | User invokes `/brief` -> resolves command def... | `commands/brief.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/color`** | User invokes `/color` -> resolves command def... | `commands/color/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/effort`** | User invokes `/effort` -> resolves command de... | `commands/effort/index.ts (Model Required...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/fast`** | User invokes `/fast` -> resolves command defi... | `commands/fast/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/help`** | User invokes `/help` -> resolves command defi... | `commands/help/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/model`** | User invokes `/model` -> resolves command def... | `commands/model/index.ts (Model Required)` | `ADAPTED` | `OPERATIONAL_RUST` | **`MODIFY`** | **`MODIFY`** | `packages/brain-shell/src/adapter/brainCa...` | Preserve Claude ModelPicker UI but extend sel... |
| **`/output-style`** | User invokes `/output-style` -> resolves comm... | `commands/output-style/index.ts (No Model...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/passes`** | User invokes `/passes` -> resolves command de... | `commands/passes/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/plan`** | User invokes `/plan` -> resolves command defi... | `commands/plan/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/status`** | User invokes `/status` -> resolves command de... | `commands/status/index.ts (Model Required...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/statusline`** | User invokes `/statusline` -> resolves comman... | `commands/statusline.tsx (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/theme`** | User invokes `/theme` -> resolves command def... | `commands/theme/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |

---

### Family 4: Memory, Knowledge & Context Management
> **Description:** Session memory extraction, long-term memory consolidation (AutoDream), prompt context compaction, turn rollback/rewind, context window token visualization, and repository memory initialization.  
> **Total Capabilities in Family:** 11

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Session Memory Periodic Extraction`** | Post-sampling hook triggers -> checks token t... | `services/SessionMemory/sessionMemory.ts,...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-storage (SQLite STM/LTM) + ...` | Replace flat text file notes with Brain's str... |
| **`Auto Dream Background Memory Consolidation`** | Background timer detects idle state -> acquir... | `services/autoDream/autoDream.ts, service...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-domain::Edge::strengthen / ...` | Replace file-lock prompt consolidation with B... |
| **`Context Compaction & Token Summarization`** | Message tokens cross limit -> microcompactMes... | `services/compact/compact.ts, services/co...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-session::SessionContext::co...` | Delegate turn condensation and token budgetin... |
| **`/compact`** | User invokes `/compact` -> resolves command d... | `commands/compact/index.ts (Model Require...` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-session::SessionContext::co...` | Delegate turn condensation and token budgetin... |
| **`/context`** | User invokes `/context` -> resolves command d... | `commands/context/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-services::retrieval token b...` | Render Brain's hybrid context breakdown (STM ... |
| **`/init-verifiers`** | User invokes `/init-verifiers` -> resolves co... | `commands/init-verifiers.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-services project scanner an...` | Synthesize repository profile and populate in... |
| **`/init`** | User invokes `/init` -> resolves command defi... | `commands/init.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-services project scanner an...` | Synthesize repository profile and populate in... |
| **`/project_areas`** | User invokes `/project_areas` -> resolves com... | `commands/insights.ts (Model Required)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-services project scanner an...` | Synthesize repository profile and populate in... |
| **`/memory`** | User invokes `/memory` -> resolves command de... | `commands/memory/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-storage (SQLite STM/LTM) + ...` | Replace flat text file notes with Brain's str... |
| **`/rewind`** | User invokes `/rewind` -> resolves command de... | `commands/rewind/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-storage CheckpointStore rol...` | Replace memory array slicing with transaction... |
| **`/usage`** | User invokes `/usage` -> resolves command def... | `commands/usage/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_RUST` | **`MODIFY`** | **`REPLACE`** | `crates/brain-services::retrieval token b...` | Render Brain's hybrid context breakdown (STM ... |

---

### Family 5: Local Developer & Filesystem Tools
> **Description:** Local file operations (Read, Write, Edit, Glob, Grep), OS shell command execution (Bash, PowerShell), git workflow commands (commit, push, branch, review, worktrees), Jupyter notebook editing, and Language Server Protocol (LSP) diagnostics.  
> **Total Capabilities in Family:** 20

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Language Server Protocol (LSP) Manager`** | Discovers workspace language server (rust-ana... | `services/lsp/LSPServerManager.ts, servic...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TS LSP manager)` | TS LSP server manager interacts directly with... |
| **`Tool: Bash`** | Model generates `Bash` tool_use block -> Tool... | `tools/BashTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: EnterWorktree`** | Model generates `EnterWorktree` tool_use bloc... | `tools/EnterWorktreeTool/prompt.ts (No Mo...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: ExitWorktree`** | Model generates `ExitWorktree` tool_use block... | `tools/ExitWorktreeTool/prompt.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: FileEdit`** | Model generates `FileEdit` tool_use block -> ... | `tools/FileEditTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: FileRead`** | Model generates `FileRead` tool_use block -> ... | `tools/FileReadTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: FileWrite`** | Model generates `FileWrite` tool_use block ->... | `tools/FileWriteTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Glob`** | Model generates `Glob` tool_use block -> Tool... | `tools/GlobTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Grep`** | Model generates `Grep` tool_use block -> Tool... | `tools/GrepTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: LSP`** | Model generates `LSP` tool_use block -> ToolU... | `tools/LSPTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: NotebookEdit`** | Model generates `NotebookEdit` tool_use block... | `tools/NotebookEditTool/prompt.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: PowerShell`** | Model generates `PowerShell` tool_use block -... | `tools/PowerShellTool/prompt.ts (No Model...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: REPL`** | Model generates `REPL` tool_use block -> Tool... | `tools/REPLTool (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/add-dir`** | User invokes `/add-dir` -> resolves command d... | `commands/add-dir/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/branch`** | User invokes `/branch` -> resolves command de... | `commands/branch/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/commit-push-pr`** | User invokes `/commit-push-pr` -> resolves co... | `commands/commit-push-pr.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/commit`** | User invokes `/commit` -> resolves command de... | `commands/commit.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/files`** | User invokes `/files` -> resolves command def... | `commands/files/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/ide`** | User invokes `/ide` -> resolves command defin... | `commands/ide/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/review`** | User invokes `/review` -> resolves command de... | `commands/review.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |

---

### Family 6: Multi-Agent Swarm, Tasks & Coordination
> **Description:** Subagent session spawning, peer-to-peer agent messaging, background task execution/management, team memory synchronization, and task list / todo tracking.  
> **Total Capabilities in Family:** 14

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Tool: ship-audit`** | Model generates `ship-audit` tool_use block -... | `tools/AgentTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Brief`** | Model generates `Brief` tool_use block -> Too... | `tools/BriefTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: SendMessage`** | Model generates `SendMessage` tool_use block ... | `tools/SendMessageTool/prompt.ts (No Mode...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TaskCreate`** | Model generates `TaskCreate` tool_use block -... | `tools/TaskCreateTool/prompt.ts (No Model...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TaskGet`** | Model generates `TaskGet` tool_use block -> T... | `tools/TaskGetTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TaskList`** | Model generates `TaskList` tool_use block -> ... | `tools/TaskListTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TaskOutput`** | Model generates `TaskOutput` tool_use block -... | `tools/TaskOutputTool (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TaskStop`** | Model generates `TaskStop` tool_use block -> ... | `tools/TaskStopTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TaskUpdate`** | Model generates `TaskUpdate` tool_use block -... | `tools/TaskUpdateTool/prompt.ts (No Model...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TeamCreate`** | Model generates `TeamCreate` tool_use block -... | `tools/TeamCreateTool/prompt.ts (No Model...` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-storage team workspace part...` | Store team workspace facts in partitioned SQL... |
| **`Tool: TeamDelete`** | Model generates `TeamDelete` tool_use block -... | `tools/TeamDeleteTool/prompt.ts (No Model...` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-storage team workspace part...` | Store team workspace facts in partitioned SQL... |
| **`Tool: TodoWrite`** | Model generates `TodoWrite` tool_use block ->... | `tools/TodoWriteTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/agents`** | User invokes `/agents` -> resolves command de... | `commands/agents/index.ts (No Model)` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-a2a-adapter (Agent-to-Agent...` | Preserve Claude subagent UI and progress bars... |
| **`/tasks`** | User invokes `/tasks` -> resolves command def... | `commands/tasks/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |

---

### Family 7: Extensibility, Plugins & Model Context Protocol (MCP)
> **Description:** Model Context Protocol (MCP) tool discovery and resource resolution, plugin marketplace management, skill discovery, local web retrieval tools, and background cron scheduling.  
> **Total Capabilities in Family:** 14

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Tool: ListMcpResources`** | Model generates `ListMcpResources` tool_use b... | `tools/ListMcpResourcesTool/prompt.ts (No...` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-mcp-adapter (Rust MCP manag...` | Preserve Claude MCP configuration UI while sy... |
| **`Tool: MCP`** | Model generates `MCP` tool_use block -> ToolU... | `tools/MCPTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-mcp-adapter (Rust MCP manag...` | Preserve Claude MCP configuration UI while sy... |
| **`Tool: ReadMcpResource`** | Model generates `ReadMcpResource` tool_use bl... | `tools/ReadMcpResourceTool/prompt.ts (No ...` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-mcp-adapter (Rust MCP manag...` | Preserve Claude MCP configuration UI while sy... |
| **`Tool: ScheduleCron`** | Model generates `ScheduleCron` tool_use block... | `tools/ScheduleCronTool/prompt.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Skill`** | Model generates `Skill` tool_use block -> Too... | `tools/SkillTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Sleep`** | Model generates `Sleep` tool_use block -> Too... | `tools/SleepTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: WebFetch`** | Model generates `WebFetch` tool_use block -> ... | `tools/WebFetchTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: WebSearch`** | Model generates `WebSearch` tool_use block ->... | `tools/WebSearchTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Workflow`** | Model generates `Workflow` tool_use block -> ... | `tools/WorkflowTool (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/createMovedToPluginCommand`** | User invokes `/createMovedToPluginCommand` ->... | `commands/createMovedToPluginCommand.ts (...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/mcp`** | User invokes `/mcp` -> resolves command defin... | `commands/mcp/index.ts (No Model)` | `MOUNTED_CANONICAL` | `PARTIAL_RUST` | **`KEEP`** | **`MODIFY`** | `crates/brain-mcp-adapter (Rust MCP manag...` | Preserve Claude MCP configuration UI while sy... |
| **`/plugin`** | User invokes `/plugin` -> resolves command de... | `commands/plugin/index.tsx (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/reload-plugins`** | User invokes `/reload-plugins` -> resolves co... | `commands/reload-plugins/index.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/skills`** | User invokes `/skills` -> resolves command de... | `commands/skills/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |

---

### Family 8: Diagnostic, Configuration & System Lifecycle
> **Description:** Environment diagnostics (Doctor screen), configuration and keybindings management, sandboxing and security reviews, terminal setup, user questions, and advice consultation.  
> **Total Capabilities in Family:** 27

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Tool: AskUserQuestion`** | Model generates `AskUserQuestion` tool_use bl... | `tools/AskUserQuestionTool/prompt.ts (No ...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Config`** | Model generates `Config` tool_use block -> To... | `tools/ConfigTool/prompt.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: EnterPlanMode`** | Model generates `EnterPlanMode` tool_use bloc... | `tools/EnterPlanModeTool/prompt.ts (No Mo...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: ExitPlanMode`** | Model generates `ExitPlanMode` tool_use block... | `tools/ExitPlanModeTool/prompt.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: SyntheticOutput`** | Model generates `SyntheticOutput` tool_use bl... | `tools/SyntheticOutputTool/SyntheticOutpu...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: Search`** | Model generates `Search` tool_use block -> To... | `tools/ToolSearchTool/prompt.ts (No Model...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`Tool: TungstenTool`** | Model generates `TungstenTool` tool_use block... | `tools/TungstenTool/TungstenTool.ts (No M...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/advisor`** | User invokes `/advisor` -> resolves command d... | `commands/advisor.ts (Model Required)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/btw`** | User invokes `/btw` -> resolves command defin... | `commands/btw/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/config`** | User invokes `/config` -> resolves command de... | `commands/config/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/cost`** | User invokes `/cost` -> resolves command defi... | `commands/cost/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/desktop`** | User invokes `/desktop` -> resolves command d... | `commands/desktop/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/doctor`** | User invokes `/doctor` -> resolves command de... | `commands/doctor/index.ts (No Model)` | `ADAPTED` | `OPERATIONAL_RUST` | **`MODIFY`** | **`MODIFY`** | `crates/brain-observability + UDS engine ...` | Adapt Doctor UI to verify local Rust engine d... |
| **`/exit`** | User invokes `/exit` -> resolves command defi... | `commands/exit/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/hooks`** | User invokes `/hooks` -> resolves command def... | `commands/hooks/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/install`** | User invokes `/install` -> resolves command d... | `commands/install.tsx (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/keybindings`** | User invokes `/keybindings` -> resolves comma... | `commands/keybindings/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/permissions`** | User invokes `/permissions` -> resolves comma... | `commands/permissions/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/privacy-settings`** | User invokes `/privacy-settings` -> resolves ... | `commands/privacy-settings/index.ts (No M...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/sandbox`** | User invokes `/sandbox` -> resolves command d... | `commands/sandbox-toggle/index.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/security-review`** | User invokes `/security-review` -> resolves c... | `commands/security-review.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/stats`** | User invokes `/stats` -> resolves command def... | `commands/stats/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/terminal-setup`** | User invokes `/terminal-setup` -> resolves co... | `commands/terminalSetup/index.ts (No Mode...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/think-back`** | User invokes `/think-back` -> resolves comman... | `commands/thinkback/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/thinkback-play`** | User invokes `/thinkback-play` -> resolves co... | `commands/thinkback-play/index.ts (No Mod...` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/ultraplan`** | User invokes `/ultraplan` -> resolves command... | `commands/ultraplan.tsx (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |
| **`/upgrade`** | User invokes `/upgrade` -> resolves command d... | `commands/upgrade/index.ts (No Model)` | `MOUNTED_CANONICAL` | `OPERATIONAL_LOCAL_TS` | **`KEEP`** | **`KEEP`** | `None (Preserve local TypeScript implemen...` | Local interaction model is completely self-co... |

---

### Family 9: External Cloud, Auth & Quarantined Integrations
> **Description:** Anthropic OAuth authentication, remote teleport / SSH bridge infrastructure, SaaS app webhooks, GitHub PR comments / autofix, cloud billing limit management, and telemetry sinks.  
> **Total Capabilities in Family:** 16

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`Tool: McpAuth`** | Model generates `McpAuth` tool_use block -> T... | `tools/McpAuthTool/McpAuthTool.ts (No Mod...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`Tool: RemoteTrigger`** | Model generates `RemoteTrigger` tool_use bloc... | `tools/RemoteTriggerTool/prompt.ts (No Mo...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/remote-control`** | User invokes `/remote-control` -> resolves co... | `commands/bridge/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/bridge-kick`** | User invokes `/bridge-kick` -> resolves comma... | `commands/bridge-kick.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/chrome`** | User invokes `/chrome` -> resolves command de... | `commands/chrome/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/extra-usage`** | User invokes `/extra-usage` -> resolves comma... | `commands/extra-usage/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/feedback`** | User invokes `/feedback` -> resolves command ... | `commands/feedback/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/install-github-app`** | User invokes `/install-github-app` -> resolve... | `commands/install-github-app/index.ts (No...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/install-slack-app`** | User invokes `/install-slack-app` -> resolves... | `commands/install-slack-app/index.ts (No ...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/login`** | User invokes `/login` -> resolves command def... | `commands/login/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/logout`** | User invokes `/logout` -> resolves command de... | `commands/logout/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/mobile`** | User invokes `/mobile` -> resolves command de... | `commands/mobile/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/pr-comments`** | User invokes `/pr-comments` -> resolves comma... | `commands/pr_comments/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/rate-limit-options`** | User invokes `/rate-limit-options` -> resolve... | `commands/rate-limit-options/index.ts (No...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/remote-env`** | User invokes `/remote-env` -> resolves comman... | `commands/remote-env/index.ts (No Model)` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |
| **`/web-setup`** | User invokes `/web-setup` -> resolves command... | `commands/remote-setup/index.ts (No Model...` | `MOUNTED_CANONICAL` | `QUARANTINED_EXTERNAL` | **`KEEP`** | **`EXTERNAL`** | `Quarantined external capability module` | Requires Anthropic cloud, remote OAuth consol... |

---

### Family 10: Claude-Specific Internal / Deprecated Utilities
> **Description:** Internal employee telemetry probes, novelty sticker generation, and deprecated debug hooks.  
> **Total Capabilities in Family:** 2

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`/heapdump`** | User invokes `/heapdump` -> resolves command ... | `commands/heapdump/index.ts (No Model)` | `NOT_MOUNTED` | `NOT_APPLICABLE` | **`REMOVE`** | **`REMOVE`** | `None` | Anthropic employee internal telemetry, test h... |
| **`/stickers`** | User invokes `/stickers` -> resolves command ... | `commands/stickers/index.ts (No Model)` | `NOT_MOUNTED` | `NOT_APPLICABLE` | **`REMOVE`** | **`REMOVE`** | `None` | Anthropic employee internal telemetry, test h... |

---

### Family 11: Brain-Specific Extensions (Exposed through Claude-Native UI)
> **Description:** Brain-specific relational memory and retrieval capabilities projected into canonical Claude design system primitives.  
> **Total Capabilities in Family:** 3

| Capability Name | Claude Frontend Behavior | Claude Backend Implementation | Brain Frontend | Brain Backend | Desired Frontend Action | Desired Backend Action | Brain Replacement Implementation | Decision Rationale |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`/graph (1-Hop Knowledge Graph Visualizer)`** | User triggers `/graph` -> queries Rust `Knowl... | `crates/brain-domain/src/graph/ + crates/...` | `ADAPTED` | `OPERATIONAL_RUST` | **`KEEP`** | **`REPLACE`** | `crates/brain-domain/src/graph/ Knowledge...` | Expose Brain's 1-hop knowledge graph via Clau... |
| **`/memory-debug (STM vs LTM Fact Inspector)`** | User triggers `/memory-debug` -> queries shor... | `crates/brain-storage/src/ + crates/brain...` | `ADAPTED` | `OPERATIONAL_RUST` | **`KEEP`** | **`REPLACE`** | `crates/brain-storage STM/LTM fact inspec...` | Expose working memory cache vs SQLite store v... |
| **`/retrieval-debug (Hybrid RRF Fusion Breakdown)`** | User triggers `/retrieval-debug` -> fetches l... | `crates/brain-services/src/retrieval.rs (...` | `ADAPTED` | `OPERATIONAL_RUST` | **`KEEP`** | **`REPLACE`** | `crates/brain-services/src/retrieval.rs R...` | Expose BM25 + Vector + Graph fusion scores vi... |

---

## 4. Summary of Recommended Actions

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   FRONTEND DECISION SUMMARY (148 TOTAL)                │
├────────────────────────────────────────────────────────────────────────┤
│  KEEP (Preserve exact Claude component tree & interaction flow):  128  │
│  MODIFY (Adapt UI to expose Brain models/health/memory facts):      18  │
│  REMOVE (Exclude internal telemetry & novelty sticker commands):    2  │
├────────────────────────────────────────────────────────────────────────┤
│                   BACKEND DECISION SUMMARY (148 TOTAL)                 │
├────────────────────────────────────────────────────────────────────────┤
│  KEEP (Preserve local TypeScript implementation):                   99  │
│  MODIFY (Adapt TS handler to bridge/sync with Rust daemon):         10  │
│  REPLACE (Power from authoritative Rust engine / storage / graph):   19  │
│  EXTERNAL (Quarantine remote cloud / SaaS / OAuth dependencies):    18  │
│  REMOVE (Dead / internal handlers):                                  2  │
└────────────────────────────────────────────────────────────────────────┘
```