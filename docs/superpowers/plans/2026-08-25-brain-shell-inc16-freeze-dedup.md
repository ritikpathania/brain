# Brain Shell Inc 16 — Freeze-Path Deduplication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the turn-freeze path from duplicating undrained typewriter tail text in frozen assistant rows.

**Architecture:** `SessionController.handleChunk` already records every `text_delta` verbatim in `this.events` before queueing it for live-view pacing; the queue is a pacing projection only. The sole defect is `finishTurn`'s remainder flush, which re-pushes still-pending queue characters as an extra event. Delete that flush; the exact-once invariant then holds by construction.

**Tech Stack:** Bun + TypeScript (brain-shell package), `bun:test`, existing React/Ink UI untouched.

**Spec:** `docs/superpowers/specs/2026-08-25-brain-shell-inc16-freeze-dedup-design.md`

## Global Constraints

- Preserve Brain's architecture, domain model, IPC contracts, runtime boundaries; shell-only change.
- No Claude/Anthropic models, APIs, authentication, pricing, billing, or LLM-specific product concepts; no vendor-derived code.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit contains ONLY explicitly-added paths (`git add <paths>`, NEVER `git add .`).
- Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- NEVER `git stash` this repo (~1k uncommitted user-WIP files); pushes to origin require explicit user approval each time.
- Sole permitted cargo failure: `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- macOS cargo wrapper for EVERY cargo invocation: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo ...'`.
- bun test path filters need the `./` prefix (`bun test ./src/test/...`); there is no `timeout` command on this macOS — use the Bash tool's timeout parameter.
- PTY fixtures byte-drift from capture nondeterminism; restore drift with `git checkout -- packages/brain-shell/src/test/fixtures/` and never commit it.
- Vendor scan scope is `crates daemon packages scripts` (docs are excluded deliberately — process docs quote constraint text and trailer strings).

---

### Task 1: Delete the freeze flush + six exact-equality tests

**Files:**
- Modify: `packages/brain-shell/src/state/sessionController.ts:385-391` (`finishTurn` head)
- Test: `packages/brain-shell/src/test/state/sessionControllerFreeze.test.ts`

**Interfaces:**
- Consumes: existing `SessionController` public surface only (`submit`, `abort`, `getSnapshot`, `dispose`) and `BrainBackendClient`/`BrainStreamChunk` types from `../../client/BrainBackendClient.js`.
- Produces: no new exports. Behavior contract changed: frozen assistant markdown equals the exact concatenation of streamed tokens, once, regardless of drain timing. Later increments and all existing tests rely on this silently.

- [ ] **Step 1: Write the failing tests**

Create `packages/brain-shell/src/test/state/sessionControllerFreeze.test.ts`:

