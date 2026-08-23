# Brain Shell — Contracts-First Frontend Rebuild

**Date:** 2026-08-23
**Status:** Approved design, pending implementation plan
**Scope:** `packages/brain-shell` presentation layer rebuild
**Supersedes:** `docs/archive/frontend-parity/*` (archived vendor-based approach)

---

## 1 · Charter

Make Brain feel like Claude Code if Claude Code were built on top of Brain — the user must
immediately recognize the Claude Code interaction model while experiencing Brain's own
memory-first, agent, workspace, and knowledge capabilities.

### Hard constraints

1. **No copied source.** The reference tree at `/Users/ritikpathania/Developer/claude-code`
   (reconstructed "leaked" Claude Code source) is *implementation archaeology only*: we extract
   observable UX contracts from it and write original code against those contracts. Nothing from
   that tree is vendored, committed, or redistributed. It stays outside this repository forever.
2. **No Anthropic product concepts.** No Claude/Anthropic models, APIs, authentication, pricing,
   billing, or LLM-vendor-specific product surfaces in Brain's UI.
3. **Brain runtime is authoritative.** The Rust daemon (`crates/brain-daemon`, UDS at
   `/tmp/brain.sock`) remains the composition root and backend of record. All UI data flows
   through existing adapter/client seams.
4. **Preserve Brain architecture.** Domain model, IPC contracts, runtime, memory, retrieval,
   graph, provenance, agents, and adapter boundaries are untouched by frontend work.
5. **Incremental delivery.** Small increments, each independently verifiable; no big-bang
   rewrites, no framework changes (stack stays Bun + React 19 + Ink 7 + yoga-layout).

## 2 · Audit summary (basis for this design)

### Reference tree (external, read-only)

- ~1,976 TS/TSX files; UI layer: `src/components/` (395 files), screens in `src/screens/`
  (`REPL.tsx` = composition root).
- `src/ink/` is a custom Ink fork (own reconciler/DOM/layout/hit-testing/selection). We do **not**
  replicate it; stock Ink 7 provides the same observable primitives.
- Composer: `PromptInput/` core (~2.3k lines) + footer/mode-indicator/history-search satellites;
  input modes: `prompt`, bash (`!` prefix).
- Transcript: 33-type message taxonomy rendered through a virtual message list with a
  static(frozen)/live(bottom pane) region split.
- Full keybinding framework (`src/keybindings/`: schema → parser → resolver → user overrides)
  and an optional vim-mode subsystem.

### Brain-shell inventory (this repo)

| Layer | Size | Vendor coupling | Disposition |
|---|---|---|---|
| `adapter/` | 18 files / ~3.3k lines | none | **Keep unchanged** |
| `client/` | 2 files / ~1.5k lines | none | **Keep unchanged** |
| `commands/` | 4 files / ~600 lines | none | Keep; re-type against contracts |
| `components/` | 9 files / ~1.5k lines | none | Keep; fold into `ui/` over time |
| `shims/` | 53 files / ~13k lines | **types imported from `vendor/claude`** | Retire; migrate into `contracts/` + `ui/` |
| `test/` | 63 files / ~15k lines | ~30 files import vendor types | Keep harness; swap imports |
| `transport/`, `stores/`, `model/` | empty dirs | — | Remove (stale scaffolding) |
| `vendor/claude/` | 1,938 files / 168 MB | is the copied source | **Delete after contracts land** |

Key finding: Brain's original code was written against Claude Code's type system via the vendored
tree. The pivotal move is defining Brain-owned contract types so every layer stands alone.

## 3 · Capability mapping (CC surface → Brain disposition)

Legend: **HAVE** = existing Brain capability covers it · **BUILD** = brain-shell must implement ·
**GAP** = missing Brain capability, identified for backend follow-up (not blocking UI).

| Claude Code capability | Brain disposition |
|---|---|
| Streaming assistant text w/ typewriter queue | **HAVE** (`adapter/BrainTurnEvents`, typewriter pattern) → re-house in `ui/transcript` |
| Thinking blocks display | **BUILD** (view exists conceptually; render from Brain turn events) |
| Tool-use / tool-result cards (collapse/expand) | **HAVE** protocol events; **BUILD** card UI |
| Markdown + syntax-highlighted code | **BUILD** (MIT markdown tokenizer such as `marked` → Ink nodes; highlighter per theme roles) |
| Composer: multiline editor, history recall, paste, undo | **BUILD** on stock Ink input primitives |
| Input modes: prompt, `!` shell passthrough | **BUILD** (`!` executes via daemon shell service if present, else local spawn — decision at plan time) |
| Slash commands + fuzzy palette | **HAVE** 4 commands; **BUILD** palette + command registry typed by contracts |
| Permission prompts (allow/deny/always) | **HAVE** `BrainPermissionMapper`; **BUILD** dialog UI |
| Session resume/picker | **HAVE** `BrainSessionStore` + daemon sessions; **BUILD** picker screen |
| Themes incl. colorblind variants | **HAVE** `BrainTheme.ts` tokens; **BUILD** picker + daltonized palettes |
| Status line | **BUILD** (Brain content: workspace/memory status — no Anthropic plan/pricing text) |
| Welcome/logo screen | **BUILD** (existing `shims/LogoV2.tsx`/`Clawd.tsx` seed it) |
| Keybinding framework w/ user overrides | **BUILD** slim version of observed contract (schema→resolver→user file) |
| Vim mode | Out of scope v1 (flag-gated later if wanted) |
| Swarm/teammate UI, MCP elicitation dialogs, cloud/desktop surfaces | Out of scope — no Brain equivalent required |

