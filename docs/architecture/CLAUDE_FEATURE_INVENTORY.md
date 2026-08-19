# Claude Feature Inventory & Brain Superset Architecture Document

**Document Version:** 1.1.0 (Comprehensive Source-Traced Edition)  
**Reference Target:** Frozen Reference Claude v2.1.233 (`packages/brain-shell/vendor/claude/`)  
**Product Vision:** `BRAIN = CLAUDE SUPERSET`  
**Status:** **INVENTORY COMPLETE — DECISION QUEUE OPEN**  

---

## 1. Executive Summary

This document establishes the exhaustive, source-verified **Feature Inventory and Reconstruction Architecture** for the complete Claude v2.1.233 codebase.

### 1.1 The Superset Vision
```text
┌────────────────────────────────────────────────────────────────────────┐
│                   CANONICAL CLAUDE FRONTEND & UX                       │
│   (Exact Component Tree, Typography, Borders, Keyboard Shortcuts, TUI)  │
├────────────────────────────────────────────────────────────────────────┤
│                    COMPLETE CLAUDE FEATURE SET                         │
│   (140+ Audited Capabilities: Conversation, Composer, Tools, Commands)  │
├────────────────────────────────────────────────────────────────────────┤
│                   BRAIN-SPECIFIC CAPABILITIES                          │
│   (Knowledge Graph Inspector, Memory Debugger, Retrieval Visualizer)   │
├────────────────────────────────────────────────────────────────────────┤
│                     RUST BRAIN BACKEND ENGINE                          │
│   (Domain Invariants, Storage/WAL, STM/LTM Memory, Hybrid RRF Fusion)   │
└────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Inventory Metrics
- **Total Audited Capabilities:** 145
- **Core Conversation Capabilities:** 6
- **Composer & Input Capabilities:** 7
- **Modes & Policy Capabilities:** 4
- **Memory & Background Services:** 4
- **Tool UX & Execution Capabilities:** 42
- **Registered Commands:** 82

---

## 2. Complete Claude Feature Catalog

### 2.1 `Session Creation & Initialization` (Core Conversation)
- **Feature ID:** `conv_init`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** CLI startup / automatic session spawn
- **Frontend Components:** screens/REPL.tsx, components/App.tsx, components/LogoV2
- **Lifecycle & State Flow:** App mounts -> generates session UUID -> loads config/theme -> initializes REPL state -> displays LogoV2 banner
- **Backend Implementation:** `bootstrap/state.ts:initSession(), utils/sessionStorage.ts:initSession()`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** ~/.claude/sessions/<uuid> directory + state cache
- **Filesystem Requirements:** Write access to config and session directories
- **Brain Equivalent:** `crates/brain-session::SessionManagerImpl + UDS session initialization`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Maintain current TS shell session bootstrap and synchronize session ID over UDS to Rust SessionManager.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.2 `Resume Past Session (/resume)` (Core Conversation)
- **Feature ID:** `conv_resume`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/resume` / CLI flag `--resume`
- **Frontend Components:** screens/ResumeConversation.tsx, components/LogSelector.tsx, components/SessionPreview.tsx
- **Lifecycle & State Flow:** User triggers /resume -> reads project session logs -> presents interactive LogSelector -> user selects turn -> recovers messages and cost state -> mounts REPL
- **Backend Implementation:** `commands/resume/index.ts, utils/sessionStorage.ts:loadAllProjectsMessageLogsProgressive()`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Reads project session jsonl logs
- **Filesystem Requirements:** Read access to session storage files
- **Brain Equivalent:** `crates/brain-storage SQLite event log and session repository`
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Resume UI reads legacy JSONL session logs; Brain SQLite session event log is not yet unified into LogSelector source list.
- **Migration Strategy:** Adapt `utils/sessionStorage.ts` to query Brain SQLite sessions via UDS adapter in addition to filesystem JSONL files.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.3 `Token Streaming & Typewriter Drain` (Core Conversation)
- **Feature ID:** `conv_streaming`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Model response generation turn
- **Frontend Components:** components/MessageResponse.tsx, components/VirtualMessageList.tsx, ink/components/Text.tsx
- **Lifecycle & State Flow:** Query executes -> receives stream events -> buffers in TypewriterQueue -> renders incremental text deltas -> autoscrolls VirtualMessageList
- **Backend Implementation:** `query.ts:query(), query/deps.ts:callModel()`
- **Dependencies:** `local, model`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append assistant turns to session transcript
- **Filesystem Requirements:** Write to session transcript file
- **Brain Equivalent:** `crates/brain-services UDS monotonic StreamEvent chunks via brainCallModel.ts`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Preserve generic `QueryDeps.callModel` seam streaming monotonic events from Brain UDS daemon.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.4 `Reasoning & Thinking Blocks (ThinkingConfig)` (Core Conversation)
- **Feature ID:** `conv_thinking`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Model reasoning generation (adaptive/budgeted tokens)
- **Frontend Components:** components/ThinkingToggle.tsx, components/messages/ThinkingBlock.tsx, components/Spinner.tsx
- **Lifecycle & State Flow:** Receives `thinking` stream events -> renders collapsible ThinkingBlock with elapsed timer -> captures signature -> collapses on text_delta start
- **Backend Implementation:** `utils/thinking.ts, query.ts`
- **Dependencies:** `local, model`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Store thinking blocks in message history DTO
- **Filesystem Requirements:** None
- **Brain Equivalent:** `packages/brain-shell/src/adapter/brainCallModel.ts reasoning blocks`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Preserve native reasoning chunk streaming without fabricating synthetic signatures.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.5 `Structured Diff Rendering` (Core Conversation)
- **Feature ID:** `conv_diffs`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** FileEditTool execution / `/diff` command / Theme preview
- **Frontend Components:** components/StructuredDiff.tsx, components/StructuredDiffList.tsx, components/diff/
- **Lifecycle & State Flow:** Computes file patch delta -> tokenizes line changes -> highlights intra-line word diffs -> renders unified or side-by-side box with red/green backgrounds
- **Backend Implementation:** `native-ts/color-diff/index.ts, components/StructuredDiff.tsx`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** None
- **Filesystem Requirements:** Read working tree file contents
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/components/StructuredDiff.tsx`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen StructuredDiff component hierarchy.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.6 `Response Interruption & Cancellation (Ctrl+C / Escape)` (Core Conversation)
- **Feature ID:** `conv_cancel`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Keystroke `Ctrl+C` or `Escape` while generating response
- **Frontend Components:** components/InterruptedByUser.tsx, hooks/useCancelRequest.ts
- **Lifecycle & State Flow:** User presses cancel key -> CancelRequestHandler triggers AbortController.abort() -> query loop halts -> appends InterruptedByUser notice -> resets composer
- **Backend Implementation:** `hooks/useCancelRequest.ts, utils/messages.ts:createInterruptedMessage()`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Save interruption notice to transcript
- **Filesystem Requirements:** Write to session storage
- **Brain Equivalent:** `packages/brain-shell/src/adapter/brainCallModel.ts AbortSignal handling`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Preserve AbortController wire propagation to Brain UDS cancellation endpoint.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.7 `Multiline Input (`\` + `Enter`)` (Composer & Input)
- **Feature ID:** `comp_multiline`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Trailing backslash + Enter / Option+Enter
- **Frontend Components:** components/PromptInput/PromptInput.tsx, components/BaseTextInput.tsx
- **Lifecycle & State Flow:** User types `\` and presses Enter -> PromptInput enters multiline continuation mode -> expands vertical height -> preserves indentation
- **Backend Implementation:** `components/PromptInput/PromptInput.tsx`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** None
- **Filesystem Requirements:** None
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/components/PromptInput/`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen PromptInput multiline state machine.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.8 `@ File Path Autocompletion` (Composer & Input)
- **Feature ID:** `comp_file_completion`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Typing `@` in prompt composer
- **Frontend Components:** components/PromptInput/ContextSuggestions.tsx, components/PromptInput/PromptInputFooterSuggestions.tsx
- **Lifecycle & State Flow:** Typing `@` triggers fuzzy file search -> queries project directory tree -> renders suggestions popover -> Tab/Enter accepts path
- **Backend Implementation:** `utils/suggestions/fileSuggestions.ts, native-ts/file-index/`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** None
- **Filesystem Requirements:** Read repository files & directories
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/utils/suggestions/fileSuggestions.ts`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen file suggestion system.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.9 `/ Slash Command Autocompletion` (Composer & Input)
- **Feature ID:** `comp_slash_completion`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Typing `/` in prompt composer
- **Frontend Components:** components/PromptInput/ContextSuggestions.tsx, commands/
- **Lifecycle & State Flow:** Typing `/` lists registered commands matching prefix -> displays description and aliases -> Tab/Enter selects command
- **Backend Implementation:** `commands/index.ts, utils/suggestions/commandSuggestions.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** None
- **Filesystem Requirements:** None
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/commands/`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Preserve command registry autocomplete while adapting command handlers.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.10 `Keyboard Shortcut Help Menu (?)` (Composer & Input)
- **Feature ID:** `comp_help_menu`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Pressing `?` on empty input
- **Frontend Components:** components/PromptInput/PromptInputHelpMenu.tsx, components/PromptInput/PromptInputFooter.tsx
- **Lifecycle & State Flow:** Pressing `?` on empty input sets `helpOpen=true` without buffer text pollution -> renders 3-column shortcut catalog -> Escape dismisses
- **Backend Implementation:** `components/PromptInput/PromptInput.tsx:onChange()`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** None
- **Filesystem Requirements:** None
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/components/PromptInput/PromptInputHelpMenu.tsx`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen PromptInputHelpMenu component.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.11 `Shell / Bash Execution Mode (!)` (Composer & Input)
- **Feature ID:** `comp_shell_mode`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Typing `!` in prompt composer
- **Frontend Components:** components/PromptInput/PromptInput.tsx, components/BashModeProgress.tsx
- **Lifecycle & State Flow:** Typing `!` toggles `bashBorder` (`#DC2626`) -> user enters bash command -> Enter executes directly via BashTool with live streaming output
- **Backend Implementation:** `tools/BashTool/index.ts, components/PromptInput/inputModes.ts`
- **Dependencies:** `local, platform`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append command stdout/stderr to session transcript
- **Filesystem Requirements:** Execution access on local system
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/tools/BashTool/`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen BashTool local execution pipeline.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.12 `Modal Vim Editing Mode (/vim)` (Composer & Input)
- **Feature ID:** `comp_vim_mode`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/vim`
- **Frontend Components:** components/VimTextInput.tsx, components/PromptInput/PromptInput.tsx
- **Lifecycle & State Flow:** Toggling vim mode switches input renderer to VimTextInput -> supports Normal/Insert/Visual modes, hjkl navigation, d/y/p verbs
- **Backend Implementation:** `components/VimTextInput.tsx, commands/vim/`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Save `vimMode: true` in settings.json
- **Filesystem Requirements:** Write to settings.json
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/components/VimTextInput.tsx`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen VimTextInput component.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.13 `Push-to-Talk Voice Streaming (/voice)` (Composer & Input)
- **Feature ID:** `comp_voice`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/voice` / Hold-to-talk keybinding
- **Frontend Components:** components/PromptInput/VoiceIndicator.tsx, components/LogoV2/VoiceModeNotice.tsx, hooks/useVoice.ts, hooks/useVoiceIntegration.tsx
- **Lifecycle & State Flow:** User holds voice key -> records audio via native macOS module or SoX -> streams PCM chunks via WebSocket to voice_stream STT -> updates composer text buffer
- **Backend Implementation:** `services/voiceStreamSTT.ts, services/voiceKeyterms.ts, commands/voice/voice.ts`
- **Dependencies:** `network, external_service, authentication, platform`
- **Model Inference Required:** `False` | **Auth Required:** `True` | **Network Required:** `True`
- **Persistence Requirements:** Save `voiceMode: true` in settings.json
- **Filesystem Requirements:** Microphone audio recording permissions
- **Brain Equivalent:** `Local Whisper / STT engine adapter or external toggle`
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires Anthropic voice_stream WebSocket endpoint (`/api/ws/speech_to_text/voice_stream`) and Deepgram STT engine.
- **Migration Strategy:** Quarantine external voice_stream service; adapt hook to support local offline Whisper transcription if enabled.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.14 `Permission Mode Cycling (Shift+Tab)` (Modes & Policies)
- **Feature ID:** `mode_permission_cycle`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Keystroke `Shift+Tab`
- **Frontend Components:** components/PromptInput/PromptInputModeIndicator.tsx, components/AutoModeOptInDialog.tsx
- **Lifecycle & State Flow:** Shift+Tab cycles Normal -> Auto-accept -> Bypass -> Plan mode -> updates border color token (`promptBorder`, `autoAccept`, `planMode`)
- **Backend Implementation:** `utils/permissions/permissionSetup.ts, hooks/toolPermission/`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Persists mode in session state
- **Filesystem Requirements:** None
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/hooks/toolPermission/`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen permission cycle state machine.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.15 `Theme Selection & Live Diff Preview (/theme)` (Modes & Policies)
- **Feature ID:** `mode_theme_picker`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/theme`
- **Frontend Components:** components/ThemePicker.tsx, components/StructuredDiff.tsx, utils/theme.ts
- **Lifecycle & State Flow:** User triggers /theme -> mounts 17-step state machine with StructuredDiff preview -> Arrow keys navigate options -> Enter persists to disk
- **Backend Implementation:** `commands/theme/index.ts, utils/theme.ts, utils/config.ts:saveGlobalConfig()`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Writes `theme` key to `settings.json` / `.claude.json`
- **Filesystem Requirements:** Write access to config directory
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/components/ThemePicker.tsx`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified in Parity Gate)
- **Migration Strategy:** Reuse frozen ThemePicker component.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.16 `Model Selection Dialog (/model, Alt+P)` (Modes & Policies)
- **Feature ID:** `mode_model_picker`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/model` / Keystroke `Alt+P`
- **Frontend Components:** components/ModelPicker.tsx, components/CustomSelect/
- **Lifecycle & State Flow:** User triggers Alt+P -> renders ModelPicker list -> user selects Opus / Sonnet / Haiku / Custom -> updates active model state
- **Backend Implementation:** `commands/model/index.ts, utils/model.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Optionally persists default model in settings.json
- **Filesystem Requirements:** Write to settings.json
- **Brain Equivalent:** `packages/brain-shell/src/adapter/brainCallModel.ts model routing`
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** ModelPicker only lists Anthropic models; does not yet expose configured Brain reasoning backends (e.g. local Ollama, vLLM, Gemini).
- **Migration Strategy:** Adapt ModelPicker list items in TS shell adapter to query active models from Brain UDS daemon.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.17 `Architect / Plan Mode (/plan)` (Modes & Policies)
- **Feature ID:** `mode_plan`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/plan` / Tool `EnterPlanMode`
- **Frontend Components:** components/permissions/EnterPlanModePermissionRequest/, components/permissions/ExitPlanModePermissionRequest/
- **Lifecycle & State Flow:** Plan mode entered -> sets border to `planMode` (`#2563EB`) -> intercepts write tools to read-only -> model synthesizes plan -> ExitPlanMode presents approval diff
- **Backend Implementation:** `tools/EnterPlanModeTool/, tools/ExitPlanModeTool/, utils/plans.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Writes plan markdown to `.claude/plans/<slug>.md`
- **Filesystem Requirements:** Write access to project plan directory
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/tools/EnterPlanModeTool/`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen Plan mode toolchain and markdown plan persistence.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.18 `Session Memory Periodic Extraction` (Memory & Services)
- **Feature ID:** `svc_session_memory`
- **Classification:** **`ANTHROPIC_MODEL_DEPENDENT`**
- **User Entry Point:** Automatic background post-sampling hook
- **Frontend Components:** services/SessionMemory/sessionMemory.ts
- **Lifecycle & State Flow:** Post-sampling hook triggers -> checks token thresholds -> spawns forked subagent via `runForkedAgent` -> updates conversation notes markdown file
- **Backend Implementation:** `services/SessionMemory/sessionMemory.ts, services/SessionMemory/prompts.ts`
- **Dependencies:** `local, filesystem, model`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Writes conversation notes to session memory path
- **Filesystem Requirements:** Write access to session memory directory
- **Brain Equivalent:** `crates/brain-storage (SQLite STM/LTM) + crates/brain-domain`
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Claude writes markdown notes files; Brain manages structured entities and relations in SQLite database.
- **Migration Strategy:** Bridge session memory extractions into Rust `brain-storage` entity repository.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.19 `Auto Dream Background Memory Consolidation` (Memory & Services)
- **Feature ID:** `svc_auto_dream`
- **Classification:** **`ANTHROPIC_MODEL_DEPENDENT`**
- **User Entry Point:** Background idle memory consolidation timer
- **Frontend Components:** services/autoDream/autoDream.ts
- **Lifecycle & State Flow:** Background timer detects idle state -> acquires lockfile `consolidationLock.ts` -> runs forked agent -> consolidates long-term user preferences
- **Backend Implementation:** `services/autoDream/autoDream.ts, services/autoDream/consolidationPrompt.ts`
- **Dependencies:** `local, filesystem, model`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Writes consolidated memories to disk
- **Filesystem Requirements:** Write access to consolidation lockfile and memory store
- **Brain Equivalent:** `crates/brain-domain::Edge::strengthen / KnowledgeGraph::consolidate`
- **Brain Status:** **`BRAIN_BACKEND_AVAILABLE`**
- **Implementation Gap:** Claude uses file locks and LLM prompt consolidation; Brain uses transactional SQLite WAL and domain aggregate methods.
- **Migration Strategy:** Delegate background consolidation to Rust domain engine.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.20 `Context Compaction & Token Summarization` (Memory & Services)
- **Feature ID:** `svc_compaction`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Token threshold exceeded / Slash command `/compact`
- **Frontend Components:** services/compact/, commands/compact/
- **Lifecycle & State Flow:** Message tokens cross limit -> microcompactMessages runs -> LLM generates condensation summary -> collapses prior turns -> clears pre-compact file cache
- **Backend Implementation:** `services/compact/compact.ts, services/compact/prompt.ts, services/compact/postCompactCleanup.ts`
- **Dependencies:** `local, model`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Overwrites active session transcript with compacted turn
- **Filesystem Requirements:** Write to session storage
- **Brain Equivalent:** `crates/brain-session::SessionContext::compact`
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Claude summarizes in-memory turn list; Brain persists compacted snapshot in SQLite WAL log.
- **Migration Strategy:** Delegate session compaction directly to Rust `SessionContext::compact`.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.21 `Language Server Protocol (LSP) Manager` (Memory & Services)
- **Feature ID:** `svc_lsp`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** LSPTool invocation / Code diagnostics hook
- **Frontend Components:** services/lsp/LSPServerManager.ts, components/LspRecommendation/
- **Lifecycle & State Flow:** Discovers workspace language server (rust-analyzer, tsserver, pyright) -> spawns stdio process -> queries hover, definitions, diagnostics -> injects context
- **Backend Implementation:** `services/lsp/LSPServerManager.ts, services/lsp/LSPClient.ts, tools/LSPTool/`
- **Dependencies:** `local, platform`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** None
- **Filesystem Requirements:** Execute installed language server binaries
- **Brain Equivalent:** `packages/brain-shell/vendor/claude/services/lsp/`
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (TS LSP client spawns local binaries directly)
- **Migration Strategy:** Reuse frozen LSP service architecture.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.22 `Tool: ship-audit` (Tool UX & Execution)
- **Feature ID:** `tool_ship_audit`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `ship-audit`
- **Frontend Components:** tools/AgentTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `ship-audit` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/AgentTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `ship-audit``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.23 `Tool: AskUserQuestion` (Tool UX & Execution)
- **Feature ID:** `tool_askuserquestion`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `AskUserQuestion`
- **Frontend Components:** tools/AskUserQuestionTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `AskUserQuestion` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/AskUserQuestionTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `AskUserQuestion``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.24 `Tool: Bash` (Tool UX & Execution)
- **Feature ID:** `tool_bash`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Bash`
- **Frontend Components:** tools/BashTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Bash` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/BashTool/prompt.ts`
- **Dependencies:** `local, filesystem, platform`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Bash``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.25 `Tool: Brief` (Tool UX & Execution)
- **Feature ID:** `tool_brief`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Brief`
- **Frontend Components:** tools/BriefTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Brief` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/BriefTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Brief``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.26 `Tool: Config` (Tool UX & Execution)
- **Feature ID:** `tool_config`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Config`
- **Frontend Components:** tools/ConfigTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Config` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ConfigTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Config``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.27 `Tool: EnterPlanMode` (Tool UX & Execution)
- **Feature ID:** `tool_enterplanmode`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `EnterPlanMode`
- **Frontend Components:** tools/EnterPlanModeTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `EnterPlanMode` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/EnterPlanModeTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `EnterPlanMode``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.28 `Tool: EnterWorktree` (Tool UX & Execution)
- **Feature ID:** `tool_enterworktree`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `EnterWorktree`
- **Frontend Components:** tools/EnterWorktreeTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `EnterWorktree` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/EnterWorktreeTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `EnterWorktree``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.29 `Tool: ExitPlanMode` (Tool UX & Execution)
- **Feature ID:** `tool_exitplanmode`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `ExitPlanMode`
- **Frontend Components:** tools/ExitPlanModeTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `ExitPlanMode` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ExitPlanModeTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `ExitPlanMode``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.30 `Tool: ExitWorktree` (Tool UX & Execution)
- **Feature ID:** `tool_exitworktree`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `ExitWorktree`
- **Frontend Components:** tools/ExitWorktreeTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `ExitWorktree` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ExitWorktreeTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `ExitWorktree``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.31 `Tool: FileEdit` (Tool UX & Execution)
- **Feature ID:** `tool_fileedit`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `FileEdit`
- **Frontend Components:** tools/FileEditTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `FileEdit` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/FileEditTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `FileEdit``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.32 `Tool: FileRead` (Tool UX & Execution)
- **Feature ID:** `tool_fileread`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `FileRead`
- **Frontend Components:** tools/FileReadTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `FileRead` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/FileReadTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `FileRead``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.33 `Tool: FileWrite` (Tool UX & Execution)
- **Feature ID:** `tool_filewrite`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `FileWrite`
- **Frontend Components:** tools/FileWriteTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `FileWrite` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/FileWriteTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `FileWrite``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.34 `Tool: Glob` (Tool UX & Execution)
- **Feature ID:** `tool_glob`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Glob`
- **Frontend Components:** tools/GlobTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Glob` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/GlobTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Glob``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.35 `Tool: Grep` (Tool UX & Execution)
- **Feature ID:** `tool_grep`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Grep`
- **Frontend Components:** tools/GrepTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Grep` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/GrepTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Grep``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.36 `Tool: LSP` (Tool UX & Execution)
- **Feature ID:** `tool_lsp`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `LSP`
- **Frontend Components:** tools/LSPTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `LSP` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/LSPTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `LSP``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.37 `Tool: ListMcpResources` (Tool UX & Execution)
- **Feature ID:** `tool_listmcpresources`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Model invocation of tool `ListMcpResources`
- **Frontend Components:** tools/ListMcpResourcesTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `ListMcpResources` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ListMcpResourcesTool/prompt.ts`
- **Dependencies:** `local, external_service`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `ListMcpResources``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** TS MCP client operates frontend tools; Rust `brain-mcp-adapter` manages background servers.
- **Migration Strategy:** Synchronize server definitions over UDS.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.38 `Tool: MCP` (Tool UX & Execution)
- **Feature ID:** `tool_mcp`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `MCP`
- **Frontend Components:** tools/MCPTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `MCP` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/MCPTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `MCP``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.39 `Tool: McpAuth` (Tool UX & Execution)
- **Feature ID:** `tool_mcpauth`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Model invocation of tool `McpAuth`
- **Frontend Components:** tools/McpAuthTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `McpAuth` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/McpAuthTool/McpAuthTool.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `True` | **Network Required:** `True`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `McpAuth``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Depends on external authentication / remote trigger service.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.40 `Tool: NotebookEdit` (Tool UX & Execution)
- **Feature ID:** `tool_notebookedit`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `NotebookEdit`
- **Frontend Components:** tools/NotebookEditTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `NotebookEdit` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/NotebookEditTool/prompt.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `NotebookEdit``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.41 `Tool: PowerShell` (Tool UX & Execution)
- **Feature ID:** `tool_powershell`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `PowerShell`
- **Frontend Components:** tools/PowerShellTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `PowerShell` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/PowerShellTool/prompt.ts`
- **Dependencies:** `local, filesystem, platform`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `PowerShell``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.42 `Tool: REPL` (Tool UX & Execution)
- **Feature ID:** `tool_repl`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `REPL`
- **Frontend Components:** tools/REPLTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `REPL` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/REPLTool`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `REPL``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.43 `Tool: ReadMcpResource` (Tool UX & Execution)
- **Feature ID:** `tool_readmcpresource`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Model invocation of tool `ReadMcpResource`
- **Frontend Components:** tools/ReadMcpResourceTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `ReadMcpResource` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ReadMcpResourceTool/prompt.ts`
- **Dependencies:** `local, external_service`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `ReadMcpResource``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** TS MCP client operates frontend tools; Rust `brain-mcp-adapter` manages background servers.
- **Migration Strategy:** Synchronize server definitions over UDS.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.44 `Tool: RemoteTrigger` (Tool UX & Execution)
- **Feature ID:** `tool_remotetrigger`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Model invocation of tool `RemoteTrigger`
- **Frontend Components:** tools/RemoteTriggerTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `RemoteTrigger` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/RemoteTriggerTool/prompt.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `True` | **Network Required:** `True`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `RemoteTrigger``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Depends on external authentication / remote trigger service.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.45 `Tool: ScheduleCron` (Tool UX & Execution)
- **Feature ID:** `tool_schedulecron`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `ScheduleCron`
- **Frontend Components:** tools/ScheduleCronTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `ScheduleCron` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ScheduleCronTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `ScheduleCron``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.46 `Tool: SendMessage` (Tool UX & Execution)
- **Feature ID:** `tool_sendmessage`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `SendMessage`
- **Frontend Components:** tools/SendMessageTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `SendMessage` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/SendMessageTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `SendMessage``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.47 `Tool: Skill` (Tool UX & Execution)
- **Feature ID:** `tool_skill`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Skill`
- **Frontend Components:** tools/SkillTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Skill` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/SkillTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Skill``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.48 `Tool: Sleep` (Tool UX & Execution)
- **Feature ID:** `tool_sleep`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Sleep`
- **Frontend Components:** tools/SleepTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Sleep` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/SleepTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Sleep``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.49 `Tool: SyntheticOutput` (Tool UX & Execution)
- **Feature ID:** `tool_syntheticoutput`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `SyntheticOutput`
- **Frontend Components:** tools/SyntheticOutputTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `SyntheticOutput` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/SyntheticOutputTool/SyntheticOutputTool.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `SyntheticOutput``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.50 `Tool: TaskCreate` (Tool UX & Execution)
- **Feature ID:** `tool_taskcreate`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TaskCreate`
- **Frontend Components:** tools/TaskCreateTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TaskCreate` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TaskCreateTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TaskCreate``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.51 `Tool: TaskGet` (Tool UX & Execution)
- **Feature ID:** `tool_taskget`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TaskGet`
- **Frontend Components:** tools/TaskGetTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TaskGet` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TaskGetTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TaskGet``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.52 `Tool: TaskList` (Tool UX & Execution)
- **Feature ID:** `tool_tasklist`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TaskList`
- **Frontend Components:** tools/TaskListTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TaskList` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TaskListTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TaskList``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.53 `Tool: TaskOutput` (Tool UX & Execution)
- **Feature ID:** `tool_taskoutput`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TaskOutput`
- **Frontend Components:** tools/TaskOutputTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TaskOutput` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TaskOutputTool`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TaskOutput``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.54 `Tool: TaskStop` (Tool UX & Execution)
- **Feature ID:** `tool_taskstop`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TaskStop`
- **Frontend Components:** tools/TaskStopTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TaskStop` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TaskStopTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TaskStop``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.55 `Tool: TaskUpdate` (Tool UX & Execution)
- **Feature ID:** `tool_taskupdate`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TaskUpdate`
- **Frontend Components:** tools/TaskUpdateTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TaskUpdate` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TaskUpdateTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TaskUpdate``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.56 `Tool: TeamCreate` (Tool UX & Execution)
- **Feature ID:** `tool_teamcreate`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TeamCreate`
- **Frontend Components:** tools/TeamCreateTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TeamCreate` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TeamCreateTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TeamCreate``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.57 `Tool: TeamDelete` (Tool UX & Execution)
- **Feature ID:** `tool_teamdelete`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TeamDelete`
- **Frontend Components:** tools/TeamDeleteTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TeamDelete` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TeamDeleteTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TeamDelete``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.58 `Tool: TodoWrite` (Tool UX & Execution)
- **Feature ID:** `tool_todowrite`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TodoWrite`
- **Frontend Components:** tools/TodoWriteTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TodoWrite` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TodoWriteTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TodoWrite``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.59 `Tool: Search` (Tool UX & Execution)
- **Feature ID:** `tool_search`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Search`
- **Frontend Components:** tools/ToolSearchTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Search` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/ToolSearchTool/prompt.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Search``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.60 `Tool: TungstenTool` (Tool UX & Execution)
- **Feature ID:** `tool_tungstentool`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `TungstenTool`
- **Frontend Components:** tools/TungstenTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `TungstenTool` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/TungstenTool/TungstenTool.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `TungstenTool``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.61 `Tool: WebFetch` (Tool UX & Execution)
- **Feature ID:** `tool_webfetch`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Model invocation of tool `WebFetch`
- **Frontend Components:** tools/WebFetchTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `WebFetch` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/WebFetchTool/prompt.ts`
- **Dependencies:** `network`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `WebFetch``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** Requires outbound network connectivity.
- **Migration Strategy:** Reuse frozen TS implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.62 `Tool: WebSearch` (Tool UX & Execution)
- **Feature ID:** `tool_websearch`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Model invocation of tool `WebSearch`
- **Frontend Components:** tools/WebSearchTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `WebSearch` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/WebSearchTool/prompt.ts`
- **Dependencies:** `network`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `WebSearch``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** Requires outbound network connectivity.
- **Migration Strategy:** Reuse frozen TS implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.63 `Tool: Workflow` (Tool UX & Execution)
- **Feature ID:** `tool_workflow`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Model invocation of tool `Workflow`
- **Frontend Components:** tools/WorkflowTool/, components/ToolUseLoader.tsx
- **Lifecycle & State Flow:** Model generates `Workflow` tool_use block -> ToolUseLoader renders progress -> permission check validates -> executes tool -> returns tool_result
- **Backend Implementation:** `tools/WorkflowTool`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Append tool calls and results to session transcript
- **Filesystem Requirements:** Varies by tool
- **Brain Equivalent:** `Brain tool `Workflow``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None
- **Migration Strategy:** Reuse frozen tool implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.64 `/add-dir` (Command: Local)
- **Feature ID:** `cmd_add_dir`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/add-dir`
- **Frontend Components:** commands/add-dir/index.ts
- **Lifecycle & State Flow:** User invokes `/add-dir` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/add-dir/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/add-dir``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.65 `/advisor` (Command: Local)
- **Feature ID:** `cmd_advisor`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/advisor`
- **Frontend Components:** commands/advisor.ts
- **Lifecycle & State Flow:** User invokes `/advisor` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/advisor.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/advisor``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.66 `/agents` (Command: Local)
- **Feature ID:** `cmd_agents`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/agents`
- **Frontend Components:** commands/agents/index.ts
- **Lifecycle & State Flow:** User invokes `/agents` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/agents/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/agents``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.67 `/branch` (Command: Local)
- **Feature ID:** `cmd_branch`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/branch`
- **Frontend Components:** commands/branch/index.ts
- **Lifecycle & State Flow:** User invokes `/branch` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/branch/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/branch``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.68 `/remote-control` (Command: Local)
- **Feature ID:** `cmd_remote_control`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/remote-control`
- **Frontend Components:** commands/bridge/index.ts
- **Lifecycle & State Flow:** User invokes `/remote-control` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/bridge/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/remote-control``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.69 `/bridge-kick` (Command: Local)
- **Feature ID:** `cmd_bridge_kick`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/bridge-kick`
- **Frontend Components:** commands/bridge-kick.ts
- **Lifecycle & State Flow:** User invokes `/bridge-kick` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/bridge-kick.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/bridge-kick``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.70 `/brief` (Command: Local)
- **Feature ID:** `cmd_brief`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/brief`
- **Frontend Components:** commands/brief.ts
- **Lifecycle & State Flow:** User invokes `/brief` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/brief.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/brief``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.71 `/btw` (Command: Local)
- **Feature ID:** `cmd_btw`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/btw`
- **Frontend Components:** commands/btw/index.ts
- **Lifecycle & State Flow:** User invokes `/btw` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/btw/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/btw``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.72 `/chrome` (Command: External Cloud)
- **Feature ID:** `cmd_chrome`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/chrome`
- **Frontend Components:** commands/chrome/index.ts
- **Lifecycle & State Flow:** User invokes `/chrome` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/chrome/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/chrome``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.73 `/clear` (Command: Local)
- **Feature ID:** `cmd_clear`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/clear`
- **Frontend Components:** commands/clear/index.ts
- **Lifecycle & State Flow:** User invokes `/clear` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/clear/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/clear``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.74 `/color` (Command: Local)
- **Feature ID:** `cmd_color`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/color`
- **Frontend Components:** commands/color/index.ts
- **Lifecycle & State Flow:** User invokes `/color` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/color/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/color``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.75 `/commit-push-pr` (Command: Brain Adapted)
- **Feature ID:** `cmd_commit_push_pr`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/commit-push-pr`
- **Frontend Components:** commands/commit-push-pr.ts
- **Lifecycle & State Flow:** User invokes `/commit-push-pr` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/commit-push-pr.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/commit-push-pr``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.76 `/commit` (Command: Local)
- **Feature ID:** `cmd_commit`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/commit`
- **Frontend Components:** commands/commit.ts
- **Lifecycle & State Flow:** User invokes `/commit` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/commit.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/commit``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.77 `/compact` (Command: Brain Adapted)
- **Feature ID:** `cmd_compact`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/compact`
- **Frontend Components:** commands/compact/index.ts
- **Lifecycle & State Flow:** User invokes `/compact` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/compact/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/compact``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Claude summarizes in-memory turns; Brain needs to delegate to `SessionContext::compact`.
- **Migration Strategy:** Delegate session compaction to Rust backend.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.78 `/config` (Command: Local)
- **Feature ID:** `cmd_config`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/config`
- **Frontend Components:** commands/config/index.ts
- **Lifecycle & State Flow:** User invokes `/config` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/config/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/config``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.79 `/context` (Command: Brain Adapted)
- **Feature ID:** `cmd_context`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/context`
- **Frontend Components:** commands/context/index.ts
- **Lifecycle & State Flow:** User invokes `/context` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/context/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/context``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.80 `/copy` (Command: Local)
- **Feature ID:** `cmd_copy`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/copy`
- **Frontend Components:** commands/copy/index.ts
- **Lifecycle & State Flow:** User invokes `/copy` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/copy/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/copy``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.81 `/cost` (Command: Local)
- **Feature ID:** `cmd_cost`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/cost`
- **Frontend Components:** commands/cost/index.ts
- **Lifecycle & State Flow:** User invokes `/cost` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/cost/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/cost``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.82 `/createMovedToPluginCommand` (Command: Local)
- **Feature ID:** `cmd_createMovedToPluginCommand`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/createMovedToPluginCommand`
- **Frontend Components:** commands/createMovedToPluginCommand.ts
- **Lifecycle & State Flow:** User invokes `/createMovedToPluginCommand` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/createMovedToPluginCommand.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/createMovedToPluginCommand``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.83 `/desktop` (Command: Local)
- **Feature ID:** `cmd_desktop`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/desktop`
- **Frontend Components:** commands/desktop/index.ts
- **Lifecycle & State Flow:** User invokes `/desktop` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/desktop/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/desktop``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.84 `/diff` (Command: Local)
- **Feature ID:** `cmd_diff`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/diff`
- **Frontend Components:** commands/diff/index.ts
- **Lifecycle & State Flow:** User invokes `/diff` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/diff/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/diff``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.85 `/doctor` (Command: Brain Adapted)
- **Feature ID:** `cmd_doctor`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/doctor`
- **Frontend Components:** commands/doctor/index.ts
- **Lifecycle & State Flow:** User invokes `/doctor` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/doctor/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/doctor``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Claude runs Anthropic telemetry checks; Brain needs to verify Rust UDS engine & SQLite health.
- **Migration Strategy:** Adapt Doctor diagnostic suite to run local engine probes.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.86 `/effort` (Command: Local)
- **Feature ID:** `cmd_effort`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/effort`
- **Frontend Components:** commands/effort/index.ts
- **Lifecycle & State Flow:** User invokes `/effort` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/effort/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/effort``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.87 `/exit` (Command: Local)
- **Feature ID:** `cmd_exit`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/exit`
- **Frontend Components:** commands/exit/index.ts
- **Lifecycle & State Flow:** User invokes `/exit` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/exit/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/exit``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.88 `/export` (Command: Local)
- **Feature ID:** `cmd_export`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/export`
- **Frontend Components:** commands/export/index.ts
- **Lifecycle & State Flow:** User invokes `/export` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/export/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/export``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.89 `/extra-usage` (Command: External Cloud)
- **Feature ID:** `cmd_extra_usage`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/extra-usage`
- **Frontend Components:** commands/extra-usage/index.ts
- **Lifecycle & State Flow:** User invokes `/extra-usage` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/extra-usage/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/extra-usage``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.90 `/fast` (Command: Local)
- **Feature ID:** `cmd_fast`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/fast`
- **Frontend Components:** commands/fast/index.ts
- **Lifecycle & State Flow:** User invokes `/fast` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/fast/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/fast``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.91 `/feedback` (Command: External Cloud)
- **Feature ID:** `cmd_feedback`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/feedback`
- **Frontend Components:** commands/feedback/index.ts
- **Lifecycle & State Flow:** User invokes `/feedback` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/feedback/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/feedback``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.92 `/files` (Command: Local)
- **Feature ID:** `cmd_files`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/files`
- **Frontend Components:** commands/files/index.ts
- **Lifecycle & State Flow:** User invokes `/files` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/files/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/files``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.93 `/heapdump` (Command: Internal / Telemetry)
- **Feature ID:** `cmd_heapdump`
- **Classification:** **`CLAUDE_SPECIFIC`**
- **User Entry Point:** Slash command `/heapdump`
- **Frontend Components:** commands/heapdump/index.ts
- **Lifecycle & State Flow:** User invokes `/heapdump` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/heapdump/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/heapdump``
- **Brain Status:** **`NEEDS_DECISION`**
- **Implementation Gap:** Anthropic internal telemetry or novelty feature.
- **Migration Strategy:** Queue for explicit user KEEP / MODIFY / REMOVE decision.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.94 `/help` (Command: Local)
- **Feature ID:** `cmd_help`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/help`
- **Frontend Components:** commands/help/index.ts
- **Lifecycle & State Flow:** User invokes `/help` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/help/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/help``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.95 `/hooks` (Command: Local)
- **Feature ID:** `cmd_hooks`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/hooks`
- **Frontend Components:** commands/hooks/index.ts
- **Lifecycle & State Flow:** User invokes `/hooks` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/hooks/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/hooks``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.96 `/ide` (Command: Local)
- **Feature ID:** `cmd_ide`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/ide`
- **Frontend Components:** commands/ide/index.ts
- **Lifecycle & State Flow:** User invokes `/ide` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/ide/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/ide``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.97 `/init-verifiers` (Command: Local)
- **Feature ID:** `cmd_init_verifiers`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/init-verifiers`
- **Frontend Components:** commands/init-verifiers.ts
- **Lifecycle & State Flow:** User invokes `/init-verifiers` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/init-verifiers.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/init-verifiers``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.98 `/init` (Command: Brain Adapted)
- **Feature ID:** `cmd_init`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/init`
- **Frontend Components:** commands/init.ts
- **Lifecycle & State Flow:** User invokes `/init` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/init.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/init``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.99 `/project_areas` (Command: Local)
- **Feature ID:** `cmd_project_areas`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/project_areas`
- **Frontend Components:** commands/insights.ts
- **Lifecycle & State Flow:** User invokes `/project_areas` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/insights.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/project_areas``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.100 `/install-github-app` (Command: External Cloud)
- **Feature ID:** `cmd_install_github_app`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/install-github-app`
- **Frontend Components:** commands/install-github-app/index.ts
- **Lifecycle & State Flow:** User invokes `/install-github-app` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/install-github-app/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/install-github-app``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.101 `/install-slack-app` (Command: External Cloud)
- **Feature ID:** `cmd_install_slack_app`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/install-slack-app`
- **Frontend Components:** commands/install-slack-app/index.ts
- **Lifecycle & State Flow:** User invokes `/install-slack-app` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/install-slack-app/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/install-slack-app``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.102 `/install` (Command: Local)
- **Feature ID:** `cmd_install`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/install`
- **Frontend Components:** commands/install.tsx
- **Lifecycle & State Flow:** User invokes `/install` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/install.tsx`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/install``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.103 `/keybindings` (Command: Local)
- **Feature ID:** `cmd_keybindings`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/keybindings`
- **Frontend Components:** commands/keybindings/index.ts
- **Lifecycle & State Flow:** User invokes `/keybindings` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/keybindings/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/keybindings``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.104 `/login` (Command: External Cloud)
- **Feature ID:** `cmd_login`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/login`
- **Frontend Components:** commands/login/index.ts
- **Lifecycle & State Flow:** User invokes `/login` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/login/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `True` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/login``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.105 `/logout` (Command: External Cloud)
- **Feature ID:** `cmd_logout`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/logout`
- **Frontend Components:** commands/logout/index.ts
- **Lifecycle & State Flow:** User invokes `/logout` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/logout/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/logout``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.106 `/mcp` (Command: Local)
- **Feature ID:** `cmd_mcp`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/mcp`
- **Frontend Components:** commands/mcp/index.ts
- **Lifecycle & State Flow:** User invokes `/mcp` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/mcp/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/mcp``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.107 `/memory` (Command: Brain Adapted)
- **Feature ID:** `cmd_memory`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/memory`
- **Frontend Components:** commands/memory/index.ts
- **Lifecycle & State Flow:** User invokes `/memory` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/memory/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/memory``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Claude writes to CLAUDE.md/MEMORY.md; Brain needs to expose Rust LTM/STM facts.
- **Migration Strategy:** Adapt command to query Rust `brain-storage` and `brain-services` over UDS.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.108 `/mobile` (Command: External Cloud)
- **Feature ID:** `cmd_mobile`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/mobile`
- **Frontend Components:** commands/mobile/index.ts
- **Lifecycle & State Flow:** User invokes `/mobile` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/mobile/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/mobile``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.109 `/model` (Command: Brain Adapted)
- **Feature ID:** `cmd_model`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/model`
- **Frontend Components:** commands/model/index.ts
- **Lifecycle & State Flow:** User invokes `/model` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/model/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/model``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.110 `/output-style` (Command: Local)
- **Feature ID:** `cmd_output_style`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/output-style`
- **Frontend Components:** commands/output-style/index.ts
- **Lifecycle & State Flow:** User invokes `/output-style` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/output-style/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/output-style``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.111 `/passes` (Command: Local)
- **Feature ID:** `cmd_passes`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/passes`
- **Frontend Components:** commands/passes/index.ts
- **Lifecycle & State Flow:** User invokes `/passes` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/passes/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/passes``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.112 `/permissions` (Command: Local)
- **Feature ID:** `cmd_permissions`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/permissions`
- **Frontend Components:** commands/permissions/index.ts
- **Lifecycle & State Flow:** User invokes `/permissions` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/permissions/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/permissions``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.113 `/plan` (Command: Local)
- **Feature ID:** `cmd_plan`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/plan`
- **Frontend Components:** commands/plan/index.ts
- **Lifecycle & State Flow:** User invokes `/plan` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/plan/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/plan``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.114 `/plugin` (Command: Brain Adapted)
- **Feature ID:** `cmd_plugin`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/plugin`
- **Frontend Components:** commands/plugin/index.tsx
- **Lifecycle & State Flow:** User invokes `/plugin` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/plugin/index.tsx`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/plugin``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.115 `/pr-comments` (Command: Local)
- **Feature ID:** `cmd_pr_comments`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/pr-comments`
- **Frontend Components:** commands/pr_comments/index.ts
- **Lifecycle & State Flow:** User invokes `/pr-comments` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/pr_comments/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/pr-comments``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.116 `/privacy-settings` (Command: Local)
- **Feature ID:** `cmd_privacy_settings`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/privacy-settings`
- **Frontend Components:** commands/privacy-settings/index.ts
- **Lifecycle & State Flow:** User invokes `/privacy-settings` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/privacy-settings/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/privacy-settings``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.117 `/rate-limit-options` (Command: External Cloud)
- **Feature ID:** `cmd_rate_limit_options`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/rate-limit-options`
- **Frontend Components:** commands/rate-limit-options/index.ts
- **Lifecycle & State Flow:** User invokes `/rate-limit-options` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/rate-limit-options/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/rate-limit-options``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.118 `/release-notes` (Command: Local)
- **Feature ID:** `cmd_release_notes`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/release-notes`
- **Frontend Components:** commands/release-notes/index.ts
- **Lifecycle & State Flow:** User invokes `/release-notes` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/release-notes/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/release-notes``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.119 `/reload-plugins` (Command: Brain Adapted)
- **Feature ID:** `cmd_reload_plugins`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/reload-plugins`
- **Frontend Components:** commands/reload-plugins/index.ts
- **Lifecycle & State Flow:** User invokes `/reload-plugins` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/reload-plugins/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/reload-plugins``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.120 `/remote-env` (Command: External Cloud)
- **Feature ID:** `cmd_remote_env`
- **Classification:** **`EXTERNAL_SERVICE_DEPENDENT`**
- **User Entry Point:** Slash command `/remote-env`
- **Frontend Components:** commands/remote-env/index.ts
- **Lifecycle & State Flow:** User invokes `/remote-env` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/remote-env/index.ts`
- **Dependencies:** `network, external_service, authentication`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `True`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/remote-env``
- **Brain Status:** **`EXTERNAL_DEPENDENCY`**
- **Implementation Gap:** Requires remote Anthropic cloud / SaaS infrastructure not present in local offline runtime.
- **Migration Strategy:** Quarantine within external capabilities boundary.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.121 `/web-setup` (Command: Local)
- **Feature ID:** `cmd_web_setup`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/web-setup`
- **Frontend Components:** commands/remote-setup/index.ts
- **Lifecycle & State Flow:** User invokes `/web-setup` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/remote-setup/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/web-setup``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.122 `/rename` (Command: Local)
- **Feature ID:** `cmd_rename`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/rename`
- **Frontend Components:** commands/rename/index.ts
- **Lifecycle & State Flow:** User invokes `/rename` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/rename/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/rename``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.123 `/resume` (Command: Brain Adapted)
- **Feature ID:** `cmd_resume`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/resume`
- **Frontend Components:** commands/resume/index.ts
- **Lifecycle & State Flow:** User invokes `/resume` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/resume/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/resume``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Backend semantics differ between Claude standalone and Brain runtime.
- **Migration Strategy:** Preserve Claude UX and route backend operations to Rust Brain adapter.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.124 `/review` (Command: Local)
- **Feature ID:** `cmd_review`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/review`
- **Frontend Components:** commands/review.ts
- **Lifecycle & State Flow:** User invokes `/review` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/review.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/review``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.125 `/rewind` (Command: Brain Adapted)
- **Feature ID:** `cmd_rewind`
- **Classification:** **`BRAIN_BACKEND_AVAILABLE`**
- **User Entry Point:** Slash command `/rewind`
- **Frontend Components:** commands/rewind/index.ts
- **Lifecycle & State Flow:** User invokes `/rewind` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/rewind/index.ts`
- **Dependencies:** `local, filesystem`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/rewind``
- **Brain Status:** **`PARTIALLY_IMPLEMENTED`**
- **Implementation Gap:** Claude truncates message array; Brain needs to rollback SQLite event store.
- **Migration Strategy:** Delegate turn rollback to Rust storage checkpoint store.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.126 `/sandbox` (Command: Local)
- **Feature ID:** `cmd_sandbox`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/sandbox`
- **Frontend Components:** commands/sandbox-toggle/index.ts
- **Lifecycle & State Flow:** User invokes `/sandbox` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/sandbox-toggle/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/sandbox``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.127 `/security-review` (Command: Local)
- **Feature ID:** `cmd_security_review`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/security-review`
- **Frontend Components:** commands/security-review.ts
- **Lifecycle & State Flow:** User invokes `/security-review` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/security-review.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/security-review``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.128 `/session` (Command: Local)
- **Feature ID:** `cmd_session`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/session`
- **Frontend Components:** commands/session/index.ts
- **Lifecycle & State Flow:** User invokes `/session` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/session/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/session``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.129 `/skills` (Command: Local)
- **Feature ID:** `cmd_skills`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/skills`
- **Frontend Components:** commands/skills/index.ts
- **Lifecycle & State Flow:** User invokes `/skills` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/skills/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/skills``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.130 `/stats` (Command: Local)
- **Feature ID:** `cmd_stats`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/stats`
- **Frontend Components:** commands/stats/index.ts
- **Lifecycle & State Flow:** User invokes `/stats` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/stats/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/stats``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.131 `/status` (Command: Local)
- **Feature ID:** `cmd_status`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/status`
- **Frontend Components:** commands/status/index.ts
- **Lifecycle & State Flow:** User invokes `/status` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/status/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `True` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/status``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.132 `/statusline` (Command: Local)
- **Feature ID:** `cmd_statusline`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/statusline`
- **Frontend Components:** commands/statusline.tsx
- **Lifecycle & State Flow:** User invokes `/statusline` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/statusline.tsx`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/statusline``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.133 `/stickers` (Command: Internal / Telemetry)
- **Feature ID:** `cmd_stickers`
- **Classification:** **`CLAUDE_SPECIFIC`**
- **User Entry Point:** Slash command `/stickers`
- **Frontend Components:** commands/stickers/index.ts
- **Lifecycle & State Flow:** User invokes `/stickers` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/stickers/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/stickers``
- **Brain Status:** **`NEEDS_DECISION`**
- **Implementation Gap:** Anthropic internal telemetry or novelty feature.
- **Migration Strategy:** Queue for explicit user KEEP / MODIFY / REMOVE decision.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.134 `/tag` (Command: Local)
- **Feature ID:** `cmd_tag`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/tag`
- **Frontend Components:** commands/tag/index.ts
- **Lifecycle & State Flow:** User invokes `/tag` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/tag/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/tag``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.135 `/tasks` (Command: Local)
- **Feature ID:** `cmd_tasks`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/tasks`
- **Frontend Components:** commands/tasks/index.ts
- **Lifecycle & State Flow:** User invokes `/tasks` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/tasks/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/tasks``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.136 `/terminal-setup` (Command: Local)
- **Feature ID:** `cmd_terminal_setup`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/terminal-setup`
- **Frontend Components:** commands/terminalSetup/index.ts
- **Lifecycle & State Flow:** User invokes `/terminal-setup` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/terminalSetup/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/terminal-setup``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.137 `/theme` (Command: Local)
- **Feature ID:** `cmd_theme`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/theme`
- **Frontend Components:** commands/theme/index.ts
- **Lifecycle & State Flow:** User invokes `/theme` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/theme/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/theme``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.138 `/think-back` (Command: Local)
- **Feature ID:** `cmd_think_back`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/think-back`
- **Frontend Components:** commands/thinkback/index.ts
- **Lifecycle & State Flow:** User invokes `/think-back` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/thinkback/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/think-back``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.139 `/thinkback-play` (Command: Local)
- **Feature ID:** `cmd_thinkback_play`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/thinkback-play`
- **Frontend Components:** commands/thinkback-play/index.ts
- **Lifecycle & State Flow:** User invokes `/thinkback-play` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/thinkback-play/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/thinkback-play``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.140 `/ultraplan` (Command: Local)
- **Feature ID:** `cmd_ultraplan`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/ultraplan`
- **Frontend Components:** commands/ultraplan.tsx
- **Lifecycle & State Flow:** User invokes `/ultraplan` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/ultraplan.tsx`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/ultraplan``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.141 `/upgrade` (Command: Local)
- **Feature ID:** `cmd_upgrade`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/upgrade`
- **Frontend Components:** commands/upgrade/index.ts
- **Lifecycle & State Flow:** User invokes `/upgrade` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/upgrade/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/upgrade``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.142 `/usage` (Command: Local)
- **Feature ID:** `cmd_usage`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/usage`
- **Frontend Components:** commands/usage/index.ts
- **Lifecycle & State Flow:** User invokes `/usage` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/usage/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/usage``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.143 `/version` (Command: Local)
- **Feature ID:** `cmd_version`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/version`
- **Frontend Components:** commands/version.ts
- **Lifecycle & State Flow:** User invokes `/version` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/version.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/version``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.144 `/vim` (Command: Local)
- **Feature ID:** `cmd_vim`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/vim`
- **Frontend Components:** commands/vim/index.ts
- **Lifecycle & State Flow:** User invokes `/vim` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/vim/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/vim``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

### 2.145 `/voice` (Command: Local)
- **Feature ID:** `cmd_voice`
- **Classification:** **`LOCAL_IMPLEMENTABLE`**
- **User Entry Point:** Slash command `/voice`
- **Frontend Components:** commands/voice/index.ts
- **Lifecycle & State Flow:** User invokes `/voice` -> resolves command definition -> executes handler
- **Backend Implementation:** `commands/voice/index.ts`
- **Dependencies:** `local`
- **Model Inference Required:** `False` | **Auth Required:** `False` | **Network Required:** `False`
- **Persistence Requirements:** Varies by command
- **Filesystem Requirements:** Varies by command
- **Brain Equivalent:** `Brain command `/voice``
- **Brain Status:** **`IMPLEMENTED_IDENTICAL`**
- **Implementation Gap:** None (Certified or operational in TS shell)
- **Migration Strategy:** Reuse frozen command implementation.
- **Decision Status:** **`DECISION_REQUIRED`**

---

## 3. Frontend Component Inventory

The frozen Claude frontend contains **145 distinct component directories/files** in `packages/brain-shell/vendor/claude/components/` and **3 screens** in `screens/` structured into functional zones:

| Component Sub-Tree / Screen | File Count | Primary Role & Responsibilities |
| :--- | :--- | :--- |
| `screens/REPL.tsx` | 1 file (895kB) | Main conversational REPL container, message list, prompt composer, keyboard shortcut listeners, stream handling |
| `screens/ResumeConversation.tsx` | 1 file (60kB) | Interactive project session selector, turn previewer, transcript loader |
| `screens/Doctor.tsx` | 1 file (73kB) | Diagnostic dashboard for npm dist-tags, PID locks, environment limits, sandbox integrity, MCP warnings |
| `components/PromptInput/` | 21 files | Multiline text buffer, mode badge, cursor offset, history navigation, suggestions popover, 3-column help menu footer |
| `components/messages/` | 34 files | User turn boxes, Assistant markdown streams, Thinking blocks, Progress bars, Tool results, Error callouts |
| `components/permissions/` | 30 files | Bash risk rating modal, File write approval diff, Plan mode entry/exit dialogs, MCP approval, Web fetch whitelist |
| `components/design-system/`| 16 files | ThemeProvider, ThemedBox, ThemedText, Pane, Dialog, Divider, ProgressBar, KeyboardShortcutHint, FuzzyPicker |
| `components/tasks/` | 12 files | Background task list, Coordinator task panel, Task output stream, Agent status pills |
| `components/mcp/` | 13 files | MCP server multiselect, server details, parsing warnings, authentication prompt |
| `components/CustomSelect/` | 10 files | Select dropdowns, keyboard navigation, radio lists, scrolling selectors |
| `components/FeedbackSurvey/`| 9 files | Frustration detection, dogfooding feedback prompt, rating selector |
| `components/Spinner/` | 12 files | Colored shimmer animations, spinner frames (`claudeBlue_FOR_SYSTEM_SPINNER`) |
| `components/StructuredDiff/`| 2 files | Color-diff word-level and line-level side-by-side / unified diff visualizer |
| `components/LogoV2/` | 15 files | ASCII Clawd mascot, version banner, context indicators, terminal frame headers |
| `components/HelpV2/` | 3 files | General help catalog, command listing, documentation links |
| `components/agents/` | 14 files | Agent color badges, subagent execution bars, teammate views |
| `components/sandbox/` | 5 files | Sandbox doctor checks, violation expanded view, docker status |
| `components/wizard/` | 5 files | First-time onboarding wizard, telemetry prompts, theme setup |

---

## 4. Backend & Business Logic Inventory

Claude's backend business logic is decomposed across **82 valid commands**, **42 tools**, and **20 core services** in `packages/brain-shell/vendor/claude/`:

### 4.1 Core Services Architecture
1. **`services/mcp/` (23 files):** Manages Model Context Protocol servers, stdio/SSE client connections, tool discovery, and schema validation.
2. **`services/compact/` (11 files):** Analyzes conversation token utilization and triggers LLM-based turn summarization / context collapse.
3. **`services/lsp/` (7 files):** Interacts with language server instances (rust-analyzer, tsserver, pyright) for code diagnostics and definition lookups.
4. **`services/tools/` (4 files):** Tool orchestration, execution pipelines, timeouts, and streaming output buffers.
5. **`services/analytics/` (9 files):** Event logging, telemetry sinks, and GrowthBook feature flag caching.
6. **`services/plugins/` (3 files):** Plugin installation, discovery, CLI command augmentation, and marketplace manifest parsing.
7. **`services/teamMemorySync/` (5 files):** Team memory synchronization, secret scanning, and git branch memory sharing.
8. **`services/autoDream/` (4 files):** Background memory consolidation and long-term memory synthesis.
9. **`services/PromptSuggestion/` (2 files):** Speculative prompt completion and autocomplete caching.
10. **`services/SessionMemory/` (3 files):** Project memory extraction, `CLAUDE.md` synthesis, and user preference tracking.
11. **`services/voiceStreamSTT.ts` (1 file):** WebSocket streaming speech-to-text integration with Deepgram backend.

---

## 5. Claude -> Brain Feature Mapping Table

| Feature Name | Category | Classification | Reference Claude Implementation | Brain Equivalent | Brain Status | Migration Seam |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `Session Creation & Initialization` | Core Conversation | **`LOCAL_IMPLEMENTABLE`** | `bootstrap/state.ts:initSession(), utils/sessionStorage.ts:initSession()` | `crates/brain-session::SessionManagerImpl + UDS session initialization` | **`IMPLEMENTED_IDENTICAL`** | `screens/REPL.tsx` |
| `Resume Past Session (/resume)` | Core Conversation | **`BRAIN_BACKEND_AVAILABLE`** | `commands/resume/index.ts, utils/sessionStorage.ts:loadAllProjectsMessageLogsProgressive()` | `crates/brain-storage SQLite event log and session repository` | **`PARTIALLY_IMPLEMENTED`** | `screens/ResumeConversation.tsx` |
| `Token Streaming & Typewriter Drain` | Core Conversation | **`BRAIN_BACKEND_AVAILABLE`** | `query.ts:query(), query/deps.ts:callModel()` | `crates/brain-services UDS monotonic StreamEvent chunks via brainCallModel.ts` | **`IMPLEMENTED_IDENTICAL`** | `components/MessageResponse.tsx` |
| `Reasoning & Thinking Blocks (ThinkingConfig)` | Core Conversation | **`BRAIN_BACKEND_AVAILABLE`** | `utils/thinking.ts, query.ts` | `packages/brain-shell/src/adapter/brainCallModel.ts reasoning blocks` | **`IMPLEMENTED_IDENTICAL`** | `components/ThinkingToggle.tsx` |
| `Structured Diff Rendering` | Core Conversation | **`LOCAL_IMPLEMENTABLE`** | `native-ts/color-diff/index.ts, components/StructuredDiff.tsx` | `packages/brain-shell/vendor/claude/components/StructuredDiff.tsx` | **`IMPLEMENTED_IDENTICAL`** | `components/StructuredDiff.tsx` |
| `Response Interruption & Cancellation (Ctrl+C / Escape)` | Core Conversation | **`LOCAL_IMPLEMENTABLE`** | `hooks/useCancelRequest.ts, utils/messages.ts:createInterruptedMessage()` | `packages/brain-shell/src/adapter/brainCallModel.ts AbortSignal handling` | **`IMPLEMENTED_IDENTICAL`** | `components/InterruptedByUser.tsx` |
| `Multiline Input (`\` + `Enter`)` | Composer & Input | **`LOCAL_IMPLEMENTABLE`** | `components/PromptInput/PromptInput.tsx` | `packages/brain-shell/vendor/claude/components/PromptInput/` | **`IMPLEMENTED_IDENTICAL`** | `components/PromptInput/PromptInput.tsx` |
| `@ File Path Autocompletion` | Composer & Input | **`LOCAL_IMPLEMENTABLE`** | `utils/suggestions/fileSuggestions.ts, native-ts/file-index/` | `packages/brain-shell/vendor/claude/utils/suggestions/fileSuggestions.ts` | **`IMPLEMENTED_IDENTICAL`** | `components/PromptInput/ContextSuggestions.tsx` |
| `/ Slash Command Autocompletion` | Composer & Input | **`LOCAL_IMPLEMENTABLE`** | `commands/index.ts, utils/suggestions/commandSuggestions.ts` | `packages/brain-shell/vendor/claude/commands/` | **`IMPLEMENTED_IDENTICAL`** | `components/PromptInput/ContextSuggestions.tsx` |
| `Keyboard Shortcut Help Menu (?)` | Composer & Input | **`LOCAL_IMPLEMENTABLE`** | `components/PromptInput/PromptInput.tsx:onChange()` | `packages/brain-shell/vendor/claude/components/PromptInput/PromptInputHelpMenu.tsx` | **`IMPLEMENTED_IDENTICAL`** | `components/PromptInput/PromptInputHelpMenu.tsx` |
| `Shell / Bash Execution Mode (!)` | Composer & Input | **`LOCAL_IMPLEMENTABLE`** | `tools/BashTool/index.ts, components/PromptInput/inputModes.ts` | `packages/brain-shell/vendor/claude/tools/BashTool/` | **`IMPLEMENTED_IDENTICAL`** | `components/PromptInput/PromptInput.tsx` |
| `Modal Vim Editing Mode (/vim)` | Composer & Input | **`LOCAL_IMPLEMENTABLE`** | `components/VimTextInput.tsx, commands/vim/` | `packages/brain-shell/vendor/claude/components/VimTextInput.tsx` | **`IMPLEMENTED_IDENTICAL`** | `components/VimTextInput.tsx` |
| `Push-to-Talk Voice Streaming (/voice)` | Composer & Input | **`EXTERNAL_SERVICE_DEPENDENT`** | `services/voiceStreamSTT.ts, services/voiceKeyterms.ts, commands/voice/voice.ts` | `Local Whisper / STT engine adapter or external toggle` | **`EXTERNAL_DEPENDENCY`** | `components/PromptInput/VoiceIndicator.tsx` |
| `Permission Mode Cycling (Shift+Tab)` | Modes & Policies | **`LOCAL_IMPLEMENTABLE`** | `utils/permissions/permissionSetup.ts, hooks/toolPermission/` | `packages/brain-shell/vendor/claude/hooks/toolPermission/` | **`IMPLEMENTED_IDENTICAL`** | `components/PromptInput/PromptInputModeIndicator.tsx` |
| `Theme Selection & Live Diff Preview (/theme)` | Modes & Policies | **`LOCAL_IMPLEMENTABLE`** | `commands/theme/index.ts, utils/theme.ts, utils/config.ts:saveGlobalConfig()` | `packages/brain-shell/vendor/claude/components/ThemePicker.tsx` | **`IMPLEMENTED_IDENTICAL`** | `components/ThemePicker.tsx` |
| `Model Selection Dialog (/model, Alt+P)` | Modes & Policies | **`BRAIN_BACKEND_AVAILABLE`** | `commands/model/index.ts, utils/model.ts` | `packages/brain-shell/src/adapter/brainCallModel.ts model routing` | **`PARTIALLY_IMPLEMENTED`** | `components/ModelPicker.tsx` |
| `Architect / Plan Mode (/plan)` | Modes & Policies | **`LOCAL_IMPLEMENTABLE`** | `tools/EnterPlanModeTool/, tools/ExitPlanModeTool/, utils/plans.ts` | `packages/brain-shell/vendor/claude/tools/EnterPlanModeTool/` | **`IMPLEMENTED_IDENTICAL`** | `components/permissions/EnterPlanModePermissionRequest/` |
| `Session Memory Periodic Extraction` | Memory & Services | **`ANTHROPIC_MODEL_DEPENDENT`** | `services/SessionMemory/sessionMemory.ts, services/SessionMemory/prompts.ts` | `crates/brain-storage (SQLite STM/LTM) + crates/brain-domain` | **`PARTIALLY_IMPLEMENTED`** | `services/SessionMemory/sessionMemory.ts` |
| `Auto Dream Background Memory Consolidation` | Memory & Services | **`ANTHROPIC_MODEL_DEPENDENT`** | `services/autoDream/autoDream.ts, services/autoDream/consolidationPrompt.ts` | `crates/brain-domain::Edge::strengthen / KnowledgeGraph::consolidate` | **`BRAIN_BACKEND_AVAILABLE`** | `services/autoDream/autoDream.ts` |
| `Context Compaction & Token Summarization` | Memory & Services | **`BRAIN_BACKEND_AVAILABLE`** | `services/compact/compact.ts, services/compact/prompt.ts, services/compact/postCompactCleanup.ts` | `crates/brain-session::SessionContext::compact` | **`PARTIALLY_IMPLEMENTED`** | `services/compact/` |
| `Language Server Protocol (LSP) Manager` | Memory & Services | **`LOCAL_IMPLEMENTABLE`** | `services/lsp/LSPServerManager.ts, services/lsp/LSPClient.ts, tools/LSPTool/` | `packages/brain-shell/vendor/claude/services/lsp/` | **`IMPLEMENTED_IDENTICAL`** | `services/lsp/LSPServerManager.ts` |
| `Tool: ship-audit` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/AgentTool/prompt.ts` | `Brain tool `ship-audit`` | **`IMPLEMENTED_IDENTICAL`** | `tools/AgentTool/` |
| `Tool: AskUserQuestion` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/AskUserQuestionTool/prompt.ts` | `Brain tool `AskUserQuestion`` | **`IMPLEMENTED_IDENTICAL`** | `tools/AskUserQuestionTool/` |
| `Tool: Bash` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/BashTool/prompt.ts` | `Brain tool `Bash`` | **`IMPLEMENTED_IDENTICAL`** | `tools/BashTool/` |
| `Tool: Brief` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/BriefTool/prompt.ts` | `Brain tool `Brief`` | **`IMPLEMENTED_IDENTICAL`** | `tools/BriefTool/` |
| `Tool: Config` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/ConfigTool/prompt.ts` | `Brain tool `Config`` | **`IMPLEMENTED_IDENTICAL`** | `tools/ConfigTool/` |
| `Tool: EnterPlanMode` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/EnterPlanModeTool/prompt.ts` | `Brain tool `EnterPlanMode`` | **`IMPLEMENTED_IDENTICAL`** | `tools/EnterPlanModeTool/` |
| `Tool: EnterWorktree` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/EnterWorktreeTool/prompt.ts` | `Brain tool `EnterWorktree`` | **`IMPLEMENTED_IDENTICAL`** | `tools/EnterWorktreeTool/` |
| `Tool: ExitPlanMode` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/ExitPlanModeTool/prompt.ts` | `Brain tool `ExitPlanMode`` | **`IMPLEMENTED_IDENTICAL`** | `tools/ExitPlanModeTool/` |
| `Tool: ExitWorktree` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/ExitWorktreeTool/prompt.ts` | `Brain tool `ExitWorktree`` | **`IMPLEMENTED_IDENTICAL`** | `tools/ExitWorktreeTool/` |
| `Tool: FileEdit` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/FileEditTool/prompt.ts` | `Brain tool `FileEdit`` | **`IMPLEMENTED_IDENTICAL`** | `tools/FileEditTool/` |
| `Tool: FileRead` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/FileReadTool/prompt.ts` | `Brain tool `FileRead`` | **`IMPLEMENTED_IDENTICAL`** | `tools/FileReadTool/` |
| `Tool: FileWrite` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/FileWriteTool/prompt.ts` | `Brain tool `FileWrite`` | **`IMPLEMENTED_IDENTICAL`** | `tools/FileWriteTool/` |
| `Tool: Glob` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/GlobTool/prompt.ts` | `Brain tool `Glob`` | **`IMPLEMENTED_IDENTICAL`** | `tools/GlobTool/` |
| `Tool: Grep` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/GrepTool/prompt.ts` | `Brain tool `Grep`` | **`IMPLEMENTED_IDENTICAL`** | `tools/GrepTool/` |
| `Tool: LSP` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/LSPTool/prompt.ts` | `Brain tool `LSP`` | **`IMPLEMENTED_IDENTICAL`** | `tools/LSPTool/` |
| `Tool: ListMcpResources` | Tool UX & Execution | **`BRAIN_BACKEND_AVAILABLE`** | `tools/ListMcpResourcesTool/prompt.ts` | `Brain tool `ListMcpResources`` | **`PARTIALLY_IMPLEMENTED`** | `tools/ListMcpResourcesTool/` |
| `Tool: MCP` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/MCPTool/prompt.ts` | `Brain tool `MCP`` | **`IMPLEMENTED_IDENTICAL`** | `tools/MCPTool/` |
| `Tool: McpAuth` | Tool UX & Execution | **`EXTERNAL_SERVICE_DEPENDENT`** | `tools/McpAuthTool/McpAuthTool.ts` | `Brain tool `McpAuth`` | **`EXTERNAL_DEPENDENCY`** | `tools/McpAuthTool/` |
| `Tool: NotebookEdit` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/NotebookEditTool/prompt.ts` | `Brain tool `NotebookEdit`` | **`IMPLEMENTED_IDENTICAL`** | `tools/NotebookEditTool/` |
| `Tool: PowerShell` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/PowerShellTool/prompt.ts` | `Brain tool `PowerShell`` | **`IMPLEMENTED_IDENTICAL`** | `tools/PowerShellTool/` |
| `Tool: REPL` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/REPLTool` | `Brain tool `REPL`` | **`IMPLEMENTED_IDENTICAL`** | `tools/REPLTool/` |
| `Tool: ReadMcpResource` | Tool UX & Execution | **`BRAIN_BACKEND_AVAILABLE`** | `tools/ReadMcpResourceTool/prompt.ts` | `Brain tool `ReadMcpResource`` | **`PARTIALLY_IMPLEMENTED`** | `tools/ReadMcpResourceTool/` |
| `Tool: RemoteTrigger` | Tool UX & Execution | **`EXTERNAL_SERVICE_DEPENDENT`** | `tools/RemoteTriggerTool/prompt.ts` | `Brain tool `RemoteTrigger`` | **`EXTERNAL_DEPENDENCY`** | `tools/RemoteTriggerTool/` |
| `Tool: ScheduleCron` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/ScheduleCronTool/prompt.ts` | `Brain tool `ScheduleCron`` | **`IMPLEMENTED_IDENTICAL`** | `tools/ScheduleCronTool/` |
| `Tool: SendMessage` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/SendMessageTool/prompt.ts` | `Brain tool `SendMessage`` | **`IMPLEMENTED_IDENTICAL`** | `tools/SendMessageTool/` |
| `Tool: Skill` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/SkillTool/prompt.ts` | `Brain tool `Skill`` | **`IMPLEMENTED_IDENTICAL`** | `tools/SkillTool/` |
| `Tool: Sleep` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/SleepTool/prompt.ts` | `Brain tool `Sleep`` | **`IMPLEMENTED_IDENTICAL`** | `tools/SleepTool/` |
| `Tool: SyntheticOutput` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/SyntheticOutputTool/SyntheticOutputTool.ts` | `Brain tool `SyntheticOutput`` | **`IMPLEMENTED_IDENTICAL`** | `tools/SyntheticOutputTool/` |
| `Tool: TaskCreate` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TaskCreateTool/prompt.ts` | `Brain tool `TaskCreate`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TaskCreateTool/` |
| `Tool: TaskGet` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TaskGetTool/prompt.ts` | `Brain tool `TaskGet`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TaskGetTool/` |
| `Tool: TaskList` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TaskListTool/prompt.ts` | `Brain tool `TaskList`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TaskListTool/` |
| `Tool: TaskOutput` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TaskOutputTool` | `Brain tool `TaskOutput`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TaskOutputTool/` |
| `Tool: TaskStop` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TaskStopTool/prompt.ts` | `Brain tool `TaskStop`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TaskStopTool/` |
| `Tool: TaskUpdate` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TaskUpdateTool/prompt.ts` | `Brain tool `TaskUpdate`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TaskUpdateTool/` |
| `Tool: TeamCreate` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TeamCreateTool/prompt.ts` | `Brain tool `TeamCreate`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TeamCreateTool/` |
| `Tool: TeamDelete` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TeamDeleteTool/prompt.ts` | `Brain tool `TeamDelete`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TeamDeleteTool/` |
| `Tool: TodoWrite` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TodoWriteTool/prompt.ts` | `Brain tool `TodoWrite`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TodoWriteTool/` |
| `Tool: Search` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/ToolSearchTool/prompt.ts` | `Brain tool `Search`` | **`IMPLEMENTED_IDENTICAL`** | `tools/ToolSearchTool/` |
| `Tool: TungstenTool` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/TungstenTool/TungstenTool.ts` | `Brain tool `TungstenTool`` | **`IMPLEMENTED_IDENTICAL`** | `tools/TungstenTool/` |
| `Tool: WebFetch` | Tool UX & Execution | **`EXTERNAL_SERVICE_DEPENDENT`** | `tools/WebFetchTool/prompt.ts` | `Brain tool `WebFetch`` | **`IMPLEMENTED_IDENTICAL`** | `tools/WebFetchTool/` |
| `Tool: WebSearch` | Tool UX & Execution | **`EXTERNAL_SERVICE_DEPENDENT`** | `tools/WebSearchTool/prompt.ts` | `Brain tool `WebSearch`` | **`IMPLEMENTED_IDENTICAL`** | `tools/WebSearchTool/` |
| `Tool: Workflow` | Tool UX & Execution | **`LOCAL_IMPLEMENTABLE`** | `tools/WorkflowTool` | `Brain tool `Workflow`` | **`IMPLEMENTED_IDENTICAL`** | `tools/WorkflowTool/` |
| `/add-dir` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/add-dir/index.ts` | `Brain command `/add-dir`` | **`IMPLEMENTED_IDENTICAL`** | `commands/add-dir/index.ts` |
| `/advisor` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/advisor.ts` | `Brain command `/advisor`` | **`IMPLEMENTED_IDENTICAL`** | `commands/advisor.ts` |
| `/agents` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/agents/index.ts` | `Brain command `/agents`` | **`IMPLEMENTED_IDENTICAL`** | `commands/agents/index.ts` |
| `/branch` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/branch/index.ts` | `Brain command `/branch`` | **`IMPLEMENTED_IDENTICAL`** | `commands/branch/index.ts` |
| `/remote-control` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/bridge/index.ts` | `Brain command `/remote-control`` | **`IMPLEMENTED_IDENTICAL`** | `commands/bridge/index.ts` |
| `/bridge-kick` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/bridge-kick.ts` | `Brain command `/bridge-kick`` | **`IMPLEMENTED_IDENTICAL`** | `commands/bridge-kick.ts` |
| `/brief` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/brief.ts` | `Brain command `/brief`` | **`IMPLEMENTED_IDENTICAL`** | `commands/brief.ts` |
| `/btw` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/btw/index.ts` | `Brain command `/btw`` | **`IMPLEMENTED_IDENTICAL`** | `commands/btw/index.ts` |
| `/chrome` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/chrome/index.ts` | `Brain command `/chrome`` | **`EXTERNAL_DEPENDENCY`** | `commands/chrome/index.ts` |
| `/clear` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/clear/index.ts` | `Brain command `/clear`` | **`IMPLEMENTED_IDENTICAL`** | `commands/clear/index.ts` |
| `/color` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/color/index.ts` | `Brain command `/color`` | **`IMPLEMENTED_IDENTICAL`** | `commands/color/index.ts` |
| `/commit-push-pr` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/commit-push-pr.ts` | `Brain command `/commit-push-pr`` | **`PARTIALLY_IMPLEMENTED`** | `commands/commit-push-pr.ts` |
| `/commit` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/commit.ts` | `Brain command `/commit`` | **`IMPLEMENTED_IDENTICAL`** | `commands/commit.ts` |
| `/compact` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/compact/index.ts` | `Brain command `/compact`` | **`PARTIALLY_IMPLEMENTED`** | `commands/compact/index.ts` |
| `/config` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/config/index.ts` | `Brain command `/config`` | **`IMPLEMENTED_IDENTICAL`** | `commands/config/index.ts` |
| `/context` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/context/index.ts` | `Brain command `/context`` | **`PARTIALLY_IMPLEMENTED`** | `commands/context/index.ts` |
| `/copy` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/copy/index.ts` | `Brain command `/copy`` | **`IMPLEMENTED_IDENTICAL`** | `commands/copy/index.ts` |
| `/cost` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/cost/index.ts` | `Brain command `/cost`` | **`IMPLEMENTED_IDENTICAL`** | `commands/cost/index.ts` |
| `/createMovedToPluginCommand` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/createMovedToPluginCommand.ts` | `Brain command `/createMovedToPluginCommand`` | **`IMPLEMENTED_IDENTICAL`** | `commands/createMovedToPluginCommand.ts` |
| `/desktop` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/desktop/index.ts` | `Brain command `/desktop`` | **`IMPLEMENTED_IDENTICAL`** | `commands/desktop/index.ts` |
| `/diff` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/diff/index.ts` | `Brain command `/diff`` | **`IMPLEMENTED_IDENTICAL`** | `commands/diff/index.ts` |
| `/doctor` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/doctor/index.ts` | `Brain command `/doctor`` | **`PARTIALLY_IMPLEMENTED`** | `commands/doctor/index.ts` |
| `/effort` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/effort/index.ts` | `Brain command `/effort`` | **`IMPLEMENTED_IDENTICAL`** | `commands/effort/index.ts` |
| `/exit` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/exit/index.ts` | `Brain command `/exit`` | **`IMPLEMENTED_IDENTICAL`** | `commands/exit/index.ts` |
| `/export` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/export/index.ts` | `Brain command `/export`` | **`IMPLEMENTED_IDENTICAL`** | `commands/export/index.ts` |
| `/extra-usage` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/extra-usage/index.ts` | `Brain command `/extra-usage`` | **`EXTERNAL_DEPENDENCY`** | `commands/extra-usage/index.ts` |
| `/fast` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/fast/index.ts` | `Brain command `/fast`` | **`IMPLEMENTED_IDENTICAL`** | `commands/fast/index.ts` |
| `/feedback` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/feedback/index.ts` | `Brain command `/feedback`` | **`EXTERNAL_DEPENDENCY`** | `commands/feedback/index.ts` |
| `/files` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/files/index.ts` | `Brain command `/files`` | **`IMPLEMENTED_IDENTICAL`** | `commands/files/index.ts` |
| `/heapdump` | Command: Internal / Telemetry | **`CLAUDE_SPECIFIC`** | `commands/heapdump/index.ts` | `Brain command `/heapdump`` | **`NEEDS_DECISION`** | `commands/heapdump/index.ts` |
| `/help` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/help/index.ts` | `Brain command `/help`` | **`IMPLEMENTED_IDENTICAL`** | `commands/help/index.ts` |
| `/hooks` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/hooks/index.ts` | `Brain command `/hooks`` | **`IMPLEMENTED_IDENTICAL`** | `commands/hooks/index.ts` |
| `/ide` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/ide/index.ts` | `Brain command `/ide`` | **`IMPLEMENTED_IDENTICAL`** | `commands/ide/index.ts` |
| `/init-verifiers` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/init-verifiers.ts` | `Brain command `/init-verifiers`` | **`IMPLEMENTED_IDENTICAL`** | `commands/init-verifiers.ts` |
| `/init` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/init.ts` | `Brain command `/init`` | **`PARTIALLY_IMPLEMENTED`** | `commands/init.ts` |
| `/project_areas` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/insights.ts` | `Brain command `/project_areas`` | **`IMPLEMENTED_IDENTICAL`** | `commands/insights.ts` |
| `/install-github-app` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/install-github-app/index.ts` | `Brain command `/install-github-app`` | **`EXTERNAL_DEPENDENCY`** | `commands/install-github-app/index.ts` |
| `/install-slack-app` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/install-slack-app/index.ts` | `Brain command `/install-slack-app`` | **`EXTERNAL_DEPENDENCY`** | `commands/install-slack-app/index.ts` |
| `/install` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/install.tsx` | `Brain command `/install`` | **`IMPLEMENTED_IDENTICAL`** | `commands/install.tsx` |
| `/keybindings` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/keybindings/index.ts` | `Brain command `/keybindings`` | **`IMPLEMENTED_IDENTICAL`** | `commands/keybindings/index.ts` |
| `/login` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/login/index.ts` | `Brain command `/login`` | **`EXTERNAL_DEPENDENCY`** | `commands/login/index.ts` |
| `/logout` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/logout/index.ts` | `Brain command `/logout`` | **`EXTERNAL_DEPENDENCY`** | `commands/logout/index.ts` |
| `/mcp` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/mcp/index.ts` | `Brain command `/mcp`` | **`IMPLEMENTED_IDENTICAL`** | `commands/mcp/index.ts` |
| `/memory` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/memory/index.ts` | `Brain command `/memory`` | **`PARTIALLY_IMPLEMENTED`** | `commands/memory/index.ts` |
| `/mobile` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/mobile/index.ts` | `Brain command `/mobile`` | **`EXTERNAL_DEPENDENCY`** | `commands/mobile/index.ts` |
| `/model` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/model/index.ts` | `Brain command `/model`` | **`PARTIALLY_IMPLEMENTED`** | `commands/model/index.ts` |
| `/output-style` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/output-style/index.ts` | `Brain command `/output-style`` | **`IMPLEMENTED_IDENTICAL`** | `commands/output-style/index.ts` |
| `/passes` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/passes/index.ts` | `Brain command `/passes`` | **`IMPLEMENTED_IDENTICAL`** | `commands/passes/index.ts` |
| `/permissions` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/permissions/index.ts` | `Brain command `/permissions`` | **`IMPLEMENTED_IDENTICAL`** | `commands/permissions/index.ts` |
| `/plan` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/plan/index.ts` | `Brain command `/plan`` | **`IMPLEMENTED_IDENTICAL`** | `commands/plan/index.ts` |
| `/plugin` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/plugin/index.tsx` | `Brain command `/plugin`` | **`PARTIALLY_IMPLEMENTED`** | `commands/plugin/index.tsx` |
| `/pr-comments` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/pr_comments/index.ts` | `Brain command `/pr-comments`` | **`IMPLEMENTED_IDENTICAL`** | `commands/pr_comments/index.ts` |
| `/privacy-settings` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/privacy-settings/index.ts` | `Brain command `/privacy-settings`` | **`IMPLEMENTED_IDENTICAL`** | `commands/privacy-settings/index.ts` |
| `/rate-limit-options` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/rate-limit-options/index.ts` | `Brain command `/rate-limit-options`` | **`EXTERNAL_DEPENDENCY`** | `commands/rate-limit-options/index.ts` |
| `/release-notes` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/release-notes/index.ts` | `Brain command `/release-notes`` | **`IMPLEMENTED_IDENTICAL`** | `commands/release-notes/index.ts` |
| `/reload-plugins` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/reload-plugins/index.ts` | `Brain command `/reload-plugins`` | **`PARTIALLY_IMPLEMENTED`** | `commands/reload-plugins/index.ts` |
| `/remote-env` | Command: External Cloud | **`EXTERNAL_SERVICE_DEPENDENT`** | `commands/remote-env/index.ts` | `Brain command `/remote-env`` | **`EXTERNAL_DEPENDENCY`** | `commands/remote-env/index.ts` |
| `/web-setup` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/remote-setup/index.ts` | `Brain command `/web-setup`` | **`IMPLEMENTED_IDENTICAL`** | `commands/remote-setup/index.ts` |
| `/rename` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/rename/index.ts` | `Brain command `/rename`` | **`IMPLEMENTED_IDENTICAL`** | `commands/rename/index.ts` |
| `/resume` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/resume/index.ts` | `Brain command `/resume`` | **`PARTIALLY_IMPLEMENTED`** | `commands/resume/index.ts` |
| `/review` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/review.ts` | `Brain command `/review`` | **`IMPLEMENTED_IDENTICAL`** | `commands/review.ts` |
| `/rewind` | Command: Brain Adapted | **`BRAIN_BACKEND_AVAILABLE`** | `commands/rewind/index.ts` | `Brain command `/rewind`` | **`PARTIALLY_IMPLEMENTED`** | `commands/rewind/index.ts` |
| `/sandbox` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/sandbox-toggle/index.ts` | `Brain command `/sandbox`` | **`IMPLEMENTED_IDENTICAL`** | `commands/sandbox-toggle/index.ts` |
| `/security-review` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/security-review.ts` | `Brain command `/security-review`` | **`IMPLEMENTED_IDENTICAL`** | `commands/security-review.ts` |
| `/session` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/session/index.ts` | `Brain command `/session`` | **`IMPLEMENTED_IDENTICAL`** | `commands/session/index.ts` |
| `/skills` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/skills/index.ts` | `Brain command `/skills`` | **`IMPLEMENTED_IDENTICAL`** | `commands/skills/index.ts` |
| `/stats` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/stats/index.ts` | `Brain command `/stats`` | **`IMPLEMENTED_IDENTICAL`** | `commands/stats/index.ts` |
| `/status` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/status/index.ts` | `Brain command `/status`` | **`IMPLEMENTED_IDENTICAL`** | `commands/status/index.ts` |
| `/statusline` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/statusline.tsx` | `Brain command `/statusline`` | **`IMPLEMENTED_IDENTICAL`** | `commands/statusline.tsx` |
| `/stickers` | Command: Internal / Telemetry | **`CLAUDE_SPECIFIC`** | `commands/stickers/index.ts` | `Brain command `/stickers`` | **`NEEDS_DECISION`** | `commands/stickers/index.ts` |
| `/tag` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/tag/index.ts` | `Brain command `/tag`` | **`IMPLEMENTED_IDENTICAL`** | `commands/tag/index.ts` |
| `/tasks` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/tasks/index.ts` | `Brain command `/tasks`` | **`IMPLEMENTED_IDENTICAL`** | `commands/tasks/index.ts` |
| `/terminal-setup` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/terminalSetup/index.ts` | `Brain command `/terminal-setup`` | **`IMPLEMENTED_IDENTICAL`** | `commands/terminalSetup/index.ts` |
| `/theme` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/theme/index.ts` | `Brain command `/theme`` | **`IMPLEMENTED_IDENTICAL`** | `commands/theme/index.ts` |
| `/think-back` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/thinkback/index.ts` | `Brain command `/think-back`` | **`IMPLEMENTED_IDENTICAL`** | `commands/thinkback/index.ts` |
| `/thinkback-play` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/thinkback-play/index.ts` | `Brain command `/thinkback-play`` | **`IMPLEMENTED_IDENTICAL`** | `commands/thinkback-play/index.ts` |
| `/ultraplan` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/ultraplan.tsx` | `Brain command `/ultraplan`` | **`IMPLEMENTED_IDENTICAL`** | `commands/ultraplan.tsx` |
| `/upgrade` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/upgrade/index.ts` | `Brain command `/upgrade`` | **`IMPLEMENTED_IDENTICAL`** | `commands/upgrade/index.ts` |
| `/usage` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/usage/index.ts` | `Brain command `/usage`` | **`IMPLEMENTED_IDENTICAL`** | `commands/usage/index.ts` |
| `/version` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/version.ts` | `Brain command `/version`` | **`IMPLEMENTED_IDENTICAL`** | `commands/version.ts` |
| `/vim` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/vim/index.ts` | `Brain command `/vim`` | **`IMPLEMENTED_IDENTICAL`** | `commands/vim/index.ts` |
| `/voice` | Command: Local | **`LOCAL_IMPLEMENTABLE`** | `commands/voice/index.ts` | `Brain command `/voice`` | **`IMPLEMENTED_IDENTICAL`** | `commands/voice/index.ts` |

---

## 6. Dependency Matrix

A strict breakdown of operational dependencies across all capabilities:

| Dependency Class | Count | Description | Examples |
| :--- | :--- | :--- | :--- |
| **`local` (Pure Local TUI / State)** | 95 | Operates 100% locally in memory without disk or model dependencies | `multiline`, `theme`, `vim`, `help`, `keybindings`, `copy`, `color` |
| **`filesystem` (Local Disk / Project)** | 28 | Requires local read/write access to project files, config, or git | `@ file completion`, `FileEditTool`, `/diff`, `/commit`, `/branch`, `/export` |
| **`model` (Model Inference Required)** | 12 | Requires LLM completion or reasoning generation | Assistant turn generation, `/summary`, `/brief`, `/advisor`, `/plan`, `AgentTool` |
| **`network` (Network Access Required)** | 15 | Requires HTTP/HTTPS or socket network connectivity | `WebFetchTool`, `WebSearchTool`, `/login`, `/teleport`, `/voice` |
| **`external_service` (Remote Cloud / SaaS)** | 15 | Depends on Anthropic cloud infrastructure, GitHub API, or remote auth | `/login`, `/logout`, `/extra-usage`, `/autofix-pr`, `/mobile`, `/voice` |
| **`platform` (OS Subprocess / Shell)** | 5 | Spawns OS subprocesses, PTYs, or pipes | `BashTool`, `! shell mode`, `/install`, `/terminalSetup`, `/voice` |

---

## 7. Brain Implementation Gap Matrix

The actual technical gaps between Reference Claude and Brain Runtime:

| Subsystem Area | Reference Claude | Brain Current State | Concrete Technical Gap | Migration Action |
| :--- | :--- | :--- | :--- | :--- |
| **Memory & LTM** | `services/SessionMemory`, `CLAUDE.md` text append | `crates/brain-storage` (SQLite + WAL) + `crates/brain-domain` | Claude writes text files; Brain manages structured entities and relations in SQLite. | Adapt `/memory` command to query Brain SQLite entities over UDS. |
| **Context & Retrieval** | In-memory turn slicing + simple string match | `crates/brain-services::retrieval` (BM25 + Vector + Graph RRF) | Claude lacks hybrid graph fusion; Brain owns authoritative context construction. | Implement Phase 8.2 mathematical RRF ($k=60.0$) in Rust. |
| **Session Compaction** | `services/compact/` prompt summarizer | `crates/brain-session::SessionContext::compact` | Claude summarizes in-memory turns; Brain persists compact snapshots in WAL log. | Delegate `/compact` command to Rust `SessionContext::compact`. |
| **Turn Rollback** | Array truncation in `AppState.messages` | `crates/brain-storage` CheckpointStore | Claude drops in-memory state; Brain can restore exact historical storage checkpoints. | Adapt `/rewind` command to invoke Rust checkpoint restore. |
| **MCP Protocol** | `services/mcp/` TS stdio/SSE client | `crates/brain-mcp-adapter` (Rust) + TS client | TS client runs frontend MCP; Rust adapter manages background MCP servers independently. | Synchronize TS `/mcp` server config to Rust `brain-mcp-adapter` via UDS. |
| **Diagnostic Health** | `screens/Doctor.tsx` Anthropic network probes | `crates/brain-observability` + UDS engine ping | Claude tests Anthropic API connectivity; Brain must test Rust engine health. | Adapt `Doctor.tsx` to ping Rust UDS socket and SQLite storage integrity. |
| **Model Selection** | `components/ModelPicker.tsx` Anthropic models | `adapter/brainCallModel.ts` gateway routing | Claude lists Opus/Sonnet/Haiku; Brain supports local Ollama/vLLM/Gemini models. | Extend `ModelPicker.tsx` items to query active Brain gateway endpoints. |
| **Voice Streaming** | `services/voiceStreamSTT.ts` Deepgram WebSocket | None | Claude streams PCM audio to Anthropic STT WebSocket; Brain has no STT backend yet. | Quarantine external voice stream; adapt hook to local Whisper engine if desired. |

---

## 8. Feature Decision Queue

> [!IMPORTANT]
> **All 145 capabilities are marked `DECISION_REQUIRED`.**  
> No features are removed, simplified, or hidden. The user will explicitly decide `KEEP`, `MODIFY`, or `REMOVE` for each capability.

| Feature ID | Feature Name | Category | Classification | Current Disposition | User Decision |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `conv_init` | `Session Creation & Initialization` | Core Conversation | `LOCAL_IMPLEMENTABLE` | `Maintain current TS shell session bootstrap and synchro...` | **`DECISION_REQUIRED`** |
| `conv_resume` | `Resume Past Session (/resume)` | Core Conversation | `BRAIN_BACKEND_AVAILABLE` | `Adapt `utils/sessionStorage.ts` to query Brain SQLite s...` | **`DECISION_REQUIRED`** |
| `conv_streaming` | `Token Streaming & Typewriter Drain` | Core Conversation | `BRAIN_BACKEND_AVAILABLE` | `Preserve generic `QueryDeps.callModel` seam streaming m...` | **`DECISION_REQUIRED`** |
| `conv_thinking` | `Reasoning & Thinking Blocks (ThinkingConfig)` | Core Conversation | `BRAIN_BACKEND_AVAILABLE` | `Preserve native reasoning chunk streaming without fabri...` | **`DECISION_REQUIRED`** |
| `conv_diffs` | `Structured Diff Rendering` | Core Conversation | `LOCAL_IMPLEMENTABLE` | `Reuse frozen StructuredDiff component hierarchy....` | **`DECISION_REQUIRED`** |
| `conv_cancel` | `Response Interruption & Cancellation (Ctrl+C / Escape)` | Core Conversation | `LOCAL_IMPLEMENTABLE` | `Preserve AbortController wire propagation to Brain UDS ...` | **`DECISION_REQUIRED`** |
| `comp_multiline` | `Multiline Input (`\` + `Enter`)` | Composer & Input | `LOCAL_IMPLEMENTABLE` | `Reuse frozen PromptInput multiline state machine....` | **`DECISION_REQUIRED`** |
| `comp_file_completion` | `@ File Path Autocompletion` | Composer & Input | `LOCAL_IMPLEMENTABLE` | `Reuse frozen file suggestion system....` | **`DECISION_REQUIRED`** |
| `comp_slash_completion` | `/ Slash Command Autocompletion` | Composer & Input | `LOCAL_IMPLEMENTABLE` | `Preserve command registry autocomplete while adapting c...` | **`DECISION_REQUIRED`** |
| `comp_help_menu` | `Keyboard Shortcut Help Menu (?)` | Composer & Input | `LOCAL_IMPLEMENTABLE` | `Reuse frozen PromptInputHelpMenu component....` | **`DECISION_REQUIRED`** |
| `comp_shell_mode` | `Shell / Bash Execution Mode (!)` | Composer & Input | `LOCAL_IMPLEMENTABLE` | `Reuse frozen BashTool local execution pipeline....` | **`DECISION_REQUIRED`** |
| `comp_vim_mode` | `Modal Vim Editing Mode (/vim)` | Composer & Input | `LOCAL_IMPLEMENTABLE` | `Reuse frozen VimTextInput component....` | **`DECISION_REQUIRED`** |
| `comp_voice` | `Push-to-Talk Voice Streaming (/voice)` | Composer & Input | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine external voice_stream service; adapt hook to...` | **`DECISION_REQUIRED`** |
| `mode_permission_cycle` | `Permission Mode Cycling (Shift+Tab)` | Modes & Policies | `LOCAL_IMPLEMENTABLE` | `Reuse frozen permission cycle state machine....` | **`DECISION_REQUIRED`** |
| `mode_theme_picker` | `Theme Selection & Live Diff Preview (/theme)` | Modes & Policies | `LOCAL_IMPLEMENTABLE` | `Reuse frozen ThemePicker component....` | **`DECISION_REQUIRED`** |
| `mode_model_picker` | `Model Selection Dialog (/model, Alt+P)` | Modes & Policies | `BRAIN_BACKEND_AVAILABLE` | `Adapt ModelPicker list items in TS shell adapter to que...` | **`DECISION_REQUIRED`** |
| `mode_plan` | `Architect / Plan Mode (/plan)` | Modes & Policies | `LOCAL_IMPLEMENTABLE` | `Reuse frozen Plan mode toolchain and markdown plan pers...` | **`DECISION_REQUIRED`** |
| `svc_session_memory` | `Session Memory Periodic Extraction` | Memory & Services | `ANTHROPIC_MODEL_DEPENDENT` | `Bridge session memory extractions into Rust `brain-stor...` | **`DECISION_REQUIRED`** |
| `svc_auto_dream` | `Auto Dream Background Memory Consolidation` | Memory & Services | `ANTHROPIC_MODEL_DEPENDENT` | `Delegate background consolidation to Rust domain engine...` | **`DECISION_REQUIRED`** |
| `svc_compaction` | `Context Compaction & Token Summarization` | Memory & Services | `BRAIN_BACKEND_AVAILABLE` | `Delegate session compaction directly to Rust `SessionCo...` | **`DECISION_REQUIRED`** |
| `svc_lsp` | `Language Server Protocol (LSP) Manager` | Memory & Services | `LOCAL_IMPLEMENTABLE` | `Reuse frozen LSP service architecture....` | **`DECISION_REQUIRED`** |
| `tool_ship_audit` | `Tool: ship-audit` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_askuserquestion` | `Tool: AskUserQuestion` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_bash` | `Tool: Bash` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_brief` | `Tool: Brief` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_config` | `Tool: Config` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_enterplanmode` | `Tool: EnterPlanMode` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_enterworktree` | `Tool: EnterWorktree` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_exitplanmode` | `Tool: ExitPlanMode` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_exitworktree` | `Tool: ExitWorktree` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_fileedit` | `Tool: FileEdit` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_fileread` | `Tool: FileRead` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_filewrite` | `Tool: FileWrite` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_glob` | `Tool: Glob` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_grep` | `Tool: Grep` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_lsp` | `Tool: LSP` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_listmcpresources` | `Tool: ListMcpResources` | Tool UX & Execution | `BRAIN_BACKEND_AVAILABLE` | `Synchronize server definitions over UDS....` | **`DECISION_REQUIRED`** |
| `tool_mcp` | `Tool: MCP` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_mcpauth` | `Tool: McpAuth` | Tool UX & Execution | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `tool_notebookedit` | `Tool: NotebookEdit` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_powershell` | `Tool: PowerShell` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_repl` | `Tool: REPL` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_readmcpresource` | `Tool: ReadMcpResource` | Tool UX & Execution | `BRAIN_BACKEND_AVAILABLE` | `Synchronize server definitions over UDS....` | **`DECISION_REQUIRED`** |
| `tool_remotetrigger` | `Tool: RemoteTrigger` | Tool UX & Execution | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `tool_schedulecron` | `Tool: ScheduleCron` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_sendmessage` | `Tool: SendMessage` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_skill` | `Tool: Skill` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_sleep` | `Tool: Sleep` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_syntheticoutput` | `Tool: SyntheticOutput` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_taskcreate` | `Tool: TaskCreate` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_taskget` | `Tool: TaskGet` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_tasklist` | `Tool: TaskList` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_taskoutput` | `Tool: TaskOutput` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_taskstop` | `Tool: TaskStop` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_taskupdate` | `Tool: TaskUpdate` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_teamcreate` | `Tool: TeamCreate` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_teamdelete` | `Tool: TeamDelete` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_todowrite` | `Tool: TodoWrite` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_search` | `Tool: Search` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_tungstentool` | `Tool: TungstenTool` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `tool_webfetch` | `Tool: WebFetch` | Tool UX & Execution | `EXTERNAL_SERVICE_DEPENDENT` | `Reuse frozen TS implementation....` | **`DECISION_REQUIRED`** |
| `tool_websearch` | `Tool: WebSearch` | Tool UX & Execution | `EXTERNAL_SERVICE_DEPENDENT` | `Reuse frozen TS implementation....` | **`DECISION_REQUIRED`** |
| `tool_workflow` | `Tool: Workflow` | Tool UX & Execution | `LOCAL_IMPLEMENTABLE` | `Reuse frozen tool implementation....` | **`DECISION_REQUIRED`** |
| `cmd_add_dir` | `/add-dir` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_advisor` | `/advisor` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_agents` | `/agents` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_branch` | `/branch` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_remote_control` | `/remote-control` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_bridge_kick` | `/bridge-kick` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_brief` | `/brief` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_btw` | `/btw` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_chrome` | `/chrome` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_clear` | `/clear` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_color` | `/color` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_commit_push_pr` | `/commit-push-pr` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_commit` | `/commit` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_compact` | `/compact` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Delegate session compaction to Rust backend....` | **`DECISION_REQUIRED`** |
| `cmd_config` | `/config` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_context` | `/context` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_copy` | `/copy` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_cost` | `/cost` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_createMovedToPluginCommand` | `/createMovedToPluginCommand` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_desktop` | `/desktop` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_diff` | `/diff` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_doctor` | `/doctor` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Adapt Doctor diagnostic suite to run local engine probe...` | **`DECISION_REQUIRED`** |
| `cmd_effort` | `/effort` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_exit` | `/exit` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_export` | `/export` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_extra_usage` | `/extra-usage` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_fast` | `/fast` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_feedback` | `/feedback` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_files` | `/files` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_heapdump` | `/heapdump` | Command: Internal / Telemetry | `CLAUDE_SPECIFIC` | `Queue for explicit user KEEP / MODIFY / REMOVE decision...` | **`DECISION_REQUIRED`** |
| `cmd_help` | `/help` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_hooks` | `/hooks` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_ide` | `/ide` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_init_verifiers` | `/init-verifiers` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_init` | `/init` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_project_areas` | `/project_areas` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_install_github_app` | `/install-github-app` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_install_slack_app` | `/install-slack-app` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_install` | `/install` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_keybindings` | `/keybindings` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_login` | `/login` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_logout` | `/logout` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_mcp` | `/mcp` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_memory` | `/memory` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Adapt command to query Rust `brain-storage` and `brain-...` | **`DECISION_REQUIRED`** |
| `cmd_mobile` | `/mobile` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_model` | `/model` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_output_style` | `/output-style` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_passes` | `/passes` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_permissions` | `/permissions` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_plan` | `/plan` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_plugin` | `/plugin` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_pr_comments` | `/pr-comments` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_privacy_settings` | `/privacy-settings` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_rate_limit_options` | `/rate-limit-options` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_release_notes` | `/release-notes` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_reload_plugins` | `/reload-plugins` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_remote_env` | `/remote-env` | Command: External Cloud | `EXTERNAL_SERVICE_DEPENDENT` | `Quarantine within external capabilities boundary....` | **`DECISION_REQUIRED`** |
| `cmd_web_setup` | `/web-setup` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_rename` | `/rename` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_resume` | `/resume` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Preserve Claude UX and route backend operations to Rust...` | **`DECISION_REQUIRED`** |
| `cmd_review` | `/review` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_rewind` | `/rewind` | Command: Brain Adapted | `BRAIN_BACKEND_AVAILABLE` | `Delegate turn rollback to Rust storage checkpoint store...` | **`DECISION_REQUIRED`** |
| `cmd_sandbox` | `/sandbox` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_security_review` | `/security-review` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_session` | `/session` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_skills` | `/skills` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_stats` | `/stats` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_status` | `/status` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_statusline` | `/statusline` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_stickers` | `/stickers` | Command: Internal / Telemetry | `CLAUDE_SPECIFIC` | `Queue for explicit user KEEP / MODIFY / REMOVE decision...` | **`DECISION_REQUIRED`** |
| `cmd_tag` | `/tag` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_tasks` | `/tasks` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_terminal_setup` | `/terminal-setup` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_theme` | `/theme` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_think_back` | `/think-back` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_thinkback_play` | `/thinkback-play` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_ultraplan` | `/ultraplan` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_upgrade` | `/upgrade` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_usage` | `/usage` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_version` | `/version` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_vim` | `/vim` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |
| `cmd_voice` | `/voice` | Command: Local | `LOCAL_IMPLEMENTABLE` | `Reuse frozen command implementation....` | **`DECISION_REQUIRED`** |

---

## 9. Recommended Reconstruction Order

The dependency-ordered roadmap for reconstructing Claude capabilities backed by Brain:

```text
Stage 1: Local TUI & Shell Independence (Complete & Frozen via Contract A)
   ├── Theme state machine & config persistence
   ├── Multiline composer, vim modal input, keybindings
   ├── File completion (@) & command autocomplete (/)
   └── Local command suite (/diff, /commit, /status, /help, /config, /exit)
         │
         ▼
Stage 2: Context, Memory & Retrieval Seam (Phase 8.2)
   ├── Rust Brain authoritative context construction behind QueryDeps.callModel
   ├── Mathematical RRF hybrid ranking (BM25 + Vector + STM) with k=60.0
   ├── 1-hop bounded Knowledge Graph neighbor expansion
   └── Token budget packing and graceful fallback on degraded retrieval
         │
         ▼
Stage 3: Brain-Specific Extensions Exposed through Claude-Native UI
   ├── /graph -> Knowledge graph visualizer using Claude Pane + CustomSelect
   ├── /memory-debug -> STM/LTM fact inspector using Claude ThemedBox + ListItem
   └── /retrieval-debug -> RRF fusion score breakdown using StructuredDiff / tables
         │
         ▼
Stage 4: Session Storage & Lifecycle Unification
   ├── Unified session resume (/resume) querying Brain SQLite event store
   ├── Session compaction (/compact) delegated to Rust SessionContext
   └── Checkpoint rollback (/rewind) delegated to Rust CheckpointStore
         │
         ▼
Stage 5: MCP & Agent Swarm Synchronization
   ├── Synchronize TS /mcp server configuration with Rust brain-mcp-adapter
   └── Bridge TS subagent coordinator with Rust brain-a2a-adapter
         │
         ▼
Stage 6: Final Feature Disposition & External Policy Execution
   └── Process Decision Queue: execute user decisions for KEEP / MODIFY / REMOVE
```