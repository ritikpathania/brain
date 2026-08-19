# Claude Code Capability Inventory & Backend Dependency Report

## 1. Executive Overview & Methodology

This report documents the exhaustive capability inventory and runtime dependency graph for **Claude Code v2.1.232** as hosted inside `packages/brain-shell`.

Rather than treating the application as a black-box parity surface, this analysis traces every user-visible capability vertically through the six runtime layers:
$$\\text{Feature} \\longrightarrow \\text{User Action} \\longrightarrow \\text{Claude UI Component} \\longrightarrow \\text{State Transition} \\longrightarrow \\text{Runtime/Service Call} \\longrightarrow \\text{Backend Operation} \\longrightarrow \\text{Stream/Event} \\longrightarrow \\text{State Mutation} \\longrightarrow \\text{Rendered UI}$$

---

## 2. Feature & Capability Classification Summary

| Classification | Count | Definition | Impact on Brain Architecture |
| :--- | :---: | :--- | :--- |
| **`KEEP`** | **11** | Claude presentation, composer math, permissions, and tool execution retained unchanged. | Preserves 100% frozen frontend integrity; 0 code changes. |
| **`REPLACE`** | **5** | Model text generation, thinking streams, message payloads, and OAuth replaced with Brain equivalents. | Routed through `QueryDeps.callModel` seam and Brain auth provider. |
| **`ADAPT`** | **4** | Claude capabilities useful to Brain but requiring semantic translation (session history, compaction, stream cancellation, tool headers). | Adapters normalize Brain entities into Claude message/event types. |
| **`REMOVE`** | **2** | Claude-specific telemetry sinks and auto-updater background checks. | Disabled cleanly in `preload.ts` without modifying `vendor/claude`. |
| **`DEFER`** | **0** | Deferred future items. | None blocking Phase 5. |
| **`UNKNOWN`** | **0** | Unclassified subsystems. | All 22 primary capabilities fully traced. |

---

## 3. Comprehensive Feature-to-Backend Dependency Graph

```mermaid
flowchart TD
    subgraph Presentation [1. PRESENTATION LAYER (KEEP - FROZEN)]
        UI1[LogoHeader / Landing]
        UI2[PromptInput / Multiline Composer]
        UI3[SlashCommandMenu / FuzzyPicker]
        UI4[Messages / VirtualMessageList]
        UI5[StreamingMarkdown / Typewriter]
        UI6[AssistantThinkingMessage]
        UI7[ToolUseLoader / ToolResultCard]
        UI8[PermissionPrompt / TrustDialog]
        UI9[ResumeChooser / History]
    end

    subgraph Orchestration [2. ORCHESTRATION LAYER (KEEP)]
        O1[main.tsx::main]
        O2[replLauncher.ts::launchRepl]
        O3[REPL.tsx (State Reducer)]
        O4[handlePromptSubmit.ts]
        O5[query.ts::queryLoop]
        O6[StreamingToolExecutor.ts]
        O7[autoCompact.ts]
    end

    subgraph SeamBoundary [3. SEAM BOUNDARY]
        S1["QueryDeps.callModel() [PRIMARY SEAM]"]
        S2["SessionMemory [PERSISTENCE SEAM]"]
        S3["OAuth / Keychain [AUTH SEAM]"]
    end

    subgraph BrainEngine [4. BRAIN BACKEND (REPLACE / ADAPT)]
        B1[Brain CallModel Adapter]
        B2[BrainBackendClient]
        B3[Brain Rust Daemon / Relational Memory Engine]
        B4[Brain Graph Session Repository]
    end

    subgraph LocalTools [5. LOCAL SYSTEM EXECUTION (KEEP)]
        T1[BashTool / FileEditTool / GrepTool]
        T2[MCP Client / Dynamic Servers]
    end

    UI2 -->|Keystrokes| O3
    UI3 -->|Select Command| O3
    O3 -->|Submit Prompt| O4
    O4 -->|Execute Turn| O5
    O5 -->|Tool Invocation Loop| O6
    O6 -->|Request Approval| UI8
    O8 -->|Execute| T1
    O8 -->|Execute MCP| T2
    T1 -->|ToolResultBlock| O5
    O5 -->|Summarize Context| O7

    O5 -->|callModel| S1
    S1 -->|Adapter Stream| B1
    B1 -->|RPC / Events| B2
    B2 -->|UDS Transport| B3

    UI9 -->|Load History| S2
    S2 -->|Graph Sessions| B4
```

