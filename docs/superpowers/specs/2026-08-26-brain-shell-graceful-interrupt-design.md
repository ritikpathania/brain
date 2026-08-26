# Brain Shell Graceful Interrupt — Design Spec

**Date:** 2026-08-26
**Status:** Approved audit recommendation (post-Inc 23 rank #1) → spec for review
**Base:** main @ `edec388b`
**Evidence:** Post-Inc 23 gap audit finding A2 (upgraded after design recon)

## 1. Problem

Mid-turn interruption is broken in three compounding ways, all verified on
current `main`:

**1a. Escape aborts the stream but the turn then lies about it.** The wire
path is fully built: `translateKey` maps escape → `{type:'abort'}`
(`translateKey.ts:38`) → `PromptInput.tsx:129-131` → `onAbort` →
`controller.abort()` (`AppShell.tsx:387`, composer is never disabled —
`AppShell.tsx:381`). The UDS client's per-turn abort listener then writes
`v1/generation/cancel` and destroys the stream socket
(`UdsBrainBackendClient.ts:126-152`), pushing a typed
`{type:'finished', status:'cancelled'}` chunk (`:145-150`; `status`
union includes `'cancelled'`, `BrainBackendClient.ts:109`). The daemon
honors cancellation with a clean branch in the stream loop
(`handlers.rs:2292-2294`) and its `GenerationGuard::drop` removes the
registry entry even on abnormal stream death (`handlers.rs:135-141`), so
the session never busy-locks. **But** the controller's post-loop settle
marks every non-error turn `'completed'` (`sessionController.ts:355-358`)
and `handleChunk` has no branch for a cancelled finish — the partial
answer freezes with zero feedback and a false "completed" status.

**1b. Ctrl+C hard-kills the app mid-turn.** Two unconditional
`process.exit(0)` sites fire while a turn streams: the global binding
(`resolve.ts:19` → `AppShell.tsx:91`) and PromptInput's own ctrl+c
command (`PromptInput.tsx:125-127`). Scrollback, notices, and the in-flight
turn are destroyed with no teardown.

**1c. `/quit` is not graceful either.** The command's `quit` action exits
raw (`AppShell.tsx:267`), same hazard when busy.

## 2. Goal

Escape during a turn visibly interrupts it (partial output freezes, a dim
"⎿ Turn interrupted" row appears, tools settle as cancelled, the composer
is immediately usable, the session stays live). Ctrl+C while busy
interrupts instead of killing, with a second press exiting; idle Ctrl+C
exits exactly as today. `/quit` tears down an active turn before exiting.
No daemon, DTO, or crates changes.

## 3. Non-goals

- No keybinding-table changes: `composer:abort` stays registered in
  `resolve.ts:22` (delivery is PromptInput-internal; the table entry is
  documentation, removing it is churn without behavior).
- No interrupt UX for parked permission dialogs (esc there closes the
  dialog path, unchanged).
- No queued-input (`offline replay queue`) cancellation semantics.
- No double-press confirmation state machine beyond the busy/idle split.
- Scores, synthetic typed-path metadata (B8), DTOs: untouched.
- Zero diffs: `crates/**`, `daemon/src/**` (tests-only exception below),
  `daemon/src/server/protocol.rs`.

## 4. Design

### 4.1 Controller: honest interrupt settlement

`packages/brain-shell/src/state/sessionController.ts`:

- New private `userInterrupted = false` (reset to `false` at submit start,
  alongside `this.sawError = false` at `:337`).
- New public method beside `abort()` (`:114-116`):

  ```ts
  /** User-facing interrupt: aborts the wire stream and remembers why, so
   * settlement renders an interruption instead of a silent completion. */
  interruptTurn(): void {
    if (!this.busy || !this.aborter) return;
    this.userInterrupted = true;
    this.aborter.abort();
  }
  ```

  Existing `abort()` stays (raw signal primitive).
- In the stream loop body, record a cancelled finish:

  ```ts
  if (chunk.type === 'finished' && chunk.status === 'cancelled') {
    this.sawCancelledFinish = true;
  }
  ```

  (`sawCancelledFinish` also reset at submit start.) Belt-and-braces: a
  daemon-side cancellation ends the stream the same way even if our flag
  missed.
- Post-loop settle (`:355-358`) becomes:

  ```ts
  const interrupted = this.userInterrupted || this.sawCancelledFinish;
  this.finishTurn(
    interrupted ? 'interrupted' : this.sawError ? 'error' : 'completed',
    interrupted ? undefined : this.lostDuringTurn ? CONNECTION_LOSS_ROW : undefined,
  );
  ```

  The catch-arm checks `this.userInterrupted` first: if the abort raced
  into a thrown teardown error, settle `'interrupted'` (not `'error'`)
  unless the message is a genuine connection loss.
- `finishTurn` signature widens to `'completed' | 'error' | 'interrupted'`.
  `'interrupted'` shares the error arm's pending-tool settlement
  (`tool_cancelled`, reason `'turn interrupted'` at `:462-467`) but pushes
  a dim system row `⎿ Turn interrupted` (same visual family as the
  existing connection-loss row; final copy pinned at plan time from the
  row-rendering constants) and applies no error styling. Busy clears,
  ticker stops, `aborter` nulls — existing tail behavior (`:487`).

### 4.2 AppShell: busy-aware exit, wired abort

`packages/brain-shell/src/ui/shell/AppShell.tsx`:

- One helper above the component:

  ```ts
  function requestExit(controller: SessionController, snapshot: ShellSnapshot): void {
    if (snapshot.busy) {
      controller.interruptTurn();
      controller.notice('Interrupted — press ctrl+c again to exit.');
    } else {
      process.exit(0);
    }
  }
  ```

- Global binding `:91`: `if (action === 'shell:exit') requestExit(controller, snapshot);`
- Command `quit` action `:267` → graceful quit: if busy, `interruptTurn()`,
  then wait bounded (≤2 s, polling `snapshot.busy` via a one-shot
  subscription with timeout fallback) and `process.exit(0)` regardless —
  explicit quit always quits, just politely.
- `:387` becomes `onAbort={() => controller.interruptTurn()}`.
- Pass `onRequestExit={() => requestExit(controller, snapshot)}` to
  `PromptInput`.

After an interrupt the composer re-renders idle within a tick, so the
advertised "second ctrl+c exits" falls out of the busy flip — no extra
state machine.

### 4.3 PromptInput: routed exit

`packages/brain-shell/src/ui/composer/PromptInput.tsx:125-127`: replace the
inline `process.exit(0)` with `props.onRequestExit?.()` (prop optional;
component standalone-safety preserved — absent prop falls back to
`process.exit(0)` exactly as today). The abort arm `:129-131` is unchanged.

### 4.4 Why no daemon change is needed

Cancellation already terminates generations cleanly and the registry
cannot leak entries (`GenerationGuard` drop-spawns removal even when the
stream task dies from the destroyed socket, `handlers.rs:135-141`). The
cancel frame rides the stream socket before destruction; the daemon's
sequential read loop may not consume it, but the subsequent pipe break
ends the stream task and drops the guard — best-effort by design, no
orphaned generations, no busy-lock.

## 5. Testing strategy

1. **Controller unit** (new `packages/brain-shell/src/test/state/interruptTurn.test.ts`):
   fake client whose `streamText` yields a few tokens then pends until
   `request.signal` aborts, then yields `{type:'finished', status:'cancelled'}`
   (mirrors the real client contract). Assert: `interruptTurn()` during
   busy → busy false after settle; frozen rows contain the interrupted
   marker; status is not error; a requested-but-unsettled tool call settles
   as `tool_cancelled`; a follow-up submit succeeds on the same session
   (proves no stuck state). Idle `interruptTurn()` is a no-op.
2. **Client-stream integration** (new `…/test/client/streamInterrupt.test.ts`):
   in-process `net.Server` speaking the newline frame protocol — emits
   token frames every few ms, and when the cancel frame arrives, answers
   `finished/cancelled` and closes. Drive the REAL
   `UdsBrainBackendClient.streamText` + controller: proves the cancel
   frame actually reaches the wire with correct `generation_id`/
   `session_id`, the generator ends, and the controller settles honestly.
   This is the deterministic mid-stream proof the PTY layer can't give.
3. **PromptInput/exit-routing unit**: with `onRequestExit` spy, ctrl+c
   invokes the spy (never `process.exit`); escape invokes `onAbort`;
   absent `onRequestExit` preserves today's direct-exit fallback.
4. **Rust wire-lifecycle test** (new tracked
   `daemon/tests/uds_generation_lifecycle_tests.rs`, harness copied from
   the relations suite): (a) `v1/generation/cancel` with no active
   generations answers `{"type":"cancelled","status":"ok"}`;
   (b) after a completed `v1/generation/stream` round-trip, a follow-up
   `session/append_turn` on the same session is NOT rejected
   session-busy — proving guard cleanup on the normal path. Mid-stream
   cancellation is deliberately NOT asserted here (the mock model streams
   instantly — zero sleeps in `mock.rs:214-300` — so no deterministic
   busy window exists server-side); layer 2 owns that proof.
5. **PTY smoke** (new `scripts/ptySmokeInc24.py`, real daemon): boot →
   short turn completes normally (completion path unregressed) → esc
   during idle is harmless → ctrl+c exits rc=0 → relaunch → `/quit`
   exits rc=0. Interrupt-window rendering is NOT asserted at this layer
   (instant mock turns make it a race); layers 1-2 own it.

## 6. Verification gates & repo constraints

Standard battery:

- Cargo with the macOS RUSTFLAGS wrapper, `-p brain-daemon`, full suite
  `--no-fail-fast`, UNFILTERED logs read from files. Expected failure
  identity: only the known untracked security-audit mismatch.
- bun suite: all NEW tests pass; the five documented failure identities
  unchanged.
- Vendor scan on added lines (`daemon/`, `packages/brain-shell/src/`) → 0.
- Zero-diff gates vs base: `crates/**`, `daemon/src/server/protocol.rs`,
  and every pre-existing dirty path byte-preserved (explicit-path commits
  only; hunk-filter recipe on shared files; never stash; ~4.9k WIP paths).
- PTY regressions: `ptySmokeInc21.py` 10/10, `ptySmokeInc2.py` 12/12, new
  `ptySmokeInc24.py` green.
- Commit trailer `Co-Authored-By: Claude <noreply@anthropic.com>`; work in
  place off a feature branch; no pushes without explicit approval.

## 7. Ledger impact

- A2 mid-turn interrupt → designed here, closes on merge (subsumes the
  audit's three dead-ends; corrected finding: escape's wire path already
  worked — settlement, exit safety, and honesty were the gaps).
- Unchanged/open: edge materialization & dangling targets; real scores
  (domain track); consolidate surfacing (rank #2 next); resume truthfulness
  contract-half; standalone-build debt (85 errors @ edec388b, root cause
  `brain_core::model` unresolved); security-audit mismatch (product
  decision); fixture hygiene; typed-path synthetic metadata (B8);
  dead-client surface inventory (9 remaining methods after this uses none
  of them — `cancelGeneration` equivalent ships inline in streamText).

## 8. Risks

| Risk | Mitigation |
|---|---|
| Abort races into the catch-arm and gets mislabeled an error | Catch checks `userInterrupted` before connection-loss/error classification; unit test pins it |
| Double-fire: PromptInput ctrl+c AND global shell:exit both handled | Both call the same idempotent `requestExit`; `interruptTurn()` guards on `this.busy`; `process.exit` twice is unreachable (first call wins) |
| Bounded-quit wait hangs | Hard 2 s timeout forces exit; explicit quit can never strand the user |
| Instant mock turns make PTY interrupt untestable | Accepted: interrupt semantics proven at controller + fake-server integration layers; PTY owns completion/exit/no-regression |
| `status:'cancelled'` chunks from sources other than user esc | Belt-and-braces flags mean such turns still settle honestly as interrupted rather than lying as completed |