```ts
import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

interface StubOpts {
  /** Sleep AFTER yielding a token, letting real ticker ticks drain. */
  postTokenSleepMs?: number;
}

function stubClient(
  chunks: BrainStreamChunk[],
  opts: StubOpts = {},
): BrainBackendClient {
  return {
    async createSession() {
      return { sessionId: 'freeze-probe', title: 't', createdAtMs: 0 };
    },
    async *streamText(_req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      for (const c of chunks) {
        yield c;
        if (c.type === 'token' && opts.postTokenSleepMs) await sleep(opts.postTokenSleepMs);
      }
    },
    // Abort case needs signal awareness; built inline in its own test.
  } as unknown as BrainBackendClient;
}

function stubSession(): { sessionId: string; title: string; createdAtMs: number } {
  return { sessionId: 'freeze-probe', title: 't', createdAtMs: 0 };
}

/** Frozen assistant markdown, joined by '|' if more than one row exists. */
function markdownOf(ctl: SessionController): string {
  return ctl
    .getSnapshot()
    .rows.filter((r) => r.kind === 'assistant')
    .map((r) => r.markdown)
    .join('|');
}

describe('Inc 16: freeze-path renders every delta exactly once', () => {
  it('renders a single instantly-completing token exactly once', async () => {
    const ctl = new SessionController(
      stubClient([{ type: 'token', token: 'abc' }, { type: 'finished', status: 'completed' }]),
    );
    await ctl.submit('q'); // microtasks beat the 16 ms tick: zero drains
    expect(markdownOf(ctl)).toBe('abc');
    ctl.dispose();
  });

  it('renders multi-token instant completion as one exact concatenation', async () => {
    const toks = ['He', 'llo', ' wo', 'rld'];
    const ctl = new SessionController(
      stubClient([
        ...toks.map((token) => ({ type: 'token' as const, token })),
        { type: 'finished', status: 'completed' as const },
      ]),
    );
    await ctl.submit('q');
    expect(markdownOf(ctl)).toBe('Hello world');
    ctl.dispose();
  });

  it('renders a partially drained stream without duplicating its tail', async () => {
    const big = 'y'.repeat(196) + 'TAIL'; // > 32×3 so ticks cannot finish it
    const ctl = new SessionController(
      stubClient(
        [{ type: 'token', token: big }, { type: 'finished', status: 'completed' }],
        { postTokenSleepMs: 50 },
      ),
    );
    await ctl.submit('q');
    expect(markdownOf(ctl)).toBe(big);
    ctl.dispose();
  });

  it('renders error-chunk completion exactly once', async () => {
    const ctl = new SessionController(
      stubClient([
        { type: 'token', token: 'partial' },
        { type: 'error', error: 'v1/generation/stream aborted' }, // abort-classified: no monitor arming
      ]),
    );
    await ctl.submit('q');
    expect(markdownOf(ctl)).toBe('partial');
    ctl.dispose();
  });

  it('renders a real mid-stream abort exactly once', async () => {
    const big = 'z'.repeat(100);
    const ctl = new SessionController({
      async createSession() {
        return stubSession();
      },
      async *streamText(req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
        yield { type: 'token', token: big };
        while (!req.signal.aborted) await sleep(5);
        throw new Error('The operation was aborted');
      },
    } as unknown as BrainBackendClient);
    const turn = ctl.submit('q');
    await sleep(25); // ~0–2 ticks drain some chars into the discarded live view
    ctl.abort();
    await turn;
    expect(markdownOf(ctl)).toBe(big);
    ctl.dispose();
  });

  it('keeps empty responses and thinking-only turns free of phantom rows', async () => {
    const emptyCtl = new SessionController(
      stubClient([{ type: 'finished', status: 'completed' }]),
    );
    await emptyCtl.submit('q');
    expect(emptyCtl.getSnapshot().rows.filter((r) => r.kind === 'assistant').length).toBe(0);
    emptyCtl.dispose();

    const thinkCtl = new SessionController(
      stubClient([
        { type: 'thinking_start' } as unknown as BrainStreamChunk,
        { type: 'thinking', thinking: 'pondering' } as unknown as BrainStreamChunk,
        { type: 'thinking_end', durationMs: 9 } as unknown as BrainStreamChunk,
        { type: 'finished', status: 'completed' },
      ]),
    );
    await thinkCtl.submit('q');
    expect(markdownOf(thinkCtl)).toBe('');
    const think = thinkCtl.getSnapshot().rows.find((r) => r.kind === 'thinking');
    expect(think?.text).toBe('pondering');
    thinkCtl.dispose();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/sessionControllerFreeze.test.ts
```

Expected: **5 fail / 1 pass** — single-token (`'abcabc'`), multi-token (`'Hello worldHello world'`), partial-drain (`<big>` + repeated suffix), error-chunk (`'partialpartial'`), and real-abort cases all fail on duplicated markdown (the abort case fails regardless of tick count: zero ticks duplicate everything, any ticks duplicate the undrained tail); only the empty/thinking guard passes pre-fix.

If any failing message shows something OTHER than duplication (e.g. a missing row), STOP — the mechanism differs from the spec and the plan must be revisited.

