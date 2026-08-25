# Brain Shell Inc 16 — Freeze-Path Deduplication (Typewriter Remainder)

**Date:** 2026-08-25 · **Base:** `main` @ `77bd8ab1` · **Type:** Bounded correctness fix
**Status:** Design approved in chat — evidence, root cause, and fix boundary confirmed by the user before this spec.

## 0. Problem

The turn-freeze path can duplicate streamed text. Whenever a turn ends with
undrained typewriter buffer, the frozen assistant row carries the full answer
**plus a repeated copy of its tail**. Reproduced deterministically against
`main`:

| Sequence | Frozen result |
|---|---|
| one token `abc`, instant finish | `"abcabc"` |
| 200-char token, real 16 ms ticker runs ~50 ms | 136 of 200 chars duplicated |
| token then abort-classified error chunk | `"partialpartial"` |
| real `abort()` mid-stream | 68 of 100 chars duplicated |

Production impact: the tail fragment of an answer visibly repeats whenever the
last chunks land within a couple of ticks of `finished` — common with fast
local daemons. Invisible to existing fixtures because PTY assertions are
`contains`-based, and to unit tests because they assert on the live view or
use `includes()`.

## 1. Root Cause

`SessionController` records every text delta **twice by design**:

- `handleChunk` pushes the mapped `text_delta` event into `this.events`
  unconditionally (`sessionController.ts:347`) — the authoritative, complete,
  timing-independent record.
- The same delta enters `TwoStageTypewriterQueue` (`:349`), a pacing buffer
  whose only consumer is the live view: the 16 ms ticker drains ≤32 chars per
  tick into `live.responseText`, which is discarded at freeze.

`finishTurn` then treats the queue as a second source needing rescue
(`:387-391`): if `queue.pending > 0`, it drains the rest and pushes it as an
**additional** `text_delta`. Since `events` already contains those exact
characters, this push can only ever duplicate. Duplication size =
`pending` at freeze.

Git archaeology: the double-routing and the flush landed together in the
founding controller commit `0699fc1d`; the flush was redundant from birth,
not orphaned by a refactor.

## 2. Invariant

> Every streamed text delta appears **exactly once** in the final frozen row,
> regardless of drain timing.

After the fix this holds **by construction**: each delta enters `events` at
exactly one push site; the transformer concatenates faithfully; no
reconciliation step exists to mis-fire.

## 3. Fix Boundary (investigated, not assumed)

| Candidate | Verdict |
|---|---|
| `TwoStageTypewriterQueue` | Innocent — plain FIFO, correct semantics, single consumer. Untouched. |
| `handleChunk` double-routing | Correct — `events` must stay complete; queue is a legitimate live-view projection. Untouched. |
| Transformer / `turnToRows` | Pure concatenation/projection given honest input. Untouched. |
| **`finishTurn` remainder flush** | **The defect. Delete lines 388–390 and replace the comment.** |

Safety checks performed: nothing reads `queue.pending` after freeze; the
queue instance is recreated on every `submit()`; the typewriter's two-stage
semantics and live-view behavior are unchanged; `runShellCommand` builds its
own events and never calls `finishTurn`; thinking deltas never touch the
queue; no test depends on the duplication (all frozen-markdown assertions
are `includes()`, including Inc 15's reconnect tests).

## 4. Testing Strategy

New unit file `src/test/state/sessionControllerFreeze.test.ts` asserting
**exact equality** of frozen markdown:

1. single-token instant completion → exact text (currently fails; passes post-fix);
2. multi-token instant completion → exact concatenation;
3. error-chunk completion → exact text once;
4. real `ctl.abort()` mid-stream → full text exactly once (exact equality holds post-fix regardless of how many ticks fired);
5. empty response → still no assistant row;
6. thinking-only turn → thinking row present, no phantom assistant row.

Cases 1–3 are fully deterministic (microtask completion provably precedes
the first 16 ms tick). Existing suites require zero edits; Inc 15/Inc 6 PTY
smokes stay positive-assertion and unaffected.

Standard gates apply (bun suite baseline + new tests, tsc touched-file parity,
vendor scan 0 on `crates daemon packages scripts`, cargo workspace with sole
permitted audit failure).

## 5. Non-Goals

- No typewriter redesign — two-stage semantics, tick rate, and chunk size stay as-is.
- No changes to the queue contract (`contracts/streaming.ts`), transformer,
  projections, daemon, or wire format.
- No retroactive repair of already-frozen transcripts (none persisted).
- No new UI affordances (nothing user-visible beyond corrected text).

## 6. Constraints

- Preserve Brain architecture and all seams; shell-only change in
  `packages/brain-shell`.
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM product
  concepts; Brain-owned implementation only.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit carries only explicitly added paths; pushes to origin require
  explicit user approval each time.
