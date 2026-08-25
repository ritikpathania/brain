# Brain Shell Inc 17 — Always-Allow Permission Rule Store (B2)

**Date:** 2026-08-25 · **Base:** `main` @ `b4cd4f2b` · **Type:** Feature increment (shell-only)
**Status:** Design approved in chat — approach A chosen from three candidates; granularity, scope, and management UX decided by the user before this spec.

## 0. Problem

Every agentic tool call prompts, every turn, forever. The daemon's server-side
permission grant-set is explicitly inert for the agentic round-trip
(`daemon/src/transport/uds/handlers.rs:820-829`: "this file has NO is_granted
precheck, so agentic calls always prompt regardless of grant-set state"), and
nothing on the wire or in the shell remembers a verdict. A user who trusts
`git status` re-approves it in every session.

Current permission lifecycle (all verified against source):

1. Daemon hits a tool call → emits `tool_permission_requested`, registers a
   waiter keyed by `call_id`, and **parks the stream**
   (`handlers.rs:2325-2363`) until a verdict arrives via `v1/tool/resolve`
   (accepted on any connection, `:1832`) — or until a deny-by-default timeout
   (`BRAIN_TOOL_PERMISSION_TIMEOUT_SECS`, default 300 s).
2. Shell `handleChunk` parks a `PendingPermissionView` on the snapshot
   (`sessionController.ts:324`).
3. `AppShell` renders `PermissionDialogView`; the `dialog` input context maps
   keys to actions via the flat table in `keybindings/resolve.ts`.
4. `controller.resolvePermission(callId, granted)` settles local rows, then
   fire-and-forgets `client.resolveToolPermission(callId, granted)` →
   `v1/tool/resolve` over its own connection (`UdsBrainBackendClient.ts:890`).

## 1. Decisions (user-approved)

| Question | Decision |
|---|---|
| Rule granularity | **Tool + input prefix**: `{ tool, inputPrefix }`; matches when the tool's primary input string starts with the stored prefix |
| Rule scope | **Global** — one rule set in `~/.brain/config.json`, effective in every session on the machine |
| Management UX | **`/permissions` slash command** — list with indices, `/permissions remove <n>` deletes |
| Architecture | **Approach A: shell-side store + auto-resolve over the existing wire.** Daemon and wire format untouched; auto-allow rides the same `v1/tool/resolve` frame manual Allow uses |

Rejected: per-tool-only rules (too blunt for bash), exact-match-only (near-zero
hit rate), per-project scoping (adds a matching dimension for little gain),
daemon-side enforcement (duplicates rule semantics in Rust, needs new wire
messages for remote management, touches the security gate path — all to save a
~1 ms local-socket round trip), session-only memory (defeats cross-session
purpose).

## 2. Rule Model & Store

New file `packages/brain-shell/src/state/permissionRules.ts`, sibling idiom to
`themeStore.ts`:

```ts
export interface AllowRule { tool: string; inputPrefix: string }
```

- **Primary-input extraction** — `primaryInputString(input: Record<string,
  unknown>): string` exported from this module: first non-empty trimmed string
  among the canonical keys `command, file_path, path, query, pattern, url,
  prompt`, else the first non-empty string value, else `""`. This is the
  proven heuristic already inside `summarizeToolInput` (`ui/transcript/
  MessageRow.tsx:43`); that function is refactored to call it, keeping only
  its display-side `.slice(0, 60)` locally — one canonical match key, so the
  dialog's shown summary and the stored rule can never drift.
- **Matching** — pure `matchingRuleIndex(toolName: string, input:
  Record<string, unknown>, rules: AllowRule[]): number`: index of the first
  rule where `rule.tool === toolName &&
  primaryInputString(input).startsWith(rule.inputPrefix)`, else `-1`.
  Byte-exact case-sensitive prefix. An **empty `inputPrefix` matches every
  invocation of that tool** — what "Always allow" stores when the input has no
  string field.
- **Persistence** — synchronous fs, merge-write into the existing config doc:
  - `readAllowRules(): AllowRule[]` — tolerant parse of `permissions.allow`;
    missing file/key → `[]`; entries failing `{tool: non-empty string,
    inputPrefix: string}` are filtered out.
  - `addAllowRule(rule): void` — read-merge-write preserving all other keys
    (the theme key survives); skips appending an identical existing rule.
  - `removeAllowRule(index): boolean` — removes the nth entry of the current
    read order; returns `false` when out of range.
  - Path resolution reuses themeStore's exported `configPath()` (honors
    `BRAIN_CONFIG_PATH`). Rules are read fresh at each check — never cached —
    so `/permissions remove` and hand-edited files take effect immediately.

## 3. Auto-Allow Decision Point & Dialog Third Option

### Controller (`state/sessionController.ts`)

The `permission_request` branch of `handleChunk` becomes:

1. Build the same `PendingPermissionView` as today.
2. Consult `matchingRuleIndex(view.toolName, view.input, readAllowRules())`.
3. **Match ≥ 0 → never park the dialog**: emit notice `` Allowed ${toolName}
   (rule ${n}) `` (n = 1-based rule index), then fire the existing
   fire-and-forget `client.resolveToolPermission?.(callId, true)`.
4. **No match → exactly today's flow** (park the view; user decides).