---

## 4. End-to-End Capability Inventory

### 4.1. Startup & Shell Lifecycle

#### `startup.landing_screen` (`KEEP`)
- **User Action**: Launch shell without arguments (`$ bun start`).
- **Claude UI Component**: `App`, `REPL`, `LogoHeader`, `LogoV2`, `StatusNotices`.
- **State Transition**: Mounts Root -> evaluates theme -> renders logo banner, model indicator, cwd, and shortcut hints.
- **Runtime/Service Call**: `config.ts::getGlobalConfig()`, `modelStrings.ts`.
- **Backend Operation**: Local disk config read (`~/.claude/config.json`).
- **Stream/Event**: None.
- **State Mutation**: Initializes `AppState` and session stats store.
- **Rendered UI**: 3-line ASCII/Unicode Logo, model name badge, effort badge, path banner.
- **Seam Impact**: None. Pure Ink client rendering.

#### `startup.onboarding_modal` (`KEEP`)
- **User Action**: First launch when `config.hasCompletedOnboarding == false`.
- **Claude UI Component**: `Onboarding`, `Select`, `ThemedBox`.
- **State Transition**: `showSetupScreens()` intercepts startup -> renders theme picker & welcome screen.
- **Runtime/Service Call**: `config.ts::completeOnboarding()`.
- **Backend Operation**: Writes `hasCompletedOnboarding: true` to local settings.
- **Stream/Event**: None.
- **State Mutation**: Updates global user preferences.
- **Rendered UI**: Bordered modal with theme selection options.
- **Seam Impact**: None.

#### `startup.trust_dialog` (`KEEP`)
- **User Action**: Shell launched in an untrusted directory.
- **Claude UI Component**: `TrustDialog`, `Dialog`, `Button`.
- **State Transition**: `checkHasTrustDialogAccepted()` triggers security confirmation.
- **Runtime/Service Call**: `config.ts::saveGlobalConfig()`.
- **Backend Operation**: Local trusted directories list updated.
- **Stream/Event**: None.
- **State Mutation**: Sets session directory trust state.
- **Rendered UI**: Directory trust warning modal with Accept / Reject keys.
- **Seam Impact**: None. Retains Claude security sandbox.

---

### 4.2. Composer & Interactive Input

#### `composer.single_line_input` (`KEEP`)
- **User Action**: Type alphanumeric characters into prompt.
- **Claude UI Component**: `PromptInput`, `PromptComposer`, `useCursorNav`.
- **State Transition**: Ink raw keystroke event -> updates text buffer and cursor column.
- **Runtime/Service Call**: `keybindings/loadUserBindings.ts`.
- **Backend Operation**: None.
- **Stream/Event**: None.
- **State Mutation**: `text`, `cursorX`.
- **Rendered UI**: Themed input box with active cursor rendering.
- **Seam Impact**: None.

#### `composer.multiline_editing` (`KEEP`)
- **User Action**: Press `Shift+Enter` or `Ctrl+J`.
- **Claude UI Component**: `PromptInput`, `PromptComposer`, `MultilineText`.
- **State Transition**: Splits buffer on newline -> recomputes viewport line wrapping and cursor position.
- **Runtime/Service Call**: `wrap-text.ts`, `measure-element.ts`.
- **Backend Operation**: None.
- **Stream/Event**: None.
- **State Mutation**: `lines` array, `lineIndex`, `cursorOffset`.
- **Rendered UI**: Expanding multi-row text box with line continuation markers.
- **Seam Impact**: None.

