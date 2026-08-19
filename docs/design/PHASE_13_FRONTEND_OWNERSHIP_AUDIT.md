# PHASE 13.1 — Frontend Ownership Forensic Audit & Migration Blueprint

> **Document Status**: CERTIFIED AUDIT REPORT  
> **Phase**: 13.1 (Strict Read-Only Audit & Gap Analysis)  
> **Objective**: Reinstating `packages/brain-frontend` (React + Ink + Yoga) as the Sole Production Frontend & Preparing `crates/brain-tui` (Ratatui) for Retirement  
> **Backend Status**: FROZEN (0 backend mutations permitted)  
> **Execution Constraint**: AUDIT ONLY — ZERO SOURCE CODE MUTATIONS  

---

## Executive Summary

This forensic audit establishes the mechanical state of the Brain repository regarding frontend ownership, dependencies, launch paths, protocol compatibility, and migration requirements.

```text
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 FRONTEND MIGRATION TOPOLOGY                                      │
├───────────────────────────────────────────────────┬──────────────────────────────────────────────┤
│ Current State (Pre-Migration)                     │ Target State (Post-Migration)                │
├───────────────────────────────────────────────────┼──────────────────────────────────────────────┤
│ Production Frontend:                              │ Sole Production Frontend:                    │
│   crates/brain-tui (Rust / Ratatui)               │   packages/brain-frontend (React / Ink / Yoga)│
│                                                   │                                              │
│ Candidate Frontend (Previously Staged):           │ Retired Infrastructure:                      │
│   packages/brain-frontend (TypeScript / Ink)      │   crates/brain-tui (DELETED in Phase 13.4)   │
│                                                   │                                              │
│ Backend Dependency:                               │ Backend Dependency:                          │
│   Production UDS Protocol (~/.brain/daemon.sock)  │   Production UDS Protocol (UNTOUCHED)        │
│                                                   │                                              │
│ Domain, Core, Storage, Services, Events:          │ Domain, Core, Storage, Services, Events:     │
│   FROZEN & PRESERVED                              │   FROZEN & PRESERVED                         │
└───────────────────────────────────────────────────┴──────────────────────────────────────────────┘
```

---

## Section A: React / Ink Frontend Deep Audit (`packages/brain-frontend/**`)

Mechanical inspection of `packages/brain-frontend/` establishes that a fully functional React + Ink + Yoga frontend is already implemented, possessing 107 passing tests and a comprehensive architecture.

### 1. Package Manifest & Runtime
- **Location**: `packages/brain-frontend/package.json`
- **Name**: `@brain/frontend` (v1.0.0, private)
- **Module Entrypoint**: `src/main.tsx`
- **Runtime & Package Manager**: `bun` (supports Node.js ESM modules)
- **Package Scripts**:
  - `start`: `bun run src/main.tsx`
  - `fixture`: `bun run src/cli.tsx`
  - `test`: `bun test`
- **Runtime Dependencies**:
  - `react`: `^18.3.1`
  - `ink`: `^5.1.0`
  - `chalk`: `^5.3.0`
  - `figures`: `^6.1.0`
- **Dev Dependencies**:
  - `typescript`: `^5.5.4`
  - `@types/react`: `^18.3.3`

### 2. Component Tree & Layout Engine
The UI is constructed using React terminal components laid out via Yoga flexbox (`FullscreenLayout.tsx`):

```text
InteractiveApp (src/main.tsx)
  │
  ▼
App (src/App.tsx)
  │
  ▼
FullscreenLayout (src/components/FullscreenLayout.tsx)
  ├── Header Slot (1 row, engine title, connection dot)
  ├── Divider Line (Subtle rule)
  ├── Sticky Prompt Slot (shown when prompt scrolls off-screen)
  ├── Scrollable Timeline Container (src/components/Messages.tsx)
  │     ├── Logo / Welcome Greeting (src/components/WelcomeHero.tsx)
  │     └── MessageRow List (src/components/MessageRow.tsx)
  │           ├── UserTextMessage (❯ prompt text)
  │           ├── AssistantThinkingMessage (reasoning trace, Ctrl+O toggle)
  │           ├── AssistantToolUseMessage (1-line card, approval prompt)
  │           ├── UserToolResultMessage (indented 20-line capped output)
  │           └── AssistantTextMessage (MarkdownText with code fences)
  ├── Fixed Bottom Region (Pinned prompt footer)
  │     ├── BaseTextInput (Multiline auto-wrapping prompt composer)
  │     └── StatusLine (Version, daemon status, memory state)
  └── Absolute Modal Overlay Slot (GlobalSearchDialog / ShortcutsHelpModal)
```

