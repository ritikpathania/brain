# Claude Frontend Parity Regression Contract

> **Contract Status**: Permanent Production Baseline (LOCKED & FROZEN)  
> **Oracle Ground Truth**: `/Users/ritikpathania/Developer/src` (114 React 18 + Ink 5 + Yoga components)  
> **Target Subsystem**: `packages/brain-frontend` (React 18 + Ink 5 + Yoga under Bun)  
> **Backend Integration Boundary**: `BrainFrontendController` → `BrainFrontendAdapter` → `BrainUdsClient` → `Brain Rust Daemon` (100% UNCHANGED)  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
CLAUDE FRONTEND PARITY REGRESSION CONTRACT
================================================================================
STATUS: PERMANENTLY LOCKED & FROZEN 🔒

ARCHITECTURE:
  Claude Code Frontend (100% Canonical Presentation Primitives)
            │
            ▼ (Claude-shaped data contract)
    BrainFrontendAdapter (Sole translation boundary)
            │
            ▼
    BrainFrontendController
            │
            ▼
      BrainUdsClient
            │
            ▼
     Brain Rust Daemon (Relational memory, hybrid search, tools)
================================================================================
```

---

## 1. The Eight Immutable Parity Invariants

Any modification to `packages/brain-frontend` or the presentation layer must strictly satisfy these eight non-negotiable invariants:

### Invariant 1 — Canonical Component Authority
- The canonical Claude Code components in `/Users/ritikpathania/Developer/src` remain the sole authoritative oracle for component taxonomy, JSX structure, styling, layout constants, and interaction behavior.
- Components must retain canonical naming (`FullscreenLayout`, `Messages`, `MessageRow`, `LogoV2`, `PromptInput`, `FuzzyPicker`, `HelpV2`, `StatusLine`, `GlobalSearchDialog`, `Markdown`, `HighlightedCode`, `UserPromptMessage`, `AssistantThinkingMessage`, `AssistantToolUseMessage`, `UserToolResultMessage`, `UserMemoryInputMessage`).

### Invariant 2 — Prohibition of Ad-Hoc UI Chrome
- Brain-specific UI chrome (e.g. persistent top status bars, custom borders, prototype banners, colored outlines, or invented badges) is strictly forbidden.
- The top canvas edge must remain completely borderless; greeting headers scroll naturally with conversation history.

### Invariant 3 — Strict Adapter Boundary Isolation
- All Brain-specific presentation logic, state conversions, and domain data mapping are strictly confined to `BrainFrontendAdapter`.
- Presentation components must receive 100% Claude-shaped props and must contain zero Brain backend logic, zero UDS networking code, and zero database dependencies.

### Invariant 4 — Explicit Allowed-Difference Manifest
- Data substitutions between Brain and Claude are restricted exclusively to the enumerated allowlist:
  1. `LogoV2`: Left panel displays Brain version, tagline, and daemon/memory status; right panel displays Brain command reference.
  2. `FuzzyPicker` & `GlobalSearchDialog`: Displays Brain's local slash command inventory.
  3. `StatusLine`: Status descriptor reflects Brain engine version, daemon health, and relational memory status.
  4. `UserMemoryInputMessage`: Relational memory recall IDs rendered via Claude's memory notification pattern (`⟡` + `#B1B9F9`).

### Invariant 5 — Exclusion of Cloud & Package Infrastructure
- Cloud-only features (Anthropic web OAuth, AWS Bedrock auth, cloud billing/overage upsells, Chrome extensions) and package auto-updaters remain permanently excluded from the local-first Brain REPL.

### Invariant 6 — Zero AST, State & Cell Regression
- All automated test suites (153+ tests) and differential gates (AST structure, render-tree hierarchy across 14 canonical fixtures, event-sequence state machine, and cell matrix across 80x24, 100x30, 120x40, 182x53 viewports) must pass with zero unexplained diffs.

### Invariant 7 — Differential-Gate Approval Required
- Any future modification to the presentation shell requires explicit differential-gate verification against the `/Users/ritikpathania/Developer/src` oracle before merging or deploying.

### Invariant 8 — Backend & Wire Protocol Immutability
- Modifications to the Rust backend (`daemon/`, `crates/`), UDS JSONL wire protocol, `BrainUdsClient`, and `BrainFrontendController` are strictly prohibited during presentation layer maintenance.

---

## 2. Verified File Manifest

```text
packages/brain-frontend/src/
├── App.tsx                                  # Root shell composition & CANONICAL_SLASH_COMMANDS
├── main.tsx                                 # Interactive runtime entrypoint
├── render.ts                                # Ink terminal renderer setup
├── adapter/
│   ├── BrainFrontendAdapter.ts              # Sole data translation boundary
│   └── BrainFrontendController.ts           # Controller & command dispatcher
├── uds/
│   └── BrainUdsClient.ts                    # UDS JSONL streaming client
├── components/
│   ├── FullscreenLayout.tsx                 # Viewport 2-region flex container (borderless top)
│   ├── Messages.tsx                         # Transcript canvas & in-transcript greeting
│   ├── MessageRow.tsx                       # Single message turn dispatcher
│   ├── LogoV2.tsx                           # Canonical LogoV2 & CondensedLogo
│   ├── PromptInput.tsx                      # Canonical PromptInput & footer hints
│   ├── FuzzyPicker.tsx                      # Canonical command FuzzyPicker
│   ├── HelpV2.tsx                           # Canonical HelpV2 modal
│   ├── StatusLine.tsx                       # Canonical StatusLine footer
│   ├── GlobalSearchDialog.tsx               # Canonical Command Palette
│   ├── Markdown.tsx                         # Canonical Markdown AST & HighlightedCode
│   ├── messages/
│   │   ├── AssistantTextMessage.tsx         # Assistant markdown message (streaming cursor ▌)
│   │   ├── AssistantThinkingMessage.tsx     # Thinking indicator & duration (∴ Thinking)
│   │   ├── AssistantToolUseMessage.tsx      # Tool action header & permission UX ([y/n])
│   │   ├── UserToolResultMessage.tsx        # 20-line line-numbered output drawer ( 1 │ )
│   │   ├── UserPromptMessage.tsx            # User prompt card (#1E1E1E fill, 10k cap)
│   │   └── UserMemoryInputMessage.tsx       # Memory notification pattern (⟡)
│   └── theme/
│       └── tokens.ts                        # Claude darkTheme tokens & Unicode glyphs
└── test/                                    # 14 test suites (153 tests passing)
```

---

```text
================================================================================
CONTRACT ENFORCEMENT: PERMANENTLY LOCKED & CERTIFIED 🔒
================================================================================
```
