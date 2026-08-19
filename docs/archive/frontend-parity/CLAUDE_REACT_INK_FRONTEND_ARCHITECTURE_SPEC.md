# Specification — Claude Code React/Ink Frontend Architecture (Phase 0)

> **Document Status**: Approved Frontend Architecture & Source Mapping Specification  
> **Target Subsystems**: Presentation Layer, React + Ink + Yoga Component Architecture, Mock Data Engine  
> **Core Architecture Change**: Transition from Ratatui reimplementation to **React + Ink + Yoga** frontend stack  
> **Authoritative Oracle**: Claude Code React Frontend Source (`/Users/ritikpathania/Developer/src/**`)  
> **Legacy Status**: `crates/brain-tui` (Ratatui) classified as `LEGACY / REFERENCE INFRASTRUCTURE`  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

```text
==================================================
CORE ARCHITECTURE DECISION
==================================================
Claude Code Source Oracle (/Users/ritikpathania/Developer/src/**)
      ↓
React + Ink + Yoga Component & Layout Engine
      ↓
Terminal Output (fd 1)

Order of Execution:
1. Reconstruct Claude Code React/Ink Frontend (Phase 0)
2. Mechanical Parity & Viewport Verification
3. Brain PresentationAdapter Boundary
4. Brain Runtime Integration & Semantics
5. Prune / Map Unsupported Claude Features
```

---

## 1. Executive Strategy & Rendering Architecture Change

This document supersedes all prior references to Ratatui as the target frontend renderer.

### Strategic Directives:
1. **Target Architecture**: **React + Ink + Yoga**. The frontend is reconstructed using React terminal components rendered through Ink and laid out via Yoga flexbox.
2. **Oracle Source Hierarchy**: The frontend component tree, props, layout rules, and interaction models directly mirror Claude Code's verified source files in `/Users/ritikpathania/Developer/src`.
3. **No Mixed Architectures**: The application will **not** combine Ink and Ratatui. Ratatui is frozen as legacy reference infrastructure for backend view models and test suites.
4. **Temporary Claude Parity Integrity**: During Phase 0, Claude-specific features (`/model`, `/effort`, model selector, token/cost counters, billing badges) are reproduced via mock data to guarantee visual and interaction parity before Brain adaptation begins.

---

## 2. Claude Source Oracle & Component Inventory Trace

A comprehensive audit of `/Users/ritikpathania/Developer/src` establishes the exact component inventory driving Claude Code:

| Component Category | Claude Source File | Primary Responsibility |
| :--- | :--- | :--- |
| **Shell & Layout** | `/Users/ritikpathania/Developer/src/components/FullscreenLayout.tsx` | Main viewport container, scrollbox slot, sticky header, new-messages pill, modal slots. |
| **Virtual Scroll** | `/Users/ritikpathania/Developer/src/components/VirtualMessageList.tsx` | Windowed message rendering, sticky header tracking, viewport scroll calculations. |
| **Timeline Container** | `/Users/ritikpathania/Developer/src/components/Messages.tsx` | Message list wrapper, unseen message divider line, timeline item dispatch. |
| **Row Dispatcher** | `/Users/ritikpathania/Developer/src/components/MessageRow.tsx` | Renders user/assistant rows based on message type. |
| **Assistant Text** | `/Users/ritikpathania/Developer/src/components/messages/AssistantTextMessage.tsx` | Markdown formatting, code fences, inline syntax highlighting. |
| **Thinking Block** | `/Users/ritikpathania/Developer/src/components/messages/AssistantThinkingMessage.tsx` | Reasoning header, status indicator, collapsible thinking trace drawer. |
| **Tool Execution** | `/Users/ritikpathania/Developer/src/components/messages/AssistantToolUseMessage.tsx` | Tool call request, permission prompt, state badges (`Pending`, `Running`, `Completed`, `Failed`). |
| **Tool Result** | `/Users/ritikpathania/Developer/src/components/messages/UserToolResultMessage/index.tsx` | Indented tool execution output drawer capped at 20 lines with line numbers. |
| **User Prompt Row** | `/Users/ritikpathania/Developer/src/components/messages/UserTextMessage.tsx` | User prompt representation `❯ <text>` in timeline. |
| **Prompt Editor** | `/Users/ritikpathania/Developer/src/components/BaseTextInput.tsx` | Multiline prompt input, soft wrapping, hard newlines, visual cursor movement. |
| **Text Input Wrapper** | `/Users/ritikpathania/Developer/src/components/TextInput.tsx` | Input state management, history escalation boundary handling. |
| **Status Bar** | `/Users/ritikpathania/Developer/src/components/StatusLine.tsx` | Bottom row status line: active model, token/cost counters, key hints. |
| **Model Picker** | `/Users/ritikpathania/Developer/src/components/ModelPicker.tsx` | `/model` selection dialog. |
| **Effort Callout** | `/Users/ritikpathania/Developer/src/components/EffortCallout.tsx` | `/effort` reasoning tier indicator. |
| **Command Palette** | `/Users/ritikpathania/Developer/src/components/GlobalSearchDialog.tsx` | `Ctrl+K` modal overlay with fuzzy matching and command ranking. |
| **Quick Open** | `/Users/ritikpathania/Developer/src/components/QuickOpenDialog.tsx` | File search and quick selection modal overlay. |
| **Ink Core Engine** | `/Users/ritikpathania/Developer/src/ink/ink.tsx` | Custom Ink terminal reconciler, ANSI parser, Yoga layout bridge. |