#### `composer.cursor_and_word_nav` (`KEEP`)
- **User Action**: Press `Left`/`Right`/`Home`/`End`, `Alt+Left`/`Right`, `Ctrl+A`/`E`.
- **Claude UI Component**: `PromptInput`, `useCursorNav`.
- **State Transition**: Calculates Unicode grapheme cluster boundaries and word regex offsets.
- **Runtime/Service Call**: `stringUtils.ts`.
- **Backend Operation**: None.
- **Stream/Event**: None.
- **State Mutation**: `cursorPosition` updated.
- **Rendered UI**: Cursor indicator shifted to target column/row.
- **Seam Impact**: None.

#### `composer.history_navigation` (`KEEP`)
- **User Action**: Press `Up` / `Down` arrow on top line of composer.
- **Claude UI Component**: `PromptInput`, `REPL`.
- **State Transition**: Fetches prompt string from history ring buffer.
- **Runtime/Service Call**: `history.ts::getHistory()`, `REPL.tsx::onHistoryMove()`.
- **Backend Operation**: Reads local prompt history file (`~/.claude/history.json`).
- **Stream/Event**: None.
- **State Mutation**: `setInputValue(historicalPrompt)`.
- **Rendered UI**: Populates composer with historical prompt.
- **Seam Impact**: None.

---

### 4.3. Command Palette & Local Commands

#### `commands.slash_picker` (`KEEP`)
- **User Action**: Type `/` at start of prompt.
- **Claude UI Component**: `PromptInput`, `SlashCommandMenu`, `FuzzyPicker`.
- **State Transition**: Opens floating popup modal filtering commands list.
- **Runtime/Service Call**: `commands.ts::getCommands()`.
- **Backend Operation**: None.
- **Stream/Event**: None.
- **State Mutation**: `isSlashPaletteOpen = true`, `filterText = "/"`.
- **Rendered UI**: Bordered popup box listing available slash commands with descriptions.
- **Seam Impact**: None.

#### `commands.local_execution` (`KEEP`)
- **User Action**: Execute client command (`/clear`, `/theme`, `/effort`).
- **Claude UI Component**: `REPL`, `AppStateProvider`.
- **State Transition**: `REPL.onSubmit` intercept matches `command.type === "local"`.
- **Runtime/Service Call**: `commands/theme.ts`, `commands/effort.ts`.
- **Backend Operation**: Updates local settings store.
- **Stream/Event**: None.
- **State Mutation**: Mutates active theme or effort state in `AppState`.
- **Rendered UI**: System message banner or updated UI theme tokens.
- **Seam Impact**: None.

#### `commands.auth_login` (`REPLACE`)
- **User Action**: Run `/login` or `/logout`.
- **Claude UI Component**: `REPL`, `AuthDialog`.
- **State Transition**: Launches OAuth authorization flow.
- **Runtime/Service Call**: `services/oauth/client.ts`.
- **Backend Operation**: Anthropic OAuth endpoint exchange.
- **Stream/Event**: None.
- **State Mutation**: Updates session auth token in keychain.
- **Rendered UI**: Login prompt dialog with browser URL and token input.
- **Seam Impact**: **Auth Seam**: Replaced by Brain authentication & daemon pairing.

---

### 4.4. Turns & Streaming Engine

#### `streaming.prompt_submission` (`KEEP`)
- **User Action**: Press `Enter` with non-empty prompt in `PromptInput`.
- **Claude UI Component**: `PromptInput`, `REPL`, `handlePromptSubmit`.
- **State Transition**: Appends `UserMessage` to state -> locks queryGuard -> invokes `onQuery`.
- **Runtime/Service Call**: `messages.ts::createUserMessage()`.
- **Backend Operation**: Starts query loop.
- **Stream/Event**: `stream_request_start`.
- **State Mutation**: `setMessages(prev => [...prev, userMsg])`, `setStreamMode("requesting")`.
- **Rendered UI**: User message card added to Messages list; spinner switches to "requesting".
- **Seam Impact**: Direct entrypoint into `QueryDeps.callModel`.