## 4 · Target architecture

```
packages/brain-shell/src/
├── contracts/     ★ Brain-owned UI types (the IP break)
│   ├── messages.ts    transcript message taxonomy + view-model shapes
│   ├── commands.ts    Command interface, CommandResult, registry types
│   ├── streaming.ts   stream view-models, typewriter queue contract
│   ├── input.ts       PromptInputMode, keybinding & vim-state types
│   └── theme.ts       semantic color roles (brand/accent/subtle/success/error/diff…)
├── ui/            ★ Presentation components built ONLY on contracts/
│   ├── shell/         AppShell: fullscreen layout, static/live region split, SIGWINCH
│   ├── composer/      prompt input, modes (!), footer, history search
│   ├── transcript/    MessageRow dispatch, markdown renderer, thinking, tool cards
│   └── overlays/      dialogs, permission prompts, slash palette
├── adapter/        unchanged (turn events → view-models, stores, permission mapper)
├── client/         unchanged (UDS transport to daemon)
├── commands/       re-typed against contracts/commands.ts
├── shims/          retired incrementally; survivors move into ui/ or contracts/
└── test/           harness kept; PTY smoke replaces cell-oracle suites
```

Deliberate deviations from the reference (both charter-sanctioned):

- **Stock Ink 7** from npm instead of their forked renderer. Observable behavior is reproduced at
  the component-contract level; internal renderer differences are acceptable.
- **`vendor/` deleted immediately after contracts land** (own commit, recoverable from history).

## 5 · Increments

| # | Delivers | Verification gate |
|---|---|---|
| **0** | `contracts/` types + shim/test import swap + **delete `vendor/`** + remove stale empty dirs | `bun test` green without vendor; bundle resolves |
| **1** | Composer + transcript loop: `AppShell` static/live split, `PromptInput` (prompt/`!` modes, history, paste, undo), `MessageRow` dispatch (UserText, AssistantText, Thinking, ToolUse collapsed→expanded, SystemError), markdown renderer, `Spinner`, typewriter drain wired to daemon stream | Unit tests + PTY smoke (launch / mid-stream / expanded tool card) |
| **2** | Command surface: slash registry + fuzzy palette, keybinding framework, `/help` | Unit tests + PTY smoke |
| **3** | Session frame: welcome/logo, resume picker, status line, themes (+daltonized), permission dialogs | Unit tests + PTY smoke |

Vim mode and swarm/teammate UI are out of scope unless explicitly added later.

## 6 · Data flow (one-way)

```
daemon (UDS) → client/ → adapter/BrainTurnEvents
             → BrainTurnTransformer → view-models typed by contracts/messages.ts
             → React render (static region = frozen transcript; live region = composer/spinner/status)
UI actions → adapter seams only (never direct socket access from components)
```

Streaming chunks buffer in the two-stage typewriter queue (network completion decoupled from
drain cadence) — preserving the existing AGENTS.md-documented pipeline.

## 7 · Error handling

- Daemon disconnect → top banner + reconnect with exponential backoff; queued input preserved.
- Malformed envelope → drop event + debug log; renderer never crashes on bad frames.
- Unknown message type → collapsible raw-JSON fallback row (protocol evolution can't break old shells).
- Renderer exceptions → Ink error boundary prints message + keeps session alive where feasible.

## 8 · Testing strategy

1. **Unit (`bun test`)**: component render tests, state-machine tests (composer modes, queue drain,
   palette matching, keybinding resolution), contract round-trip tests against adapter events.
2. **PTY smoke**: script drives the real binary through three canonical flows, capturing text
   fixtures committed beside tests: launch frame, mid-stream output, expanded tool card.
3. **No cell-level differential oracle** against Claude Code (Brain renders its own content).

## 9 · Governance

- Reference tree is never copied into the repo, referenced by path in code, or bundled.
- Each increment lands as its own commit series; `vendor/` deletion is isolated for easy revert.
- AGENTS.md TUI rules continue to apply (theme tokens everywhere, rounded-border panels,
  SIGWINCH-safe flex layouts, compact-width handling).