**Wire-failure fallback.** The manual path swallows resolution errors because
local UX is already settled. The auto path must not: an unresolved `call_id`
parks the daemon's waiter for up to 300 s and then denies silently. So the
auto path's rejection handler **re-parks the dialog** with the same view —
the user sees the normal prompt and decides manually; a retry resolves the
same `call_id` over a fresh connection. Failure degrades to pre-B2 behavior,
never to a silent deny.

**Rule creation.** New controller method `resolvePermissionAlways(callId)`:
derives `{ tool: view.toolName, inputPrefix: primaryInputString(view.input) }`
from the pending view while it still exists, persists via `addAllowRule`
(save failure → notice, but this call is still granted), then proceeds down
the identical granted path.

**Rider fix.** The stale docblock on `resolvePermission` ("The daemon cannot
receive resolutions yet…", `sessionController.ts:106-107`) contradicts its own
wire call at line 121 and is rewritten to describe the real behavior.

### Dialog

Three options: `[ Allow ]  [ Deny ]  [ Always allow ]`.

- `keybindings/resolve.ts`: add `{ action: 'dialog:always', context:
  'dialog', key: 'a' }`.
- `permissionDialogLogic.ts`: `dialogDecision` gains `'always'` —
  `dialog:always → {type:'always'}`; `dialog:commit` with `selected === 2` →
  `'always'`; `dialog:left`/`dialog:right` clamp within indices 0–2;
  y/n/esc semantics untouched (**esc always denies** remains law).
- `PermissionDialog.tsx`: renders the third option; help line becomes
  `←→ choose · enter confirm · y allow · a always · n deny · esc denies`.
- `AppShell.tsx`: `permSelected` widens to `0 | 1 | 2`; the `always` decision
  calls `controller.resolvePermissionAlways(permission.callId)`.

## 4. `/permissions` Command

Registered in the existing `COMMANDS` table (`name: 'permissions'`,
description "List or remove always-allow rules"); dispatched in `runCommand`:

- Bare `/permissions` → pure helper `describeRules(rules): string[]` rendered
  through `controller.notice`:

  ```
  Always-allow rules (~/.brain/config.json):
   1. bash — commands starting with "git "
   2. read_file — any invocation
  Remove with: /permissions remove <n>
  ```

  Empty store → single line `No always-allow rules saved.` A shared
  `describeRule(rule)` formatter produces both list lines (`commands starting
  with "git "` / `any invocation`) and removal notices, so wording cannot fork.
- `/permissions remove <n>` → strict integer parse; `removeAllowRule(n - 1)`;
  success notices `` Removed rule 1 (bash "git "). ``; out-of-range or
  non-numeric input notices the error. Works while busy (pure local, like
  `/clear`).

## 5. Testing Strategy

All new logic is pure or fs-seamed; unit tests carry the weight.

1. `src/test/state/permissionRules.test.ts` — matcher truth table (tool
   mismatch, prefix hit/miss, empty-prefix tool-wide rule, first-match wins);
   store round-trips against a `BRAIN_CONFIG_PATH` tmp file including theme
   key survives merge-write, dedupe, remove, out-of-range, malformed-entry
   filtering; `describeRules`/`describeRule` formatting.
2. Controller tests (stub client recording `resolveToolPermission`) — matching
   rule auto-allows with no parked dialog and resolution recorded
   `granted: true`; non-matching request parks the dialog as today; rejecting
   wire resolution falls back to parking the dialog; `resolvePermissionAlways`
   persists the derived rule and grants.
3. `permissionDialogLogic.test.ts` extensions — clamping at both ends,
   `a` mapping, commit-index dispatch.
4. PTY smoke `scripts/ptySmokeInc17.py` — daemon stub emits
   `permission_request` for `bash` + `{"command":"git status"}` with the
   config pre-seeded with the `"git "` rule; assert no permission dialog
   renders, the allowed notice does, and the stub receives `v1/tool/resolve`
   with `granted: true`. Proves the full wire loop end-to-end.

Standard gates: full bun suite vs baseline 280 tests / 275 pass / 5 documented
fails plus new tests; tsc touched-file parity vs pristine main; vendor scan 0
over `crates daemon packages scripts` (docs excluded by design); cargo
workspace with sole permitted audit failure; PTY fixture drift restored, never
committed.

## 6. Non-Goals

- No daemon changes; no wire-format changes; no server-side rule enforcement.
- No per-project scoping, TTL/expiry, glob/regex patterns — byte prefix only.
- No multi-permission queueing (single-slot `pendingPermission` stays as-is;
  pre-existing limitation).
- No audit log of auto-allowed calls beyond the notice row.
- `!` shell passthrough untouched — keystroke-as-grant never prompts, so no
  rule interplay exists there.

## 7. Constraints

- Preserve Brain architecture and all seams; shell-only change inside
  `packages/brain-shell`.
- No Claude/Anthropic models, APIs, authentication, pricing, billing, or
  LLM-specific product concepts; Brain-owned implementation only.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit carries only explicitly added paths; pushes to origin require
  explicit user approval each time.
- **Rider commit (first on the branch):** remove the unused
  `"@anthropic-ai/sdk"` dependency (`package.json:14`), regenerate `bun.lock`
  via `bun install`, verify the suite — separate commit ahead of feature work.