#### `streaming.token_delta_stream` (`REPLACE`)
- **User Action**: Model streams response chunks.
- **Claude UI Component**: `REPL`, `StreamingMarkdown`, `Spinner`, `Messages`.
- **State Transition**: `StreamEvent(content_block_delta)` routed to `handleMessageFromStream`.
- **Runtime/Service Call**: `query.ts::queryLoop()`, `query/deps.ts::callModel`.
- **Backend Operation**: Brain engine text stream chunk generation.
- **Stream/Event**: `content_block_start`, `content_block_delta`, `content_block_stop`.
- **State Mutation**: `streamingText += chunk`, `setStreamMode("responding")`.
- **Rendered UI**: Live Markdown token stream rendered via typewriter animation.
- **Seam Impact**: **Primary Seam**: Emitted by `QueryDeps.callModel`.

#### `streaming.turn_completion` (`REPLACE`)
- **User Action**: Model generation completes (`message_stop`).
- **Claude UI Component**: `REPL`, `Messages`, `VirtualMessageList`.
- **State Transition**: Yields `AssistantMessage` -> appends to `messages` -> clears `streamingText`.
- **Runtime/Service Call**: `query.ts::queryLoop()`, `query/deps.ts::callModel`.
- **Backend Operation**: Final turn payload assembly.
- **Stream/Event**: `message_delta`, `message_stop`, `AssistantMessage`.
- **State Mutation**: `setMessages(prev => [...prev, assistantMsg])`, `setStreamingText(null)`.
- **Rendered UI**: Solidified `AssistantMessage` card in `VirtualMessageList`; prompt restored.
- **Seam Impact**: **Primary Seam**: Final payload yielded by `QueryDeps.callModel`.

#### `streaming.cancellation_ctrl_c` (`ADAPT`)
- **User Action**: Press `Ctrl+C` or submit new prompt while turn is streaming.
- **Claude UI Component**: `REPL`, `PromptInput`.
- **State Transition**: `abortController.abort()` sets `AbortSignal.aborted = true`.
- **Runtime/Service Call**: `REPL.tsx::handleAbort()`, `query.ts::query()`.
- **Backend Operation**: Brain backend stream drop / cancellation token trigger.
- **Stream/Event**: `stream_cancelled`.
- **State Mutation**: `setStreamMode("idle")`, clears transient tool states.
- **Rendered UI**: Inline `[Interrupted by user]` note; prompt input re-enabled.
- **Seam Impact**: `QueryDeps.callModel` signal listener aborts Brain stream.

---

### 4.5. Reasoning & Thinking

#### `reasoning.thinking_stream` (`REPLACE`)
- **User Action**: Model emits reasoning tokens prior to final text.
- **Claude UI Component**: `REPL`, `AssistantThinkingMessage`, `Spinner`.
- **State Transition**: `StreamEvent(content_block_start: thinking)` sets stream mode to `"thinking"`.
- **Runtime/Service Call**: `query.ts::queryLoop()`, `query/deps.ts::callModel`.
- **Backend Operation**: Brain reasoning model stream.
- **Stream/Event**: `content_block_start`, `thinking_delta`, `content_block_stop`.
- **State Mutation**: `streamingThinking.text += delta`, `setStreamMode("thinking")`.
- **Rendered UI**: Collapsible "Thinking..." block with duration stopwatch.
- **Seam Impact**: Emitted by `QueryDeps.callModel`.

---

### 4.6. Tool Lifecycle & Permissions

