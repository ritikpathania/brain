# Brain Shell Inc 21 — Command Surface II: `/doctor`, `/memory`, Canonical Registry Design

*Committed as part of the Brain shell program. Reference-only: the Claude Code tree at `/Users/ritikpathania/Developer/claude-code` informs UX interaction grammar, never code.*

## 0. Problem

The shell wires exactly six slash commands (`help clear resume theme permissions quit`, `packages/brain-shell/src/commands/matcher.ts:23-28`) while two complete, committed command components are unreachable:

- `commands/doctor/DoctorCommand.tsx` (118 lines) — full diagnostics UX over the local-only `DoctorProbe`.
- `commands/memory/MemoryCommand.tsx` (195 lines) — searchable knowledge-graph browser.

They are orphaned because they import modules that were never tracked (`components/BrainModal.tsx`, `components/BrainSearchField.tsx`, `adapter/BrainMemoryService.ts`). Meanwhile the shell carries two competing command catalogs: the live static array in `matcher.ts` and a tested-but-importer-less Map registry (`commands/registry.ts:16-39`).

Recon verdict (Inc 21 recon, this session): both commands wire with **zero IPC/schema changes** — `/memory` rides the existing `v1/memory/search` RPC (`daemon/src/transport/uds/handlers.rs:1306`, real retrieval pipeline) already exposed by the client (`UdsBrainBackendClient.ts:813`), and `/doctor` performs no RPC at all (raw socket ping + fs check, `doctorProbe.ts:46-139`).

## 1. Decisions (user-approved in recon rounds)

| Decision | Choice |
|---|---|
| Scope | Wire `/doctor` + `/memory`; canonicalize `commands/registry.ts`; retire duplicate matcher registrations. **Out:** `/config`, transcript search |
| Dependencies | Never import or commit the user's untracked WIP files (`components/*`, `adapter/BrainMemoryService.*`); house equivalents live at fresh non-colliding paths under `ui/overlays/` |
| Registry model | Registry = pure catalog + declarative results; AppShell interprets results into shell state (no shell-state coupling inside the registry) |
| Input handling | Overlays normalize onto the house grammar: `useBoundInput({contexts:['overlay']})` + existing resolver rows — never raw `useInput` |
| Provenance | `doctorProbe.ts` is NOT modified in this increment (its line-3 comment contains a vendor word; untouched file = clean diff by construction) |

## 2. Canonical Command Registry

`packages/brain-shell/src/keybindings`-style data-driven authority, applied to commands:

### 2.1 Contract changes in `commands/registry.ts`

```ts
export type CommandResult =
  | { type: 'text'; value: string }          // notice line
  | { type: 'none' }                          // silent success
  | { type: 'action'; action: 'clear' | 'quit' | 'resume' | 'theme' }
  | { type: 'overlay'; overlay: 'doctor' | 'memory' };

export interface Command {
  name: string;
  description: string;
  aliases?: string[];
  argumentHint?: string;
  hidden?: boolean;
  run(ctx: CommandContext): CommandResult;    // sync, pure, no I/O
}
```

Rationale: the reference product solves component-opening commands with JSX-returning command objects; the Brain-owned equivalent is a declarative result the shell interprets — same capability, no copied mechanism, and the registry stays synchronously testable.

### 2.2 Catalog (single source of truth)

All eight commands register in one new module `commands/builtin.ts` (plain data + pure `run` bodies):

| name | aliases | result |
|---|---|---|
| help | — | text: rendered from `getCommands()` itself |
| clear | — | action: clear |
| resume | — | action: resume (busy-guard + async fetch stay AppShell-side) |
| theme | — | action: theme |
| permissions | — | text: `runPermissionsCommand(args)` output |
| quit | q | action: quit |
| doctor | — | overlay: doctor |
| memory | — | overlay: memory |

### 2.3 Matcher retirement

`matcher.ts` keeps ONLY its pure palette functions (`parseCommandQuery`, `fuzzyMatchCommands`) and they take the catalog as a parameter; call sites pass `getCommands()`:

- `AppShell.tsx:15` drops `COMMANDS`; help text and prefix-disambiguation ("Ambiguous command", alias `q`) read `getCommands()` and behave byte-identically.
- `PromptInput.tsx:5` unchanged mechanically (same function names), now fed the registry catalog.
- The static `COMMANDS` array is deleted. `test/commands/matcher.test.ts` is rewritten against the parameterized signatures; `test/contracts/commandRegistry.test.ts` extends to cover all eight entries, aliases, kinds, and result shapes.

### 2.4 AppShell interpretation

`runCommand` resolves via registry (exact → alias → unique-prefix), then switches on the result: `text` → `controller.notice(value)`; `action` → today's existing branches verbatim (busy-guard on resume included); `overlay` → set `<x>Open(true)`. Only one overlay is ever open: commands execute from the composer, which is paused whenever an overlay is open — invariant preserved from Incs 15–20.

## 3. House Overlay Primitives

### 3.1 `ui/overlays/ModalFrame.tsx` (new)

Props `{ title, subtitle?, footerHints?, width, children }`. Renders the established bordered frame idiom (same visual family as ThemePickerView/ResumePickerView/PermissionDialogView), `Math.min(width, columns)` capping left to callers. No input handling of its own.

No `BrainSearchField` equivalent is built: typed filtering reuses the B5 machinery verbatim (resolver rows `overlay:insert`/`overlay:backspace`, `resolve.ts:39-40`, printable probe `resolve.ts:78`) plus a per-overlay pure reducer.

### 3.2 View components (new, house paths)

