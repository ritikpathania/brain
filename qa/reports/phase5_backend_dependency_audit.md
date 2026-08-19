# Phase 5.8 — Remaining Backend Dependency Audit Report

**Document Version**: 1.0.0  
**Status**: ARCHITECTURAL AUDIT & SEAM SPECIFICATION  
**Governing Invariants**:
1. `vendor/claude/` is **100% frozen & immutable** (1,925/1,925 files SHA-256 identical).
2. `QueryDeps.callModel` is the **exclusive, frozen model-generation boundary**.
3. Claude presentation, orchestration, tool execution, permissions, and stream reducers remain **100% Claude-owned**.
4. Brain provides model intelligence, reasoning streams, and relational knowledge memory **behind the seam**.

---

## 1. Executive Summary

Phases 5.1 through 5.7 proved that an unmodified Claude runtime can successfully drive a real Brain daemon over a live Unix Domain Socket through the single `QueryDeps.callModel` seam.

Before proceeding to any further integration, Phase 5.8 audits every remaining backend subsystem in `vendor/claude/services/`, `vendor/claude/utils/`, and `vendor/claude/commands/`.

### Key Conclusions

```text
┌────────────────────────────────────────────────────────────────────────┐
│ CLAUDE PRESENTATION & ORCHESTRATION LAYER (KEEP)                       │
│ • PromptInput & Interactive Terminal Composer (Ink / React)            │
│ • REPL & queryLoop Turn Management (query.ts)                          │
│ • Tool Schemas, Permissions (canUseTool), & Local Execution            │
│ • Microcompaction (In-memory string cleanup for old tool outputs)      │
│ • Autocompaction Threshold & Window Math                               │
│ • Local Session Replay Transcripts (~/.claude/projects/*.jsonl)       │
│ • User Preferences & Local Settings (~/.claude/settings.json)         │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    │ QueryDeps.callModel (FROZEN SEAM)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ BRAIN ADAPTER & UDS TRANSPORT LAYER (ADAPT)                            │
│ • brainCallModel: Normalization of Messages, Tools, & Thinking         │
│ • UdsBrainBackendClient: Unix Domain Socket line-delimited transport   │
│ • Deterministic Disconnect Handling (Zero auto-reconnect)              │
│ • Clean Cancellation via AbortSignal                                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    │ Live UDS (/tmp/brain.sock)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ BRAIN RELATIONAL ENGINE LAYER (BRAIN OWNED)                            │
│ • Relational Knowledge Graph (crates/brain-domain)                     │
│ • Persistent Entity, Relation, & Semantic Indexing                     │
│ • Model Execution & Reasoning Stream Generation                        │
│ • Asynchronous Memory Observation & Ingest                             │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. In-Depth Subsystem Audits

---

### 2.1 Context Compaction Subsystem

Claude's context management is composed of four distinct layers in `vendor/claude/services/compact/` and `vendor/claude/query/deps.ts`:

```text
Message History Accumulation
             │
             ├── 1. Microcompaction ──────────> [KEEP: Native Claude]
             │      (In-memory string replacement of old tool results)
             │
             ├── 2. Token Accounting ─────────> [KEEP: Native Claude]
             │      (getAutoCompactThreshold / calculateTokenWarningState)
             │
             ├── 3. Compaction Triggering ────> [KEEP: Native Claude]
             │      (queryLoop evaluates if tokenUsage >= autocompactThreshold)
             │
             └── 4. Summarization Turn ───────> [ADAPT: via QueryDeps.callModel]
                    (compactConversation forks sub-query to generate summary)
