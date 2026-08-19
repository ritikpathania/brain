# Phase 5.3 Forensic UI Audit & Implementation Specification: Conversation Rendering

> **Document Status**: Forensic Audit & Technical Specification (Complete — Pre-Implementation)  
> **Target Subsystem**: `packages/brain-frontend` (`src/components/Messages.tsx`, `src/components/MessageRow.tsx`, `src/components/messages/*`)  
> **Canonical Reference**: [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) (Sections 3, 4, 7, 8) & [`docs/design/CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md) (Primitives 5, 9, 10, 12, 13)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 5.3 FORENSIC UI AUDIT: CONVERSATION RENDERING
================================================================================
AUDITED RENDER PATH:
  PresentationState.timeline
        ↓
  <Messages /> (src/components/Messages.tsx)
        ↓
  <MessageRow /> (src/components/MessageRow.tsx)
        ↓
  ├── <UserTextMessage />          --> Upgrade to Claude-style User Prompt
  ├── <AssistantThinkingMessage />  --> Upgrade to subtle single-line Thinking Drawer
  ├── <AssistantToolUseMessage />   --> Upgrade to structured Tool Card
  ├── <UserToolResultMessage />     --> Upgrade to Line-Numbered Output Drawer
  └── <AssistantTextMessage />      --> Upgrade with MarkdownText & Syntax Highlighter
STATE MODEL GAPS: ZERO (0 types modified in types/presentation.ts)
BACKEND & PROTOCOL GAPS: ZERO (0 Rust lines modified, 0 UDS changes)
AUDIT VERDICT: PROCEED TO PHASE 5.3 IMPLEMENTATION
================================================================================
```

---

## 1. Exact Current Render Tree

```text
<Messages messages={state.timeline} isStreaming={state.streaming.isStreaming}>
  │
  ├── [Empty State] -> <WelcomeHero /> (Phase 5.2 certified)
  │
  ├── [Historical / Active Messages]
  │     └── <MessageRow message={msg}>
  │           │
  │           ├── IF msg.role === 'user':
  │           │     └── <UserTextMessage content={msg.content} />
  │           │           └── <Text color="cyan">❯ </Text> <Text color="white">{content}</Text>
  │           │
  │           └── IF msg.role === 'assistant' || msg.role === 'system':
  │                 ├── <AssistantThinkingMessage thinking={msg.thinking} />
  │                 │     └── <Text color="magenta">⏺ Thinking... (Xs)</Text>
  │                 │           └── (if isExpanded) Box borderColor="magenta"
  │                 │
  │                 ├── <AssistantToolUseMessage tool={tool} />
  │                 │     └── <Box borderStyle="round">
  │                 │           └── <Text>{name} ({JSON.stringify(args)}) [{state}]</Text>
  │                 │
  │                 ├── <UserToolResultMessage output={tool.output} />
  │                 │     └── (if isExpanded) Box borderColor="gray"
  │                 │
  │                 └── <AssistantTextMessage content={msg.content} />
  │                       └── <Text color="green">◈ Assistant</Text>
  │                       └── <Text color="white">{content}</Text>
  │
  └── [Active Streaming Chunk (Trailing)]
        └── (Rendered inline with active cursor)
```

---

## 2. Available Message Variants & Fields in `types/presentation.ts`

| Type / Interface | Field | Type | Description |
|---|---|---|---|
| `PresentationMessage` | `id` | `string` | Unique identifier. |
| | `role` | `'user' \| 'assistant' \| 'system'` | Role discriminator. |
| | `content` | `string` | Text content (User prompt, assistant markdown, system notice). |
| | `thinking` | `{ text, durationMs, isExpanded, isStreaming }` | Structured reasoning state. |
| | `tools` | `PresentationToolCall[]` | Tool invocations attached to message. |
| `PresentationToolCall` | `id` | `string` | Call ID (e.g. `call_123`). |
| | `name` | `string` | Tool name (e.g. `read_file`, `compile`). |
| | `args` | `Record<string, unknown>` | Parsed JSON tool arguments. |
| | `state` | `'pending' \| 'running' \| 'completed' \| 'failed' \| 'denied'` | Interactive execution lifecycle. |
| | `output` | `string` (optional) | Standard output / execution result. |
| | `error` | `string` (optional) | Error message on failure. |
| | `isExpanded` | `boolean` (optional) | Drawer expansion state. |

**Audit Finding**: The existing `types/presentation.ts` contract is **100% complete and expressive**. Zero fields need to be added or modified.

---

## 3. Streaming Text, Thinking & Tool Lifecycles

1. **Streaming Text**:
   - `BrainFrontendAdapter` appends incoming tokens to both `currentMsg.content` and `state.streaming.activeText`.
   - In `Messages.tsx`, we streamline the streaming renderer so that `AssistantTextMessage` renders the active message directly with trailing cursor `▌` during streaming, avoiding duplicate text blocks.
2. **Thinking / Reasoning Events**:
   - `Stage` stream events update `msg.thinking.isStreaming = true` and track `durationMs`.
   - On first token or completion, `isStreaming = false` and elapsed duration freezes.
   - Claude contract: Renders `⠋ Thinking (X.Xs)...` while active, collapsing to `✔ Thought for X.Xs` with subtle `[Ctrl+O to expand]` hint on completion.
3. **Tool Invocations & Approvals**:
   - `ToolCallRequest` sets `state: 'pending'` (if `requires_approval`) or `'running'`.
   - Controller handles `y` / `Enter` $\rightarrow$ `'running'`, `n` / `Esc` $\rightarrow$ `'denied'`.
   - Claude contract: Structured card showing tool name, formatted key parameters, status badge, and line-numbered output drawer.

---

## 4. Markdown & Syntax Highlighting Architecture

1. **Repository Audit**:
   - No external markdown npm package is currently installed in `package.json`.
   - `crates/brain-tui` contains a custom Rust markdown AST parser.
2. **Implementation Strategy**:
   - Implement `src/components/messages/MarkdownText.tsx` natively in TypeScript with Ink components.
   - **Zero New Dependencies**: Uses existing React + Ink primitives (`Box`, `Text`) and `ThemeTokens`.
   - **Supported Markdown Features**:
     - Fenced code blocks (`` ```lang ... ``` ``) with language tag, rounded container box, line numbers, and keyword syntax highlighting.
     - Diff blocks (`` ```diff `` or `+`/`-` lines) with green/red line coloring.
     - Headings (`#`, `##`, `###`) with bold accent styling.
     - Bullet lists (`-`, `*`) with `•` bullet glyph.
     - Inline formatting (`**bold**`, `*italic*`, `` `code` ``).

---

## 5. Component Migration Plan for Phase 5.3

| Component File | Action | Changes in Phase 5.3 |
|---|---|---|
| `src/components/messages/MarkdownText.tsx` | **[NEW]** | Lightweight native Markdown & syntax-highlighted code block renderer. |
| `src/components/messages/UserTextMessage.tsx` | **[REFACTOR]** | Clean `❯ ` user prompt with high-contrast text and clean margin spacing. |
| `src/components/messages/AssistantTextMessage.tsx` | **[REFACTOR]** | Replaces plain text with `<MarkdownText />` and trailing streaming cursor. |
| `src/components/messages/AssistantThinkingMessage.tsx` | **[REFACTOR]** | Single-line subtle summary (`✔ Thought for 2.4s`) + collapsible dimmed trace. |
| `src/components/messages/AssistantToolUseMessage.tsx` | **[REFACTOR]** | Structured tool card with status glyphs (`✔`, `⌛`, `✖`), formatted parameters, and inline `[y/n]` approval prompt. |
| `src/components/messages/UserToolResultMessage.tsx` | **[REFACTOR]** | Line-numbered collapsible drawer with 20-line cap and subtle border. |
| `src/components/Messages.tsx` & `MessageRow.tsx` | **[REFACTOR]** | Unified streaming display, system message styling, and 1-cell vertical spacing. |

---

## 6. Audit Verdict

```text
================================================================================
AUDIT VERDICT:
PROCEED — ALL REQUIREMENTS & STATE MAPPINGS FULLY VERIFIED
================================================================================
```
