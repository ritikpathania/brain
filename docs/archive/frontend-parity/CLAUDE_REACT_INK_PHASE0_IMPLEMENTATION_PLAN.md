# Implementation Plan — Phase 0 Claude React/Ink Frontend Reconstruction

> **Document Status**: Authoritative Phase 0 Implementation Plan  
> **Target Package**: `packages/brain-frontend` / Presentation Stack (`React + Ink + Yoga`)  
> **Core Strategy**: **Claude First, Brain After** (Full Standalone React/Ink Reconstruction before Brain Adapter Integration)  
> **Authoritative Baseline Reference**: [`docs/design/CLAUDE_REACT_INK_FRONTEND_ARCHITECTURE_SPEC.md`](CLAUDE_REACT_INK_FRONTEND_ARCHITECTURE_SPEC.md)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-13  

---

```text
==================================================
PHASE 0 EXECUTION ORDER
==================================================
1. Setup React/Ink Runtime Stack (Node.js / Bun + TypeScript + Ink v5 + Yoga)
2. Create PresentationState Schema & 25 Mock Fixture Generators
3. Build Component Architecture (FullscreenLayout, Messages, BaseTextInput, StatusLine)
4. Implement Single Yoga Layout Hierarchy (Zero split geometry math)
5. Implement 25 Fixture Scenarios & Interactive Behavior Matrix
6. Perform Mechanical Viewport & Visual Parity Verification (80x24, 69x24, 70x40, 100x26, 120x30, 120x40, 182x53)
```

---

## 1. Repository & Component Source Mapping

The component mapping maps every visual surface in the Claude Source Oracle (`/Users/ritikpathania/Developer/src/**`) to the standalone React/Ink frontend implementation:

| Surface Component | Claude Source Oracle Path | Brain Frontend Implementation | Implementation Method |
| :--- | :--- | :--- | :--- |
| **App Root & Terminal Size** | `/Users/ritikpathania/Developer/src/ink/ink.tsx` | `src/App.tsx` | Clean-Room React/Ink Container |
| **Fullscreen Layout Shell** | `/Users/ritikpathania/Developer/src/components/FullscreenLayout.tsx` | `src/components/FullscreenLayout.tsx` | Direct Component Structure Reconstruction |
| **Virtual Scroll Box** | `/Users/ritikpathania/Developer/src/ink/components/ScrollBox.tsx` | `src/components/ScrollBox.tsx` | Ink ScrollBox Integration |
| **Message List & Divider** | `/Users/ritikpathania/Developer/src/components/Messages.tsx` | `src/components/Messages.tsx` | Clean-Room Timeline Component |
| **Message Row Dispatcher** | `/Users/ritikpathania/Developer/src/components/MessageRow.tsx` | `src/components/MessageRow.tsx` | Row Type Dispatcher |
| **Assistant Text Response** | `/Users/ritikpathania/Developer/src/components/messages/AssistantTextMessage.tsx` | `src/components/messages/AssistantTextMessage.tsx` | Markdown / Text Formatter |
| **Thinking Block Header & Drawer** | `/Users/ritikpathania/Developer/src/components/messages/AssistantThinkingMessage.tsx` | `src/components/messages/AssistantThinkingMessage.tsx` | Indented Drawer with `Ctrl+O` Toggle |
| **Tool Execution Cards** | `/Users/ritikpathania/Developer/src/components/messages/AssistantToolUseMessage.tsx` | `src/components/messages/AssistantToolUseMessage.tsx` | 5 Lifecycle States (`Pending`, `Running`, `Completed`, `Failed`, `Denied`) |
| **Tool Output Result Drawer** | `/Users/ritikpathania/Developer/src/components/messages/UserToolResultMessage/index.tsx` | `src/components/messages/UserToolResultMessage.tsx` | 20-line Drawer Cap & Line Numbers |
| **User Prompt Row** | `/Users/ritikpathania/Developer/src/components/messages/UserTextMessage.tsx` | `src/components/messages/UserTextMessage.tsx` | Left-aligned Prompt `❯ <text>` |
| **Base Text Input / Editor** | `/Users/ritikpathania/Developer/src/components/BaseTextInput.tsx` | `src/components/BaseTextInput.tsx` | Multiline visual cursor, Home/End, Ctrl+A/E/K/Y |
| **Status & Token Footer Bar** | `/Users/ritikpathania/Developer/src/components/StatusLine.tsx` | `src/components/StatusLine.tsx` | Model badge, token/cost counters, status hints |
| **Command Palette Modal** | `/Users/ritikpathania/Developer/src/components/GlobalSearchDialog.tsx` | `src/components/GlobalSearchDialog.tsx` | `Ctrl+K` Overlay Modal Slot |
| **Slash Completion Popup** | `/Users/ritikpathania/Developer/src/components/PromptInput/PromptInputFooterSuggestions.tsx` | `src/components/PromptInputFooterSuggestions.tsx` | `/` Menu Popup Component |
| **Shortcuts Help Modal** | `/Users/ritikpathania/Developer/src/components/HelpV2/index.tsx` | `src/components/ShortcutsHelpModal.tsx` | `?` / `F1` Keybinding Reference Modal |