---

## 3. Explicit Source Mapping & Reuse Policy

```text
REUSE & RECONSTRUCTION POLICY
- Architecture & Boundaries: 100% Reused from Claude Source Oracle
- Component Contracts & Props: 100% Reused from Claude Source Oracle
- Layout Geometry (Yoga): 100% Reused from Ink/Yoga Layout Tree
- Licensing Strategy: Clean-room TypeScript/React implementations matching Claude component contracts
```

### Component Mapping Table:

| Claude Source Oracle File | Brain React/Ink Component | Reuse / Reconstruction Method | Justification |
| :--- | :--- | :--- | :--- |
| `FullscreenLayout.tsx` | `BrainFullscreenLayout` | **Direct Structure Reconstruction** | Authoritative 3-slot flexbox layout (`scrollable`, `bottom`, `modal`). |
| `VirtualMessageList.tsx` | `BrainVirtualMessageList` | **Direct Structure Reconstruction** | Manages scroll-derived chrome (sticky prompt header, new-messages pill). |
| `Messages.tsx` | `BrainMessageList` | **Clean-Room Implementation** | Renders timeline rows and unseen divider. |
| `AssistantTextMessage.tsx` | `BrainAssistantTextMessage` | **Direct Structure Reconstruction** | Markdown parser and syntax highlighting tree. |
| `AssistantThinkingMessage.tsx` | `BrainThinkingMessage` | **Direct Structure Reconstruction** | Indented reasoning drawer with `Ctrl+O` toggle contract. |
| `AssistantToolUseMessage.tsx` | `BrainToolExecutionCard` | **Direct Structure Reconstruction** | 5 tool lifecycle states (`Pending`, `Running`, `Completed`, `Failed`, `Denied`). |
| `UserToolResultMessage/` | `BrainToolResultDrawer` | **Direct Structure Reconstruction** | 20-line drawer truncation cap and line numbers. |
| `BaseTextInput.tsx` | `BrainBaseTextInput` | **Direct Structure Reconstruction** | Visual-line Up/Down, Home/End, Ctrl+A/E/K/Y editor logic. |
| `StatusLine.tsx` | `BrainStatusLine` | **Clean-Room Implementation** | Status bar, model indicator, token/cost counters. |
| `GlobalSearchDialog.tsx` | `BrainCommandPalette` | **Clean-Room Implementation** | `Ctrl+K` overlay modal slot in `FullscreenLayout`. |

---

## 4. React + Ink + Yoga Component Tree Architecture