- [ ] **Step 3: Implement the minimal fix**

In `packages/brain-shell/src/state/sessionController.ts`, `finishTurn` currently begins:

```ts
  private finishTurn(status: 'completed' | 'error', errorText?: string): void {
    this.stopTicker();
    // Flush any undrained typewriter text so frozen rows carry the whole answer.
    const remainder = this.queue.pending > 0 ? this.queue.drain(this.queue.pending) : '';
    if (remainder.length > 0) {
      this.events.push({ type: 'text_delta', delta: remainder });
    }
```

Replace those five lines after `this.stopTicker();` with:

```ts
  private finishTurn(status: 'completed' | 'error', errorText?: string): void {
    this.stopTicker();
    // Frozen rows are built solely from `events`, which already holds every
    // delta verbatim (handleChunk records them before queueing for pacing).
    // The typewriter queue feeds only the live view, which freeze discards —
    // flushing it here would re-push its pending tail (Inc 16 dedup fix).
```

Nothing else in the method changes: tool settlement, `turn_error`/`thinking_end`/`turn_complete` pushes, transform, and projection stay byte-identical. No other file is touched.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/sessionControllerFreeze.test.ts
```

Expected: **6 pass / 0 fail.** Then run the full suite — the fix must regress nothing:

```bash
bun test
```

Expected: previous baseline plus these 6 → **280 tests / 275 pass / 5 fail**, where the 5 failures are exactly the documented pre-existing set (visualCellParity ×2, sessionSemanticIntegration, brainMemoryIntegration, brainTurnTransformer Scenario 8).

Also run the Inc 15 controller tests explicitly since they assert on frozen rows:

```bash
bun test ./src/test/state/sessionControllerReconnect.test.ts
```

Expected: 3 pass / 0 fail unchanged (their assertions are `includes()`-based and stay true under exact-once rendering).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionControllerFreeze.test.ts
git commit -m "fix(shell): render frozen rows without typewriter tail duplication

Inc 16: handleChunk already records every text_delta in events before
queueing it for live-view pacing, so finishTurn's remainder flush could
only ever re-push pending characters — duplicating the answer's tail
whenever the turn ended with undrained buffer (instant completions, fast
daemons, aborts). Frozen rows now come solely from events; the two-stage
typewriter keeps its semantics untouched as a pacing-only projection.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Full gates + finishing

**Files:** none created — verification only.

**Interfaces:**
- Consumes: everything.
- Produces: gate evidence for the finishing-a-development-branch menu.

- [ ] **Step 1: Full bun suite**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test
```

Expected: **280 tests / 275 pass / 5 fail** — the documented five only.

- [ ] **Step 2: tsc touched-file parity vs pristine main**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bunx tsc --noEmit > "$CLAUDE_JOB_DIR/tmp/inc16-tsc.log" 2>&1; echo "exit:$?"
sed $'s/\x1b\[[0-9;]*m//g' "$CLAUDE_JOB_DIR/tmp/inc16-tsc.log" \
  | grep -E "^src/(state/sessionController|test/state/sessionControllerFreeze)\.tsx?" \
  | grep -oE "error TS[0-9]+" | sort | uniq -c
```

Expected for the branch log: ambient classes only on the new test file (`TS2591` node typedefs / `TS2307` bun:test+bun modules — the classes every state test shows) and NOTHING on `sessionController.ts` beyond what pristine main shows. Then prove parity with the worktree probe:

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git worktree add --detach "$CLAUDE_JOB_DIR/tmp/inc16-probe" origin/main
ln -s /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/node_modules \
  "$CLAUDE_JOB_DIR/tmp/inc16-probe/packages/brain-shell/node_modules"
cd "$CLAUDE_JOB_DIR/tmp/inc16-probe/packages/brain-shell"
bunx tsc --noEmit > "$CLAUDE_JOB_DIR/tmp/inc16-tsc-main.log" 2>&1
for f in inc16-tsc-main.log inc16-tsc.log; do
  echo "== $f =="
  sed $'s/\x1b\[[0-9;]*m//g' "$CLAUDE_JOB_DIR/tmp/$f" \
    | grep -E "^src/state/sessionController\.ts" \
    | grep -oE "error TS[0-9]+" | sort | uniq -c
done
cd /Users/ritikpathania/Developer/PyCharm/brain
rm "$CLAUDE_JOB_DIR/tmp/inc16-probe/packages/brain-shell/node_modules"
git worktree remove --force "$CLAUDE_JOB_DIR/tmp/inc16-probe"
git worktree prune
```