---

## 2. Dependency & Runtime Plan

- **Node.js / Bun Runtime**: Standard TypeScript execution environment supporting ESM modules.
- **Dependencies**: `react`, `ink`, `yoga-layout`, `ink-text-input`, `chalk`, `figures`, `zustand` (or simple React Context for `PresentationState`).
- **Isolation Boundary**: The standalone React/Ink frontend will reside in `sdks/brain-frontend` or `frontend/` with zero native Rust dependencies, communicating strictly via `PresentationState` fixtures in Phase 0.

---

## 3. Source Reuse & Licensing Assessment

- **Source Trace Oracle**: `/Users/ritikpathania/Developer/src/**` provides authoritative visual, structural, and behavioral specifications.
- **Licensing Strategy**: All React/Ink components are implemented as **Clean-Room TypeScript/React code** matching the component boundaries, props interfaces, and layout geometry of the source oracle. Zero proprietary Anthropic API tokens or credentials required.

---

## 4. Test & Fixture Strategy

### A. 25 Deterministic Mock PresentationState Fixtures
Every scenario (`empty_landing`, `single_user_query`, `short_assistant_response`, `long_assistant_response`, `multiline_user_prompt`, `active_streaming_response`, `thinking_block_active`, `thinking_block_expanded`, `tool_execution_pending`, `tool_execution_running`, `tool_execution_completed`, `tool_execution_failed`, `tool_drawer_expanded`, `scrolled_above_tail`, `new_messages_pill_visible`, `sticky_prompt_header_active`, `command_palette_open`, `slash_completion_open`, `shortcuts_help_open`, `viewport_narrow_69x24`, `viewport_wide_182x53`, `viewport_small_height_12`, `combined_thinking_tool_stream`, `loading_connecting_state`, `error_banner_active`) will be instantiated as a static fixture factory.

### B. Viewport Matrix Verification Procedure
Each fixture will be evaluated against:
1. `80x24`
2. `69x24`
3. `70x40`
4. `100x26`
5. `120x30`
6. `120x40`
7. `182x53`

### C. State & Rendering Verification Checklist:
- **First-Frame Rendering**: Verify frame 1 renders correctly without needing a terminal resize (`SIGWINCH`).
- **Content Insertion & Streaming**: Verify typewriter streaming appends smoothly.
- **Drawer Collapsing & Expansion**: Verify `Ctrl+O` / `Alt+T` toggles drawers without scroll-anchor drift.
- **Sticky Header & New-Messages Pill**: Verify header occupies top row (`y = chat_area.y`) and pill occupies bottom row (`y = chat_area.y + height - 1`) with zero overlap.

---

## 5. Migration Boundary Statement

During Phase 0, the React/Ink frontend executes completely standalone:
```text
Mock Fixture → PresentationState → React Component Tree → Ink Engine → Terminal Output
```
Once Phase 0 passes mechanical verification across all 25 fixtures and 7 viewports, Phase 1 will connect `Brain FrontendAdapter` to map live UDS backend events into the exact same `PresentationState` schema.
