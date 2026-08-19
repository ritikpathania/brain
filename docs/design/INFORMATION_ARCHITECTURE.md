# Information Architecture Specification

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`.
> **CANONICAL DESIGN AUTHORITY**: All visual layout, screen hierarchy, and disclosure grammar are strictly governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).

---

## 1. Information Hierarchy & Screen Structure

In strict accordance with [`CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md), the user interface is structured around a **Two-Region Vertical Stack** with full-width conversation primacy:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Primary Conversation Canvas (Always Visible, flexGrow: 1)                │
│    ├── Typographic Greeting Header (Scrolls naturally with conversation)    │
│    ├── User Query Blocks                                                    │
│    ├── Assistant Responses & Code Blocks                                    │
│    ├── Collapsible Thinking Blocks (⠋ Thinking 2.4s)                       │
│    ├── Collapsible Tool Execution Cards (✓ Read 42 lines)                   │
│    └── Recalled Memory Provenance Chips (⟡ Recalled 4 memories)             │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. Pinned Bottom Region (Always Visible, flexShrink: 0)                     │
│    ├── Multi-Line Auto-Expanding Prompt Composer                            │
│    └── Single-Row Borderless Status Line (y = height - 1)                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 3. Floating Overlays (Contextual & On-Demand)                               │
│    ├── Slash Command Autocomplete Popup (Anchored above prompt)             │
│    ├── Command Palette Modal Overlay (Ctrl+K)                               │
│    ├── Session History & Workspace Drawer (Ctrl+S)                          │
│    ├── Tool Security Permission Review Dialog (RFC-009)                     │
│    └── Card Expansion Detail Viewport (Ctrl+O)                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Progressive Disclosure Rules

1. **Thinking & Reasoning Logs**: Presented as single-line collapsed spinners (`⠋ Thinking (2.4s)...`) during inference and auto-collapse upon completion. Full token chains are disclosed only on `Ctrl+O` expansion.
2. **Tool Execution**: Tool invocations (file reads, searches, bash executions) are presented as 1-line summary cards (`✓ Read 42 lines`). Full parameters, outputs, and diffs are disclosed upon `Ctrl+O` expansion.
3. **Relational Memory Provenance**: Graph traversal and retrieved entities are projected as inline summary chips (`⟡ Recalled 4 memories · [Ctrl+O View Graph]`).
4. **Operational Diagnostics**: Internal cache rates, UDS socket latencies, and telemetry are isolated from the main conversation canvas and accessible strictly via `/doctor` or background logging.