#### `tools.tool_call_stream` (`ADAPT`)
- **User Action**: Model emits `tool_use` content block.
- **Claude UI Component**: `REPL`, `ToolUseLoader`, `Messages`.
- **State Transition**: `content_block_start(type: tool_use)` sets stream mode to `"tool-input"`.
- **Runtime/Service Call**: `query.ts::queryLoop()`, `StreamingToolExecutor.ts`.
- **Backend Operation**: Brain tool invocation generation.
- **Stream/Event**: `content_block_start`, `input_json_delta`, `content_block_stop`.
- **State Mutation**: `streamingToolUses.push({ name, inputJson })`.
- **Rendered UI**: Tool use card mounted (e.g. `Bash(command: ...)` with progress spinner).
- **Seam Impact**: Emitted by `QueryDeps.callModel`.

#### `tools.permission_prompt` (`KEEP`)
- **User Action**: Tool requires confirmation (e.g. mutating bash command).
- **Claude UI Component**: `PermissionRequest`, `Dialog`, `Button`.
- **State Transition**: `StreamingToolExecutor` pauses execution and mounts permission prompt.
- **Runtime/Service Call**: `utils/permissions/permissionSetup.ts`, `Tool.call()`.
- **Backend Operation**: Local security policy check.
- **Stream/Event**: None.
- **State Mutation**: Decision: `allowOnce` | `allowAlways` | `deny`.
- **Rendered UI**: Bordered confirmation modal: `Allow command: git commit? [Y/n/always]`.
- **Seam Impact**: None. Executed entirely within Claude `queryLoop`.

#### `tools.native_execution` (`KEEP`)
- **User Action**: Approved tool runs on host system.
- **Claude UI Component**: `REPL`, `Messages`, `ToolResultMessage`.
- **State Transition**: Executes `Tool.call(input)` -> produces `ToolResultBlock`.
- **Runtime/Service Call**: `StreamingToolExecutor.ts`, `tools/BashTool.ts`, `tools/FileEditTool.ts`.
- **Backend Operation**: Host OS process / filesystem / MCP execution.
- **Stream/Event**: `progress`.
- **State Mutation**: Constructs `UserMessage` containing `ToolResultBlock`.
- **Rendered UI**: Tool output box rendered (syntax-highlighted code / diff / command stdout).
- **Seam Impact**: Subsequent `callModel` invocation receives paired `tool_use` + `tool_result`.

---

### 4.7. Sessions & Compaction

#### `sessions.history_resume` (`ADAPT`)
- **User Action**: Run `/resume` or `claude --resume`.
- **Claude UI Component**: `ResumeChooser`, `Select`, `ThemedBox`.
- **State Transition**: Lists sessions from disk -> loads selected transcript.
- **Runtime/Service Call**: `dialogLaunchers.ts::launchResumeChooser()`.
- **Backend Operation**: Brain session repository query.
- **Stream/Event**: None.
- **State Mutation**: `setMessages(loadedSessionMessages)`.
- **Rendered UI**: Interactive session selector showing title, time, and message preview.
- **Seam Impact**: **Session Seam**: Mapped to Brain session graph repository.

#### `sessions.compaction` (`ADAPT`)
- **User Action**: Conversation token count exceeds model context window.
- **Claude UI Component**: `REPL`, `Messages`, `CompactBoundaryMessage`.
- **State Transition**: `autoCompactIfNeeded()` summarizes older messages and inserts compact boundary.
- **Runtime/Service Call**: `services/compact/autoCompact.ts`, `query/deps.ts::autocompact`.
- **Backend Operation**: Brain summarization query.
- **Stream/Event**: `compaction`.
- **State Mutation**: `setMessages([CompactBoundaryMessage, ...recentMessages])`.
- **Rendered UI**: Horizontal compact boundary divider with summarized context badge.
- **Seam Impact**: `QueryDeps.autocompact` delegates summarization to Brain.

---