### 3. State Management & Typed Adapters
- **Presentation State**: `PresentationState` interface (`src/types/presentation.ts`) models timeline messages, streaming state, prompt buffer, overlay visibility, scroll position, and footer metrics.
- **`BrainFrontendAdapter` (`src/adapter/BrainFrontendAdapter.ts`)**:
  - Ingests normalized `BrainStreamEvent` variants (`Token`, `Progress`, `Stage`, `Finished`, `Cancelled`, `ToolCallRequest`, `ToolCallResult`, etc.).
  - Ingests raw JSON wire messages from UDS (`stream_start`, `stream_progress`, `stream_chunk`, `stream_end`, `stream_cancelled`, `v1/subscribe`).
  - Enforces strict isolation: presentation components consume only strongly-typed state objects.
- **`BrainFrontendController` (`src/adapter/BrainFrontendController.ts`)**:
  - Manages prompt submissions, slash command routing (`/help`, `/status`, `/sessions`, `/reflect`, `/compile`, `/inspect`, etc.).
  - Handles interactive tool approval decisions (`y` / `n`).
  - Manages session restoration (`v1/sessions/get`) and telemetry subscriptions.

### 4. Unix Domain Socket Client (`src/uds/BrainUdsClient.ts`)
- Implements Node/Bun `net.Socket` connection to `~/.brain/daemon.sock` (configurable via `BRAIN_SOCKET_PATH`).
- Implements newline-delimited JSON Lines framing with streaming chunk buffering (`handleData`).
- Implements one-shot query dispatch (`requestOneShot`) for non-streaming RPCs (`list_sessions`, `v1/search`, `v1/status`, `v1/metrics`, `v1/projections`, etc.).
- Implements automatic exponential backoff reconnection.

### 5. Test Suite Verification
- **Test Command**: `bun test` in `packages/brain-frontend/`
- **Result**: **107 passed / 0 failed / 387 assertions in 140ms**
  - `adapter.test.ts`: 100% pass
  - `controller.test.ts`: 100% pass
  - `conversationRendering.test.ts`: 100% pass
  - `fixtures.test.ts`: 100% pass
  - `mainRuntime.test.ts`: 100% pass
  - `productionPathAudit.test.ts`: 100% pass
  - `sessionPersistence.test.ts`: 100% pass
  - `udsClient.test.ts`: 100% pass

---

## Section B: Legacy Rust / Ratatui Frontend Deep Audit (`crates/brain-tui/**`)

### 1. Crate Membership & Workspace Position
- **Cargo Path**: `crates/brain-tui/Cargo.toml`
- **Workspace Membership**: Registered in root `Cargo.toml` (`[workspace.members]`).
- **Crate Type**: Library crate (`lib.rs`) providing `brain_tui::run()` and `brain_tui::client::UdsClient`.
- **Dependencies**: `ratatui 0.28`, `crossterm 0.28`, `tokio`, `serde`, `unicode-width`, `fuzzy-matcher`.

### 2. Consumers of `brain-tui` in the Repository
1. `apps/brain/Cargo.toml`: Declares dependency `brain-tui = { path = "../../crates/brain-tui" }`.
2. `apps/brain/src/host.rs`:
   - `CLIHost::run_tui()` invokes `brain_tui::run(client)`.
   - `CLIHost::run_query()` uses `brain_tui::client::UdsClient`.
3. `crates/brain-arch-tests/tests/dependency_boundaries.rs`: Verifies architecture layer boundaries including `brain-tui`.
4. `crates/brain-fitness-tests`: Verifies layer rules.
5. `scripts/ux_audit/run.sh` & `xtask/src/main.rs`: Developer maintenance scripts.

### 3. Critical Invariant Verified: Zero Backend Dependencies on `brain-tui`
- `crates/brain-domain`: **0 dependencies** on `brain-tui` or `ratatui`.
- `crates/brain-core`: **0 dependencies** on `brain-tui` or `ratatui`.
- `crates/brain-storage`: **0 dependencies** on `brain-tui` or `ratatui`.
- `crates/brain-services`: **0 dependencies** on `brain-tui` or `ratatui`.
- `crates/brain-events`: **0 dependencies** on `brain-tui` or `ratatui`.
- `daemon`: **0 dependencies** on `brain-tui` or `ratatui`.

**Conclusion**: `crates/brain-tui` is a pure presentation leaf. Removing it will not impact any backend subsystems.

---

## Section C: Repository-Wide Dependency & Reference Graph

Mechanical regex search across all files (excluding `.git`, `target`, `node_modules`) revealed the following classification:

```text
┌────────────────────────────┬─────────────┬──────────────┬───────────────────────────────────────────────────────┐
│ Target Pattern             │ File Count  │ Occurrences  │ Classification Breakdown                              │
├────────────────────────────┼─────────────┼──────────────┼───────────────────────────────────────────────────────┤
│ \bbrain-tui\b              │ 156 files   │ 23,230 hits  │ Build: 3 (Cargo.toml, apps/brain, crates/brain-tui)  │
│                            │             │              │ Source: 23 (apps/brain/src/host.rs, etc.)             │
│                            │             │              │ Tests: 9 (arch/fitness tests)                         │
│                            │             │              │ Documentation / Historical: 121 (ADRs, reports)       │
├────────────────────────────┼─────────────┼──────────────┼───────────────────────────────────────────────────────┤
│ \bratatui\b                │ 254 files   │ 2,386 hits   │ Build: 1 (crates/brain-tui/Cargo.toml)                │
│                            │             │              │ Source/Widgets: 95 (crates/brain-tui/src/**)          │
│                            │             │              │ Tests: 68 (crates/brain-tui/tests/**)                 │
│                            │             │              │ Documentation / Historical: 90 (Architecture docs)    │
├────────────────────────────┼─────────────┼──────────────┼───────────────────────────────────────────────────────┤
│ \bpackages/brain-frontend\b│ 35 files    │ 115 hits     │ Active Package: packages/brain-frontend/**            │
│                            │             │              │ Documentation: docs/archive/frontend-parity/**        │
├────────────────────────────┼─────────────┼──────────────┼───────────────────────────────────────────────────────┤
│ \bink\b (exact word)       │ 91 files    │ 451 hits     │ Active Source: packages/brain-frontend/src/**         │
│                            │             │              │ Documentation: docs/archive/frontend-parity/**        │
├────────────────────────────┼─────────────┼──────────────┼───────────────────────────────────────────────────────┤
│ \byoga\b (exact word)      │ 51 files    │ 146 hits     │ Active Source: packages/brain-frontend/**             │
│                            │             │              │ Documentation: docs/archive/frontend-parity/**        │
└────────────────────────────┴─────────────┴──────────────┴───────────────────────────────────────────────────────┘
```

---

## Section D: Production Launch Path Analysis

### 1. Current Launch Path (Rust / Ratatui)
```text
User executes: `brain` or `brain ui`
      │
      ▼
Executable: `target/debug/brain` (compiled from `apps/brain/src/main.rs`)
      │
      ▼
Entrypoint: `CLIHost::run_tui()` in `apps/brain/src/host.rs`
      │
      ▼
Frontend Engine: `brain_tui::run(client)` (Ratatui TUI render loop)
      │
      ▼
IPC Socket: `UnixStream` connection to `~/.brain/daemon.sock`
      │
      ▼
Backend: `brain-daemon` background process
```

### 2. Target Production Launch Path (React / Ink / Yoga)
```text
User executes: `brain` or `brain ui` (or `bun run @brain/frontend`)
      │
      ▼
Executable Launcher: `apps/brain` launcher or binary runner invoking `@brain/frontend`
      │
      ▼
Entrypoint: `packages/brain-frontend/src/main.tsx` (`render(<InteractiveApp />)`)
      │
      ▼
Frontend Engine: React 18 + Ink 5 + Yoga flexbox
      │
      ▼
IPC Socket: `BrainUdsClient` (Node/Bun `net.Socket` to `~/.brain/daemon.sock`)
      │
      ▼
Backend: `brain-daemon` background process (UNTOUCHED)
```

---

## Section E: Backend / UDS Compatibility & Gap Analysis

A mechanical comparison between `daemon/src/transport/uds/handlers.rs` and `packages/brain-frontend/src/uds/BrainUdsClient.ts` demonstrates:

### 1. Zero Backend Mutations Required
The production daemon wire protocol is already 100% compatible with `BrainUdsClient.ts`:
- **Query Action**: `{"action": "query", "payload": "<prompt>", "workspace_context": [...]}`
- **Streaming Events**: `stream_start`, `stream_progress`, `stream_chunk`, `stream_end`, `stream_cancelled`
- **Session Management**: `{"action": "list_sessions"}`, `{"action": "v1/sessions/get", ...}`
- **System Commands**: `v1/status`, `v1/metrics`, `v1/projections`, `v1/reflect`, `v1/compile`, `v1/inspect_node`, `v1/search`

### 2. Exact Gaps in `packages/brain-frontend` to Complete in Phase 13.2

While `packages/brain-frontend` is functional and tested, the following specific visual and interaction gaps must be resolved to achieve full compliance with the frozen Claude Visual Contract:

1. **Theme Token Parity (`tokens.ts`)**:
   - Update generic named colors (`accent: 'cyan'`, `brandGold: 'yellow'`) to the source-grounded Claude palette:
     - `claude`: `#D77757` (`rgb(215, 119, 87)`)
     - `promptBorder`: `#888888` (`rgb(136, 136, 136)`)
     - `subtle`: `#505050` (`rgb(80, 80, 80)`)
     - `permission`: `#B1B9F9` (`rgb(177, 185, 249)`)
     - `autoAccept`: `#AF87FF` (`rgb(175, 135, 255)`)
     - `userMessageBackground`: `#1E1E1E` (`rgb(30, 30, 30)`)
