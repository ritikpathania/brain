# Brain → Claude Surface & Capability Mapping Specification

**Document Status**: `CANONICAL SPECIFICATION` (Capability Surface Mapping for Brain Frontend)  
**Target Architecture**: Native Rust Terminal User Interface (`crates/brain-tui` / Ratatui)  
**Authority Hierarchy**: Governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md) and [`docs/design/CLAUDE_COMPONENT_MODEL.md`](./CLAUDE_COMPONENT_MODEL.md)  
**Provenance**: Source-grounded mapping of Brain backend capabilities onto Claude presentation surfaces.

---

## 1. Executive Product Strategy

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                             VISUAL & PRODUCT CONVERGENCE MODEL                              │
├──────────────────────────────────────────────┬──────────────────────────────────────────────┤
│               CLAUDE FRONTEND                │                BRAIN BACKEND                 │
│              (Visual Contract)               │            (Capabilities & State)            │
├──────────────────────────────────────────────┼──────────────────────────────────────────────┤
│ • Clean typographic hierarchy                │ • Reciprocal Rank Fusion (RRF) Hybrid Search │
│ • Borderless conversational canvas           │ • Relational Knowledge Graph & Projections   │
│ • Collapsible thinking & tool cards          │ • Temporal Decay & Reflection Consolidation  │
│ • Multi-line auto-expanding prompt composer  │ • Local SQLite & UDS Socket IPC Streaming    │
└──────────────────────────────────────────────┴──────────────────────────────────────────────┘
```

> **Core Invariant**: Brain adopts Claude's clean, frictionless visual surfaces while preserving 100% of Brain's unique capabilities (persistent memory, graph relations, local-first privacy, deterministic ranking). Brain does NOT invent fake Claude cloud features (e.g. cloud model selectors, subscription tiers) or strip away its own features.

---

## 2. Definitive Surface & Capability Provenance Matrix

| Claude Visual Surface | Brain Capability / Concept | Backend Source Engine | Fake Cloud Feature Allowed? | Presentation Grammar in Brain UI |
| :--- | :--- | :--- | :---: | :--- |
| **Conversation Stream** | Interactive query session & dialogue | `crates/brain-services` | **No** | Rendered directly on borderless canvas floor (`Color::Reset`) with markdown syntax highlighting. |
| **Sidebar / Drawer** | Session history & workspace switcher | `crates/brain-services` (SessionManager) | **No** | Collapsible drawer (`Ctrl+S`) or command palette entry (`Ctrl+K`). |
| **Prompt Composer** | Brain natural language prompt & command input | `crates/brain-tui` | **No** | Auto-expanding rounded box (`❯`), focused border `claude` (`#D77757`). |
| **Slash Commands (`/`)** | Local management tools (`/session`, `/memory`, `/search`, `/doctor`) | Command Router (`brain-cli-adapter`) | **No** | Floating autocomplete popup anchored above prompt box when user types `/`. |
| **Command Palette (`Ctrl+K`)** | Global action dispatcher & quick navigation | Action Dispatcher | **No** | Centered floating modal overlay with fuzzy filtering. |
| **Thinking / Reasoning** | Chain-of-thought analysis & graph traversal logs | UDS Stream (`StreamEvent::Thinking`) | **No** | Inline `⠋ Thinking (X.Xs)...` spinner block; auto-collapses on completion. |
| **Tool Execution Cards** | Tool invocations (file reads, searches, memory updates) | `crates/brain-tools` (ToolExecutor) | **No** | Streamlined 1-line summary cards (`✓ Read file.rs`); expandable via `Ctrl+O`. |
| **Context / Memory Chips** | Relational memory graph expansion & recalled nodes | `crates/brain-storage` (RetrievalService) | **No** | Subtle, collapsible inline provenance chips (`⟡ Recalled 4 memories (RRF: 0.94)`). |
| **Permission Review Modal** | Tool execution security approvals (`RFC-009`) | Permission Engine | **No** | Centered themed modal with soft violet border (`permission: #B1B9F9`) for `[y/n/always]` confirmation. |
| **Status Footer** | Background daemon status & telemetry hints | Background Daemon UDS | **No** | Single-row, borderless hint line pinned at `y = height - 1`. |
| **Model Selector** | None (Configured in `brain.toml`) | Local Config Engine | **Strictly Forbidden** | **DO NOT INVENT**: No dynamic cloud model selector menus in the primary UI. |
| **Effort / Reasoning Slider** | None (Deterministic mathematical decay & RRF) | Domain Engine | **Strictly Forbidden** | **DO NOT INVENT**: No arbitrary `/effort` slider. |
| **Cloud Billing / Subscription UI** | None (100% Local-First Engine) | Local SQLite | **Strictly Forbidden** | **DO NOT INVENT**: No cloud account or subscription UI. |

---

## 3. Brain-Specific Feature Representation

### 3.1 Relational Memory Provenance
When Brain retrieves memories from its relational knowledge graph via hybrid RRF search (lexical BM25 + vector similarity + graph neighbor expansion), the retrieved context is presented to the user unobtrusively:
- **Collapsed Form**: An inline chip at the top of the assistant response:
  ```text
  ┌────────────────────────────────────────────────────────┐
  │ ⟡ Recalled 4 memories (RRF Score: 0.94)  [Ctrl+O view] │
  └────────────────────────────────────────────────────────┘
  ```
- **Expanded Form (`Ctrl+O`)**: Opens an overlay drawer displaying the recalled entity nodes, relationship predicates (`DEPENDS_ON`, `IMPLEMENTS`), and temporal decay weights.

### 3.2 Offline Diagnostics
Diagnostic health checks (`brain doctor` or `/doctor`) are presented via clean tabular markdown blocks in the conversation stream rather than permanent flashing dashboard telemetry cards, preserving visual cleanliness.

---

## 4. Interaction Workflow & Keyboard Navigation

```text
┌─────────────────────────┐
│ User Types Prompt / Cmd │
└───────────┬─────────────┘
            │
            ├──────────────► [Type '/'] ──► Floating Slash Autocomplete Popup
            │
            ├──────────────► [Ctrl+K]   ──► Global Command Palette Modal
            │
            ├──────────────► [Ctrl+S]   ──► Session History & Workspace Drawer
            │
            ├──────────────► [Ctrl+O]   ──► Expand / Collapse Active Card (Thinking / Tool)
            │
            └──────────────► [Enter]    ──► Dispatches Query & Initiates 60fps Streaming
```

---

## 5. Summary of Architecture Protections

1. **Zero Backend Regression**: All services (`RetrievalService`, `SessionService`, `ConsolidationService`) continue operating identically over UDS socket streams.
2. **Zero Fake UI Constructs**: No empty placeholders for cloud features that Brain does not support.
3. **Purity of Visual Presentation**: Complete elimination of obsolete visual noise (pixel art, heavy double boxes, neon colors) in favor of Claude's clean, modern terminal aesthetic.