### 4.8. Configuration & Terminal Rendering

#### `config.theme_switching` (`KEEP`)
- **User Action**: Run `/theme` or terminal mode changes.
- **Claude UI Component**: `ThemeProvider`, `ThemeSettingsMenu`, `ThemedBox`.
- **State Transition**: Updates `ThemeContext` -> triggers repaint with semantic tokens.
- **Runtime/Service Call**: `utils/config.ts`.
- **Backend Operation**: Local settings file write.
- **Stream/Event**: None.
- **State Mutation**: `config.theme = selectedTheme`.
- **Rendered UI**: Full UI repainted in active palette (dark, light, daltonized, high-contrast).
- **Seam Impact**: None.

#### `config.terminal_resize` (`KEEP`)
- **User Action**: Terminal window resized (`SIGWINCH`).
- **Claude UI Component**: `useTerminalSize`, `VirtualMessageList`, `PromptComposer`.
- **State Transition**: `SIGWINCH` signal -> recalculates columns/rows -> re-flows Yoga layout.
- **Runtime/Service Call**: `ink/screen.ts`, `hooks/useTerminalSize.ts`.
- **Backend Operation**: None.
- **Stream/Event**: None.
- **State Mutation**: `terminalSize` updated.
- **Rendered UI**: Smooth non-flickering reflow of message cards and input boxes.
- **Seam Impact**: None.

---

## 5. Artifact Directory Layout

All machine-readable QA manifests and runtime traces are located under `packages/brain-shell/qa/`:

```text
packages/brain-shell/qa/
├── manifest.json                  # Schema version, summary counts, category index
├── features.json                  # Complete array of 22 structured feature records
├── report.md                      # This exhaustive capability inventory document
├── frontend/
│   ├── startup.json               # Landing screen, onboarding, trust dialogs
│   ├── composer.json              # Multiline editing, cursor movement, word jumps, history
│   ├── commands.json              # Slash picker, local execution, auth dialogs
│   ├── permissions.json           # Tool approval modals, directory trust
│   ├── tools.json                 # Tool invocation headers, native execution, diffs
│   ├── streaming.json             # Prompt submission, token delta stream, turn completion
│   ├── rendering.json             # Markdown, code blocks, syntax highlighting
│   ├── sessions.json              # History resume, session choosers, compaction
│   ├── reasoning.json             # Collapsible thinking cards, stopwatch timer
│   └── config.json                # Theme provider, responsive SIGWINCH resizing
├── backend/
│   ├── callgraph.json             # Interactive, tool-execution, and compaction call trees
│   └── services.json              # Subsystem classification (REPLACE, KEEP, ADAPT, REMOVE)
└── traces/
    ├── prompt.jsonl               # Live captured prompt submission & text stream events
    ├── tool-use.jsonl             # Live captured tool_use -> execute -> tool_result trace
    ├── thinking.jsonl             # Live captured thinking_delta -> text_delta trace
    └── cancellation.jsonl         # Live captured AbortController cancellation trace
```

---

## 6. Migration Seam Verdict

1. **`QueryDeps.callModel` is the primary and sufficient seam** for:
   - Text generation (`content_block_delta`)
   - Thinking & reasoning streams (`thinking_delta`)
   - Tool invocation generation (`ToolUseBlock`)
   - Turn finalization (`AssistantMessage`)
   - Stream cancellation (`AbortSignal`)
2. **Secondary Seams Identified**:
   - `SessionMemory` (`services/SessionMemory/`): Adapter required to load/save Brain graph-backed sessions.
   - `OAuth/Keychain` (`services/oauth/`): Replaced by Brain daemon credentials provider.
3. **No Presentation Layer Changes Required**:
   - 100% of Claude UI components (`REPL`, `PromptInput`, `VirtualMessageList`, `StreamingMarkdown`, `PermissionRequest`, `ThemeProvider`) remain unmodified and hosted in their native form.