2. **Scrollable `LogoHeader` Parity (`WelcomeHero.tsx` / `Messages.tsx`)**:
   - Replace the standalone `WelcomeHero` with the Claude-grounded `LogoHeader` containing the terracotta `Clawd` avatar (`▐▛███▜▌`) anchored at `y=0/1` of the scrollable transcript.
   - Enforce the 70-column responsive breakpoint (`LOGO_BREAKPOINT = 70`): compact header when `< 70` cols, full two-panel split when $\ge 70$ cols (`LEFT_PANEL_MAX_WIDTH = 50`).
   - Ensure the header naturally scrolls away as messages grow in the transcript.
3. **Relational Memory Provenance Chips (`MessageRow.tsx`)**:
   - Ingest `StreamEventKind::WorkspaceContextUsed` and `context_used` metadata in `stream_end`.
   - Render inline collapsible chip `⟡ Recalled 4 memories (Ctrl+M to inspect)` above assistant messages.
4. **Prompt Composer Floating Slash Autocomplete Popup**:
   - Add floating autocomplete suggestion box directly above the input prompt when typing `/` commands, separate from the centered `Ctrl+K` command palette modal.
5. **Launcher Bridge in `apps/brain`**:
   - Update `apps/brain/src/host.rs` to launch `@brain/frontend` via Bun/Node process invocation or provide a direct executable entrypoint.

---

## Section F: Files to be Removed vs Retained

### Files to be Removed in Phase 13.4 (After Cutover Certification)
- Entire directory: `crates/brain-tui/**` (all Rust TUI code, widgets, view models, and 85+ tests)
- `apps/brain/src/host.rs` (Ratatui launch logic replaced with frontend subprocess launcher)
- Root `Cargo.toml`: Remove `"crates/brain-tui"` from `[workspace.members]`
- `apps/brain/Cargo.toml`: Remove `brain-tui` dependency

### Files to be Retained (Permanent Core Assets)
- `crates/brain-domain/**`: 100% retained (Core DDD entities, aggregates, events)
- `crates/brain-core/**`: 100% retained (Streaming events, traits, errors)
- `crates/brain-storage/**`: 100% retained (SQLite, DuckDB, RocksDB, Vector DB)
- `crates/brain-services/**`: 100% retained (Hybrid retrieval, reflection, compilation)
- `crates/brain-events/**`: 100% retained (Event bus, envelopes)
- `daemon/**`: 100% retained (Production daemon, UDS server, RPC handlers)
- `apps/brain/**`: Retained (CLI launcher, daemon management commands)
- `packages/brain-frontend/**`: Retained as sole production frontend

---

## Section G: Risk Assessment & Mitigation Plan

| Risk Description | Severity | Mitigation Strategy |
| :--- | :---: | :--- |
| **Runtime Dependency on Bun/Node** | Medium | `apps/brain` launcher will detect available JS runtimes (`bun`, `node`) with informative diagnostics or package frontend with Bun standalone executable compile. |
| **Terminal Resize / SIGWINCH Handling** | Low | Ink 5 and Yoga handle terminal layout recalculation automatically on SIGWINCH. Viewport tests will verify terminal resize bounds. |
| **Premature Deletion of Ratatui** | High | Ratatui crate is strictly preserved throughout Phase 13.1, 13.2, and 13.3. Deletion occurs **only** in Phase 13.4 after production cutover passes all verification gates. |
| **Negative Capability Drift** | High | Strictly enforce prohibition of fake cloud capabilities (model selector, `/effort`, billing UI). |

---

## Section H: Recommended Phase 13.2 Implementation Sequence

```text
Phase 13.2 Implementation Sequence:
  Step 1: Theme Token Parity Update (tokens.ts)
  Step 2: Claude LogoHeader & Responsive 70-Col Breakpoint (LogoHeader.tsx)
  Step 3: Relational Memory Provenance Chip (MemoryChip.tsx)
  Step 4: Floating Slash Autocomplete Popup (SlashPopup.tsx)
  Step 5: Production Launcher Integration (apps/brain/src/host.rs)
  Step 6: End-to-End Test Suite Execution (bun test + cargo test)
```

---

## Phase 13.1 Conclusion & Audit Certification

The forensic audit is complete.
- **Backend Invariant**: 100% preserved (zero backend mutations).
- **Frontend Architecture**: Ready for Phase 13.2 React/Ink implementation and subsequent cutover.
- **Current State**: Awaiting user approval to begin Phase 13.2.