```tsx
<App>
  <TerminalSizeContext.Provider value={terminalSize}>
    <FullscreenLayout
      scrollable={
        <VirtualMessageList scrollRef={scrollRef}>
          <Messages messages={presentationState.timeline}>
            {presentationState.timeline.map(msg => (
              <MessageRow key={msg.id} message={msg} />
            ))}
          </Messages>
        </VirtualMessageList>
      }
      stickyPrompt={presentationState.stickyPrompt}
      newMessageCount={presentationState.unseenCount}
      bottom={
        <Box flexDirection="column">
          <PromptInput
            value={presentationState.promptBuffer}
            onChange={presentationState.setPromptBuffer}
            onSubmit={presentationState.submitPrompt}
          />
          <StatusLine
            model={presentationState.model}
            tokens={presentationState.tokens}
            cost={presentationState.cost}
          />
        </Box>
      }
      modal={
        presentationState.activeModal ? (
          <CommandPalette modal={presentationState.activeModal} />
        ) : null
      }
    />
  </TerminalSizeContext.Provider>
</App>
```

---

## 5. PresentationState Contract Schema

The frontend components consume a pure `PresentationState` object. It contains **zero dependencies** on SQLite, `brain-storage`, `brain-domain`, or UDS sockets:

```typescript
export interface PresentationState {
  session: {
    id: string;
    title: string;
    workingDir: string;
  };
  header: {
    visible: boolean;
    title: string;
  };
  timeline: PresentationMessage[];
  streaming: {
    isStreaming: boolean;
    activeText: string;
  };
  thinking: {
    isThinking: boolean;
    durationMs: number;
    text: string;
    isExpanded: boolean;
  };
  tools: PresentationToolCall[];
  prompt: {
    buffer: string;
    cursorOffset: number;
    historyIndex: number | null;
  };
  scroll: {
    followTail: boolean;
    unseenCount: number;
    stickyPromptText: string | null;
  };
  overlays: {
    activeModal: 'commandPalette' | 'slashCompletion' | 'shortcutsHelp' | null;
    searchQuery: string;
  };
  footer: {
    modelName: string; // Temporary Claude parity (/model)
    effortTier: string; // Temporary Claude parity (/effort)
    totalTokens: number; // Temporary Claude parity
    totalCostUsd: number; // Temporary Claude parity
  };
  connection: {
    status: 'connected' | 'connecting' | 'disconnected';
  };
}
```

---

## 6. Execution of 25 Deterministic Mock Fixtures

Phase 0 implements the 25 deterministic fixtures using mock `PresentationState` data trees passed directly into the React/Ink component root. The frontend runs completely standalone without requiring a background daemon:

```text
Fixture JSON/TS
      ↓
PresentationState Mock Factory
      ↓
React Root <App state={mockState} />
      ↓
Ink + Yoga Flexbox Layout Engine
      ↓
Terminal Screen Render (fd 1)
```

---

## 7. Ink / Yoga Authoritative Layout & Bug Elimination

### Elimination of Split Geometry Bugs:
In the legacy Ratatui architecture, height math was split between `state.rs`, `renderer.rs`, `prompt.rs`, and `scroll.rs`, leading to content requiring a resize event to appear.

Under **React + Ink + Yoga**:
- **Single Source of Truth**: Yoga flexbox layout solves all child dimensions (`flexGrow`, `flexShrink`, `height`, `width`) in a single pass before frame emission.
- **Immediate First Render**: For all 25 fixtures, content renders immediately on frame 1 without waiting for a `SIGWINCH` event.
- **Tested Render States**:
  1. Render immediately
  2. Render after content insertion
  3. Render after streaming
  4. Render after scrolling
  5. Render after expansion
  6. Render after resize

---

## 8. Viewport Matrix & Visual Comparison Procedure

Visual and interaction parity will be verified across the canonical viewport matrix:
- **80x24** (Standard Terminal)
- **69x24** (Narrow Sidebar Viewport)
- **70x40** (Medium Porting Viewport)
- **100x26** (Wide Terminal)
- **120x30** (Full Screen Viewport)
- **120x40** (Large Monitor Viewport)
- **182x53** (Ultrawide Viewport)

---

## 9. Next Phase Roadmap

```text
Phase 0 (Current)
  Reconstruct React/Ink Frontend + 25 Fixtures Standalone
        │
        ▼
Phase 1: Brain PresentationAdapter
  Translate UDS StreamEvents & Domain Models into PresentationState
        │
        ▼
Phase 2: Brainification & Feature Pruning
  Connect Graph Retrieval / Sessions & Prune /model, /effort, billing counters
```