```

#### Detailed Layer Breakdown:

| Layer | Implementation File | Model Dependent? | Ownership | Action |
| :--- | :--- | :---: | :---: | :---: |
| **Microcompaction** | `services/compact/microCompact.ts` | ❌ No | Claude | **`KEEP`** |
| **Token Estimation** | `utils/tokens.ts`, `services/tokenEstimation.ts` | ❌ No | Claude | **`KEEP`** |
| **Threshold Math** | `services/compact/autoCompact.ts` | ❌ No | Claude | **`KEEP`** |
| **Compaction Trigger** | `vendor/claude/query.ts` | ❌ No | Claude | **`KEEP`** |
| **Summary Generation** | `services/compact/compact.ts` | ✅ Yes | Brain via `callModel` | **`REPLACE backend call`** |
| **Summary Insertion** | `services/compact/compact.ts` | ❌ No | Claude | **`KEEP`** |

#### Architectural Rule:
> **Do not create an independent compaction seam.** `compactConversation()` delegates its summarization model turn directly through `QueryDeps.callModel()`. By keeping Claude's native compaction orchestration and delegating the model turn to `brainCallModel`, compaction works out of the box with zero custom compaction code.

---

### 2.2 Session Persistence vs. Brain Relational Memory

There is a fundamental architectural distinction between **Claude Session Transcripts** and **Brain Relational Memory**:

```text
                              Interactive Turn
                                     │
                 ┌───────────────────┴───────────────────┐
                 ▼                                       ▼
       Claude Session Transcript                  Brain Memory Ingest
         JSONL (~/.claude/projects/)             (Asynchronous Observer)
                 │                                       │
                 ▼                                       ▼
         /resume /history UI                     Knowledge Graph / RocksDB
         Terminal Replay                         Entities & Relations
```

#### Detailed Comparison:

| Attribute | Claude Session Transcript | Brain Relational Memory |
| :--- | :--- | :--- |
| **Primary Purpose** | Interactive turn replay, `/resume`, scrollback buffer | Long-term knowledge graph, cross-session consolidation |
| **Storage Engine** | Append-only `.jsonl` files on disk | Embedded Key-Value / Vector / Graph Storage |
| **Data Model** | Raw UI `Message[]` (`user`, `assistant`, `tool_result`) | DDD Entities (`Node`, `Edge`, `Memory`, `Concept`) |
| **Upstream Coupling** | High (`vendor/claude/utils/sessionStorage.ts`) | Zero (Isolated within `crates/brain-domain/`) |
| **Source of Truth** | **Authoritative for Claude UI history** | **Authoritative for Domain Intelligence** |

#### Architectural Rule:
> **Brain must never become the source of truth for Claude's interactive session transcript.** Claude continues writing local `.jsonl` files to preserve native `/resume`, `/history`, and scrollback rehydration. Brain services asynchronously observe completed conversation turns to update the relational knowledge graph.

---

### 2.3 Authentication & Authorization Subsystem

Claude's upstream auth infrastructure in `vendor/claude/utils/auth.ts` and `vendor/claude/services/oauth/` was designed exclusively for Anthropic cloud APIs.

```text
Claude CLI
    │
    │ QueryDeps.callModel()
    ▼
brainCallModel
    │
    │ Local UDS (/tmp/brain.sock)
    ▼
Brain Daemon
    │
    ├── Local OS permissions (file permissions on socket)
    └── Model & provider credentials (Brain internal)