Expected: identical counts both sides for `sessionController.ts` (pristine main has none — the deletion adds none).

- [ ] **Step 3: Vendor scan on the increment diff**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
BASE=$(git merge-base HEAD origin/main)
git diff "$BASE"..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"
```

Expected: `0`. If nonzero, inspect: docs paths are excluded by design; any match inside `packages/` must be justified line-by-line before proceeding (expected source: none).

- [ ] **Step 4: Cargo workspace (Rust untouched, prove it anyway)**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --workspace --no-fail-fast' 2>&1 | grep -E "test result: FAILED|failures:" -A2 | grep -vE "^--" | sort | uniq -c
```

Expected: exactly ONE failed suite — the sole permitted `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.

- [ ] **Step 5: Regression smokes**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
rm -f /tmp/brain-inc15-smoke.sock
python3 scripts/ptySmokeInc15.py
python3 scripts/ptySmokeInc6.py
```

Expected: inc15 all-PASS exit 0 (its replay-answer assertion stays true under exact-once rendering); inc6 all 16 assertions PASS exit 0. Afterwards check fixture drift and restore it — never commit it:

```bash
git checkout -- packages/brain-shell/src/test/fixtures/
git status --porcelain packages/brain-shell/src/test/fixtures/
```

Expected: empty output from the final status call.

- [ ] **Step 6: Finishing**

Announce finishing-a-development-branch, verify tests (Steps 1–5 ARE the verification), detect environment (normal repo — standard menu), base branch `main`, and present exactly:

```
Implementation complete. What would you like to do?

1. Merge back to main locally
2. Push and create a Pull Request
3. Keep the branch as-is (I'll handle it later)

Which option?
```

On Option 1: `git checkout main && git pull --ff-only && git merge feature/brain-shell-inc16-freeze-dedup`, confirm `git rev-parse main` equals the branch tip hash, rerun the bun suite once as post-merge sanity, delete the branch with `git branch -d feature/brain-shell-inc16-freeze-dedup`, report `[ahead N]` state. Pushes require explicit user approval.

---

## Self-Review (completed during planning)

1. **Spec coverage:** §1 root cause → Task 1 Step 3 deletes exactly lines 388–390 plus comment; §2 invariant → asserted by construction and pinned by all six tests; §4 testing strategy → cases 1–6 map one-to-one onto the six `it()` blocks (single-token, multi-token, partial-drain, error-chunk, real abort, empty+thinking guards); §4 gates → Task 2 Steps 1–5; §5 non-goals respected — no queue/transformer/UI edits anywhere in the plan; §6 constraints copied into Global Constraints verbatim. No gaps.
2. **Placeholder scan:** all steps carry full code or exact commands; no TBD/TODO/"similar to" anywhere. (A draft of test 1 used a self-comparing expectation to advertise the pre-fix failure mode; replaced inline with the plain `.toBe('abc')` — Step 2's expected failure output documents that mode instead.)
3. **Type consistency:** test imports match the real module layout (`state/sessionController.js`, `client/BrainBackendClient.js`); `BrainStreamChunk` literal shapes verified against `chunkToTurnEvents.ts` (wire thinking shape is `{type:'thinking', thinking}` — no `thinking_delta` wire type); stub `createSession` return matches `BrainSessionSummary` usage elsewhere; `markdownOf` helper used identically in every case; commit paths match created/modified files exactly.