- `ui/overlays/DoctorOverlayView.tsx` — props `{ probe?: DoctorProbe, tokens, onDismiss(): void }`. On mount runs `probe.runDiagnostics()`; renders overall health banner (● HEALTHY / ▲ DEGRADED), per-subsystem ✔/✖ rows with latency, remediation hint row; loading and failed states included. Enter/Esc dismiss with system notice "Completed system diagnostics".
- `ui/overlays/MemoryOverlayView.tsx` — props `{ search(query, limit): Promise<MemorySearchResult>, tokens, initialQuery?, onDismiss(): void }`. Query line (`› <query>▏`) captures printables via the B5 rows through a local pure reducer (`memoryOverlayLogic.ts`: insert/backspace/no-op); ↑↓ navigate via `overlayListDecision`; Enter toggles detail expansion of the selected row (excerpt + relations list); Esc dismisses with system notice.
- Superseded tracked components are deleted in the same commits that add their replacements: `commands/doctor/DoctorCommand.tsx`, `commands/memory/MemoryCommand.tsx`. `commands/config/*` stays untouched (out of scope; still inert).

## 4. Data Flow

**/doctor:** view → injected `DoctorProbe` (default real) → local socket/fs probes → report state. No RPC, no daemon dependency beyond liveness semantics.

**/memory:** AppShell opens overlay → passes bound method `controller.searchMemories`. New thin controller wrapper returning a liveness-discriminated result so the view can render "Brain daemon is offline or unreachable." versus "No concepts recorded…":

```ts
async searchMemories(query: string, limit = 20):
  Promise<{ ok: true; memories: RetrievedMemory[] } | { ok: false }> {
  try {
    const res = await this.backend.searchMemory({ query, limit });
    return { ok: true, memories: res.memories };
  } catch {
    return { ok: false };   // ECONNREFUSED / ENOENT / timeout all collapse here
  }
}
```

The view consumes `MemorySearchResult = { ok: true; memories } | { ok: false }`. Client-side fix rides here: `UdsBrainBackendClient.searchMemory` mapping copies the dropped `relations` field (`m.relations ?? []`, one line at `UdsBrainBackendClient.ts:820-830`) so detail panes render connections. Wire action remains `'memory/search'` (router accepts both aliases, `handlers.rs:1306`).

Debounce: keystrokes update the query instantly; fetches fire ≥200 ms after the last keystroke with a monotonic token guard so stale responses never paint.

## 5. Error Handling & Edge Cases

Doctor: probe rejection or missing report → red "Failed to collect diagnostic signals." row; dismissal works from every state. Probe internals untouched (its storage check remains `$HOME/.brain` presence — deepening it is a non-goal).

Memory: offline (`ok:false`) → offline row with the existing start-hint copy; empty results with query → `No concepts matching "<q>".`; empty without query → `No concepts recorded in the Brain knowledge graph yet.`; selection clamps on every refetch; wrap-around normalized to house `overlayListDecision` semantics (clamped, not wrapping — matches theme/resume pickers); enter-on-empty-list is a guarded no-op; score renders as `round(clamp(score,0,100))%`; missing relations → "(No outgoing relations)".

Registry: unknown token → existing "Unknown command" notice; ambiguous prefix → existing "Ambiguous command" notice; `/help` lists all eight commands including the two new ones.

## 6. Testing Strategy

**Unit (TDD, bun test):**
- `commandRegistry.test.ts` — eight entries, alias resolution, kind/result shape, help rendering from the catalog itself.
- `matcher.test.ts` — rewritten: `parseCommandQuery`/`fuzzyMatchCommands` against injected catalog; palette narrowing order unchanged.
- `memoryOverlayLogic.test.ts` — reducer insert/backspace/passthrough; debounce-token guard logic (pure clock injection).
- `doctorOverlayView.test.tsx` / `memoryOverlayView.test.tsx` — house textOf walker: healthy/degraded/offline/loading rows; memory populated/empty/offline, selection movement, expand/collapse detail with relations.
- `client` wire test addition — `searchMemory` preserves `relations`.

**PTY smoke** — new `scripts/ptySmokeInc21.py` against a real daemon (established harness rules: TIOCSWINSZ, per-keystroke writes, occurrence-count waits, behavioral assertions over tail scans):
- Flow A: `/doctor` typed through composer → modal appears → "HEALTHY" visible → enter dismisses → system notice.
- Flow B: seed one memory via RPC `v1/memory/store` → `/memory` → modal lists it → type a fragment → row persists → enter expands details naming a relation → esc closes.
- Regression: `scripts/ptySmokeInc2.py` rerun green (palette behavior over the migrated catalog).

**Gates:** bundle gate (`bun build src/main.tsx --outdir dist --target bun`); failure identities ⊆ documented five; touched-file tsc drift-tolerant; diff-scoped vendor scan of added lines = 0. **No Rust surface is touched — zero cargo scope.**

## 7. Non-Goals

- `/config` (placeholder data + third missing dependency — future increment after real-data source exists).
- Transcript search; session-summary enrichment/live preview; rename UI.
- Deepening `DoctorProbe` probes or touching `doctorProbe.ts` in any way.
- Lazy/async command loading, plugin commands, argument parsing beyond existing word-splitting.
- Any IPC contract, schema, or daemon/domain code change.

## 8. Constraints & Riders

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, adapter boundaries.
- Stack: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit contains ONLY explicitly-added paths (`git add <paths>`, never `git add .`); working-tree user WIP (~3.7k dirty paths) is never staged, stashed, or reverted — including the untracked `components/*` and `adapter/BrainMemoryService.*` files whose NAMES must not be reused for new modules.
- Pushes to origin require explicit user approval each time.
- Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- No cargo invocations (no Rust surface); provenance scan greps only added lines and must return 0.