```

#### Detailed Consumer Audit:

| Auth Consumer | Upstream Location | Cloud Dependency | Status under Brain Host |
| :--- | :--- | :---: | :--- |
| **Anthropic API Client** | `services/api/client.ts` | `ANTHROPIC_API_KEY` | **Bypassed completely** via `QueryDeps.callModel` |
| **OAuth Code Listener** | `services/oauth/auth-code-listener.ts` | Anthropic OAuth | **Bypassed / Not invoked** |
| **Keychain Prefetch** | `utils/secureStorage/keychainPrefetch.ts` | macOS Keychain | **Inert** (No API keys requested) |
| **`/login` Command** | `commands/login.ts` | Anthropic Web Auth | **No-op / Display Brain Local Status** |
| **Subscription Quotas** | `services/claudeAiLimits.ts` | Claude.ai Rate Limits | **Inert** (Rate limits managed locally by Brain) |

#### Architectural Rule:
> **Anthropic cloud authentication is completely bypassed.** The shell connects to the local Brain daemon over UDS. Authorization is governed by local Unix socket file permissions and Brain daemon configuration.

---

### 2.4 User Settings & Client Preferences

Claude stores user preferences in `~/.claude/settings.json` via `vendor/claude/utils/settings/settings.ts`.

#### Detailed Settings Classification:

| Setting Key | Purpose | Layer | Classification |
| :--- | :--- | :---: | :---: |
| `theme` | UI color scheme (`dark`, `light`, `auto`) | UI / Ink | **`KEEP`** |
| `verbose` | Extended debug logging in terminal | UI / Logging | **`KEEP`** |
| `editor` | Preferred terminal editor (`vim`, `nano`, `code`) | Local Tool | **`KEEP`** |
| `autoUpdates` | Auto-update flag (disabled by env) | Upstream | **`REMOVE` (Disabled)** |
| `permissions` | Tool allow/deny rules (`canUseTool`) | Execution | **`KEEP`** |
| `outputStyle` | Terminal layout & line formatting | UI / Ink | **`KEEP`** |

#### Architectural Rule:
> **All client settings in `settings.json` remain active and Claude-owned.** They are local client preferences with zero cloud or backend dependencies.

---

### 2.5 Telemetry, Analytics & Auto-Updater

| Subsystem | Upstream Location | Default Behavior | Brain Shell Strategy |
| :--- | :--- | :--- | :---: |
| **Event Logging** | `services/analytics/index.ts` | Queues events until sink is attached | **`REMOVE`** (No sink attached; zero outbound telemetry) |
| **GrowthBook Feature Flags** | `services/analytics/growthbook.ts` | Fetches remote experiments | **`REMOVE`** (Hardcoded build-time flags in `preload.ts`) |
| **Auto-Updater** | `cli/update.ts` | Checks npm/anthropic registry | **`REMOVE`** (`DISABLE_AUTOUPDATER=1` enforced) |

---

## 3. Exhaustive Service Classification Matrix

| Service Path | Functionality | CallModel Cross? | Decision | Rationale |
| :--- | :--- | :---: | :---: | :--- |
| `services/api/claude.ts` | Anthropic API streaming | Yes | **`REPLACE`** | Replaced by `brainCallModel` via `QueryDeps.callModel` |
| `services/compact/microCompact.ts` | Trims old tool results | No | **`KEEP`** | Pure in-memory string trimming on message objects |
| `services/compact/autoCompact.ts` | Context threshold calculation | No | **`KEEP`** | Evaluates token counts against model window |
| `services/compact/compact.ts` | Compaction orchestration & summary | Yes | **`ADAPT`** | Orchestration kept; summary model call routes via `callModel` |
| `services/oauth/*` | Anthropic OAuth login flows | No | **`REMOVE`** | Cloud auth bypassed by local UDS transport |
| `services/analytics/*` | Datadog / 1P event logging | No | **`REMOVE`** | Sinks unattached; silent in-memory queue |
| `services/tools/*` | Tool execution & permissions | No | **`KEEP`** | Claude retains 100% authoritative tool lifecycle |
| `utils/sessionStorage.ts` | JSONL transcript logging | No | **`KEEP`** | Powers `/resume`, `/history`, and terminal scrollback |
| `utils/sessionRestore.ts` | Reconstructs session from log | No | **`KEEP`** | Powers instant session rehydration |
| `utils/settings/settings.ts` | Client preferences management | No | **`KEEP`** | Pure local client settings (`~/.claude/settings.json`) |
| `services/claudeAiLimits.ts` | Anthropic subscription limits | No | **`REMOVE`** | Rate limits handled by local Brain engine |
| `services/PromptSuggestion/*` | Prompt autocompletion hints | No | **`KEEP`** | Local client heuristics |
| `services/SessionMemory/*` | Upstream text session memory | Yes | **`REPLACE`** | Brain Relational Knowledge Graph supersedes text memory |

---

## 4. Final Recommendation & Implementation Roadmap

With the CallModel seam **frozen** and all backend dependencies audited:

1. **Phase 5.9 — End-to-End Compaction Verification**:
   - Exercise `autoCompactIfNeeded` triggering under high token volume.
   - Verify summary message creation through `brainCallModel`.
   - Confirm in-memory message history compression without vendor edits.
2. **Phase 5.10 — Session Replay & Asynchronous Knowledge Ingestion**:
   - Verify `/resume` against local `.jsonl` transcripts.
   - Verify asynchronous turn observation into `crates/brain-domain` knowledge graph.
3. **Phase 5.11 — Final Differential Parity & Workspace Certification**:
   - Run full 66+ test suite + PTY interactive suite + Gate A vendor integrity check.
