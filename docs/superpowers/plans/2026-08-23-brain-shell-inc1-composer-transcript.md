# Brain Shell Increment 1 — Composer + Transcript Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the interactive turn loop — `AppShell` static/live split, `PromptInput` (prompt/`!` modes, history, paste truncation, undo), `MessageRow` dispatch (user / assistant / thinking / tool card collapsed→expanded / error), a markdown renderer, `Spinner`, and typewriter drain wired to the real daemon stream over UDS.

**Architecture:** One-way data flow per spec §6: UDS chunks (`client/`) → pure mapper (`adapter/chunkToTurnEvents.ts`) → `BrainTurnEvent[]` → existing `BrainTurnTransformer` → `BrainTurnViewModel` → frozen `TranscriptRow[]`; text deltas additionally buffer in a two-stage typewriter queue so network completion stays decoupled from drain cadence (AGENTS.md pipeline). React reads state through `useSyncExternalStore` off a non-React `SessionController`; UI actions flow back through adapter/client seams only.

**Tech Stack:** Bun 1.4.0, TypeScript ESM, stock Ink 7 via `src/compat/index.js`, React 19 (`useSyncExternalStore`), `bun:test`. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-08-23-brain-shell-contracts-first-design.md` §5 row "Increment 1", §6 data flow, §7 error handling, §8 testing strategy.

## Global Constraints

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, adapter boundaries. `client/` and existing `adapter/` files stay unmodified except where a task explicitly says otherwise (this plan adds two new adapter files and one new contract file — it edits zero existing adapter/client files).
- No Claude/Anthropic models, APIs, auth, pricing, billing, or LLM product concepts. No vendor-tree references anywhere: the reference tree at `/Users/ritikpathania/Developer/claude-code` is read-only archaeology outside the repo.
- Every commit contains only explicitly-added paths (never `git add -A` on broad dirs; stage each file).
- All colors go through `BrainTokens` from `useTheme()` — no raw hex/ansi. Rounded-border panels where panels are used; SIGWINCH-safe layout via flexbox + `width={columns}`; compact-width graceful behavior.
- Unit-test constraint: `bun test` cannot pump the React 19 scheduler — mounted Ink trees render empty. Test pattern = pure `XView(props)` functions asserted via element-tree text extraction (see `src/test/contracts/shell.test.tsx` for the proven walker). Live rendering is verified only through PTY smoke.
- PTY harness: `pty.fork()` defaults to 0×0 — must set winsize via `fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))` or `useTerminalSize()` collapses. Strip ANSI before substring assertions.
- Bun bundler nondeterminism ("vendor storm"): if `bun build` suddenly walks dead graphs, purge the FULL cache `rm -rf ~/Library/Caches/bun` (partial purges do not fix it) and rebuild.
- Baseline rule: `bun test` currently has 5 documented baseline failures. Zero NEW failures is the bar; record counts before/after every task.
- Entry-point ordering: `main.tsx` keeps its static `import './preload.js'` first line (NODE_ENV must be set before react build selection).
- Working dir: `packages/brain-shell/` unless stated. Repo root is `/Users/ritikpathania/Developer/PyCharm/brain`. Shell cwd resets between tool calls — use absolute paths or explicit `cd` chains.

## Branch

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git checkout main && git pull 2>/dev/null; git checkout -b inc1-composer-transcript
```

## File Structure (new/modified)

```
packages/brain-shell/src/
├── contracts/streaming.ts            NEW   StreamPhase, LiveStreamView, TypewriterQueue contract
├── contracts/messages.ts             MODIFY append TranscriptRow union (dependency-free)
├── adapter/streaming/TwoStageTypewriterQueue.ts  NEW  queue implementation + cadence constants
├── adapter/chunkToTurnEvents.ts      NEW   BrainStreamChunk → BrainTurnEvent|null (pure)
├── state/sessionController.ts        NEW   turn-loop owner; ShellSnapshot; useSyncExternalStore source
├── ui/composer/composerState.ts      NEW   pure reducer: editing, undo, history nav
├── ui/composer/paste.ts              NEW   large-paste truncation + expansion (pure)
├── ui/composer/translateKey.ts       NEW   (input, key) → KeyCommand (pure)
├── ui/composer/historyStore.ts       NEW   ~/.brain/history.jsonl persistence (100 entries, newest-first)
├── ui/composer/PromptInput.tsx       NEW   PromptInputView (pure) + PromptInput (interactive)
├── ui/transcript/markdown.ts         NEW   markdown → styled segments (pure subset renderer)
├── ui/transcript/Markdown.tsx        NEW   segment renderer bound to theme tokens
├── ui/transcript/toRows.ts           NEW   BrainTurnViewModel → TranscriptRow[]
├── ui/transcript/MessageRow.tsx      NEW   row dispatch + UserRow/AssistantRow/ThinkingRow/ToolRow/ErrorRow views
├── ui/shell/useShellSnapshot.ts      NEW   useSyncExternalStore binding
├── ui/shell/Spinner.tsx              NEW   SpinnerView (pure) + Spinner (timed)
├── ui/shell/AppShell.tsx             NEW   static transcript region + live region + composer + footer
├── ui/shell/AppSkeleton.tsx          DELETE (replaced by AppShell)
├── main.tsx                          MODIFY mount AppShell instead of AppSkeleton
├── test/architectureFitness.test.ts  MODIFY expect AppShell instead of AppSkeleton
└── test/
    ├── contracts/streamingQueue.test.ts   NEW
    ├── ui/composerState.test.ts           NEW
    ├── ui/promptInputView.test.ts         NEW
    ├── ui/markdown.test.ts                NEW
    ├── ui/toRows.test.ts                  NEW
    ├── ui/messageRowView.test.ts          NEW
    ├── adapter/chunkToTurnEvents.test.ts  NEW
    ├── state/sessionController.test.ts    NEW
    └── ui/spinnerView.test.ts             NEW
scripts/ptySmokeInc1.py                 NEW   PTY smoke: launch / mid-stream / expanded tool card / bash-strip
src/test/fixtures/pty/inc1/*.txt        NEW   recorded fixtures (artifacts, not golden oracles)
```

---

### Task 1: Streaming contract + TwoStageTypewriterQueue

**Files:**
- Create: `src/contracts/streaming.ts`
- Create: `src/adapter/streaming/TwoStageTypewriterQueue.ts`
- Test: `src/test/contracts/streamingQueue.test.ts`

**Interfaces:**
- Consumes: nothing (leaf module).
- Produces (later tasks import these):
  - `export type StreamPhase = 'idle' | 'thinking' | 'responding' | 'tool' | 'error'`
  - `export interface LiveStreamView { phase: StreamPhase; thinkingText: string; responseText: string; activeToolName?: string; errorText?: string }`
  - `export interface TypewriterQueue { push(text: string): void; end(): void; drain(maxChars: number): string; readonly pending: number }`
  - `export const TYPEWRITER_TICK_MS = 16`, `export const TYPEWRITER_CHARS_PER_TICK = 32` (from TwoStageTypewriterQueue.ts)

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/streamingQueue.test.ts
import { describe, it, expect } from 'bun:test';
import { TwoStageTypewriterQueue } from '../../adapter/streaming/TwoStageTypewriterQueue.js';

describe('TwoStageTypewriterQueue', () => {
  it('buffers pushes and releases up to maxChars per drain', () => {
    const q = new TwoStageTypewriterQueue();
    q.push('hello ');
    q.push('world');
    expect(q.pending).toBe(11);
    expect(q.drain(6)).toBe('hello ');
    expect(q.pending).toBe(5);
    expect(q.drain(100)).toBe('world');
    expect(q.pending).toBe(0);
    expect(q.drain(10)).toBe('');
  });

  it('end() does not auto-release but marks completion; drains still work after end()', () => {
    const q = new TwoStageTypewriterQueue();
    q.push('abc');
    q.end();
    expect(q.pending).toBe(3);          // completion is decoupled from drain
    expect(q.drain(3)).toBe('abc');
    expect(q.pending).toBe(0);
  });

  it('handles multibyte-safe slicing by code units (ASCII contract)', () => {
    // Drain operates on UTF-16 code units; emoji may split across drains.
    // Contract: callers treat released text as opaque concatenation.
    const q = new TwoStageTypewriterQueue();
    q.push('a✻b');
    expect(q.drain(2) + q.drain(2)).toBe('a✻b');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/contracts/streamingQueue.test.ts`
Expected: FAIL — cannot resolve `../../adapter/streaming/TwoStageTypewriterQueue.js`.

- [ ] **Step 3: Write minimal implementation**

```ts
// src/contracts/streaming.ts
/**
 * Stream presentation contract: live-region view-model shape and the
 * typewriter queue seam that decouples network completion from drain cadence.
 * Dependency-free (no transport, no React, no vendor).
 */

export type StreamPhase = 'idle' | 'thinking' | 'responding' | 'tool' | 'error';

export interface LiveStreamView {
  phase: StreamPhase;
  /** Accumulated thinking text for the active turn (dim preview above the response). */
  thinkingText: string;
  /** Typewriter-released response text for the active turn. */
  responseText: string;
  /** Tool name while phase === 'tool'. */
  activeToolName?: string;
  errorText?: string;
}

/** Two-stage typewriter queue: stage 1 buffers network deltas; stage 2 drains on cadence. */
export interface TypewriterQueue {
  push(text: string): void;
  /** Network side signals completion; does not itself release anything. */
  end(): void;
  /** Release at most maxChars buffered characters; '' when nothing pending. */
  drain(maxChars: number): string;
  readonly pending: number;
}
```

```ts
// src/adapter/streaming/TwoStageTypewriterQueue.ts
import type { TypewriterQueue } from '../../contracts/streaming.js';

/** Drain cadence used by SessionController's interval. */
export const TYPEWRITER_TICK_MS = 16;
export const TYPEWRITER_CHARS_PER_TICK = 32;

/** FIFO char-buffer queue implementing the contracts/streaming.ts seam. */
export class TwoStageTypewriterQueue implements TypewriterQueue {
  private buffer = '';

  get pending(): number {
    return this.buffer.length;
  }

  push(text: string): void {
    if (text.length > 0) this.buffer += text;
  }

  end(): void {
    // Completion is tracked by the caller (turn loop); the queue only holds text.
  }

  drain(maxChars: number): string {
    if (maxChars <= 0 || this.buffer.length === 0) return '';
    const out = this.buffer.slice(0, maxChars);
    this.buffer = this.buffer.slice(maxChars);
    return out;
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/contracts/streamingQueue.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/contracts/streaming.ts packages/brain-shell/src/adapter/streaming/TwoStageTypewriterQueue.ts packages/brain-shell/src/test/contracts/streamingQueue.test.ts && \
git commit -m "feat(shell): streaming contract + two-stage typewriter queue

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Composer input state machine (pure reducer, paste, key translation, history store)

**Files:**
- Create: `src/ui/composer/composerState.ts`
- Create: `src/ui/composer/paste.ts`
- Create: `src/ui/composer/translateKey.ts`
- Create: `src/ui/composer/historyStore.ts`
- Test: `src/test/ui/composerState.test.ts`

**Interfaces:**
- Consumes: `PromptInputMode` from `src/contracts/input.js`.
- Produces:
  - `modeOf(value: string): PromptInputMode` — `'bash'` iff value starts with `'!'`, else `'prompt'`.
  - `paste.ts`: `TRUNCATION_THRESHOLD = 10_000`; `interface StoredPaste { id: string; content: string }`; `processPaste(text: string, pasteCounter: number): { inserted: string; stored?: StoredPaste; nextCounter: number }`; `expandPastedPlaceholders(value: string, pastedContents: Record<string, string>): string` — placeholder format `` `[Pasted text #N +<lines> lines]` ``.
  - `composerState.ts`: `ComposerState`, `createComposerState(history?: HistoryEntry[]): ComposerState`, `ComposerAction` union, `reduceComposer(state, action): ComposerState`, `wordBackStart(value, cursor): number`, `expandedValue(state): string` (submit-time placeholder expansion).
    - Actions: `{type:'insert',text}` `{type:'newline'}` `{type:'backspace'}` `{type:'delete'}` `{type:'left'|'right'|'home'|'end'}` `{type:'kill_to_end'|'kill_to_start'|'delete_word_back'}` `{type:'undo'}` `{type:'history_up'|'history_down'}` `{type:'submit_done', entry: HistoryEntry}`.
    - State fields: `value`, `cursor`, `pastedContents`, `pasteCounter`, `undoStack` (max 50 snapshots `{value,cursor}`), `history: HistoryEntry[]` (newest-first), `historyIndex` (-1 when editing), `historyDraft`.
  - `translateKey.ts`: `KeyInfo { upArrow?,downArrow?,leftArrow?,rightArrow?,return?,escape?,ctrl?,meta?,shift?,backspace?,delete? }`, `KeyCommand` union (see Step 3), `translateKey(input: string, key: KeyInfo): KeyCommand`.
  - `historyStore.ts`: `HistoryEntry { mode: PromptInputMode; value: string }`, `HISTORY_MAX_ITEMS = 100`, `historyPath(): string` (`~/.brain/history.jsonl`), `loadHistory(): HistoryEntry[]` (sync, [] on any error), `appendHistory(entry: HistoryEntry): void` (dedupe consecutive duplicates, cap 100, atomic tmp+rename write).

- [ ] **Step 1: Write the failing test**

```ts
// src/test/ui/composerState.test.ts
import { describe, it, expect } from 'bun:test';
import {
  createComposerState, reduceComposer, modeOf, wordBackStart, expandedValue,
} from '../../ui/composer/composerState.js';
import { processPaste, expandPastedPlaceholders } from '../../ui/composer/paste.js';
import { translateKey } from '../../ui/composer/translateKey.js';

const HIST = [
  { mode: 'prompt' as const, value: 'second prompt' },
  { mode: 'prompt' as const, value: 'first prompt' },
  { mode: 'bash' as const, value: 'ls -la' },
];

describe('composer modes', () => {
  it('derives mode from leading bang like the reference inputModes contract', () => {
    expect(modeOf('')).toBe('prompt');
    expect(modeOf('hi')).toBe('prompt');
    expect(modeOf('!ls')).toBe('bash');
    expect(modeOf('! ls')).toBe('bash');
    expect(modeOf("don't!")).toBe('prompt');   // only position 0 counts
  });
});

describe('editing actions', () => {
  it('inserts at cursor and moves it', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'ab' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'insert', text: 'X' });
    expect(s.value).toBe('aXb');
    expect(s.cursor).toBe(2);
  });

  it('backspace/delete respect boundaries without pushing no-op undo entries', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'ab' });
    const depth = s.undoStack.length;
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'delete' });            // removes 'b'
    expect(s.value).toBe('a');
    s = reduceComposer(s, { type: 'backspace' });         // removes 'a'
    expect(s.value).toBe('');
    s = reduceComposer(s, { type: 'backspace' });         // boundary no-op
    expect(s.value).toBe('');
    expect(s.undoStack.length).toBeLessThanOrEqual(depth + 2);
  });

  it('kills to start/end and deletes word-back', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'one two three' });
    s = reduceComposer(s, { type: 'kill_to_start' });
    expect(s.value).toBe('');
    expect(s.cursor).toBe(0);

    s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'one two three' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });              // cursor before 'three'
    s = reduceComposer(s, { type: 'delete_word_back' });  // kills 'two '
    expect(s.value).toBe('one three');

    s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'keep me' });
    s = reduceComposer(s, { type: 'home' });
    s = reduceComposer(s, { type: 'kill_to_end' });
    expect(s.value).toBe('');
  });

  it('wordBackStart skips trailing spaces then the word run', () => {
    expect(wordBackStart('foo bar   ', 10)).toBe(4);
    expect(wordBackStart('foo bar', 7)).toBe(4);
    expect(wordBackStart('foo', 3)).toBe(0);
    expect(wordBackStart('', 0)).toBe(0);
  });

  it('undo restores previous snapshot and can hit empty-stack floor', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'undo' });               // empty stack no-op
    expect(s.value).toBe('');
    s = reduceComposer(s, { type: 'insert', text: 'v1' });
    s = reduceComposer(s, { type: 'insert', text: '-v2' });
    s = reduceComposer(s, { type: 'undo' });
    expect(s.value).toBe('v1');
    s = reduceComposer(s, { type: 'undo' });
    expect(s.value).toBe('');
  });
});

describe('large-paste truncation', () => {
  it('passes small pastes through unchanged', () => {
    const r = processPaste('short', 0);
    expect(r.inserted).toBe('short');
    expect(r.stored).toBeUndefined();
    expect(r.nextCounter).toBe(0);
  });

  it('replaces huge pastes with a counted placeholder and stores full text', () => {
    const big = Array.from({ length: 900 }, (_, i) => `line-${i}`).join('\n');
    const r = processPaste(big, 3);
    expect(r.nextCounter).toBe(4);
    expect(r.stored?.id).toBe('paste_4');
    expect(r.stored?.content).toBe(big);
    expect(r.inserted).toBe(`[Pasted text #4 +${big.split('\n').length} lines]`);
    const round = expandPastedPlaceholders(`${r.inserted} tail`, { paste_4: big });
    expect(round).toBe(`${big} tail`);
  });

  it('expansion leaves unknown placeholders alone', () => {
    const v = '[Pasted text #9 +5 lines]';
    expect(expandPastedPlaceholders(v, {})).toBe(v);
  });
});

describe('history navigation', () => {
  it('up jumps to newest matching-mode entry, walks older, down restores draft', () => {
    let s = createComposerState(HIST);
    s = reduceComposer(s, { type: 'insert', text: 'dra' });
    s = reduceComposer(s, { type: 'history_up' });
    expect(s.value).toBe('second prompt');     // newest prompt-mode entry
    expect(s.historyDraft).toBe('dra');
    s = reduceComposer(s, { type: 'history_up' });
    expect(s.value).toBe('first prompt');
    s = reduceComposer(s, { type: 'history_up' });
    expect(s.value).toBe('first prompt');      // oldest prompt entry clamps
    s = reduceComposer(s, { type: 'history_down' });
    expect(s.value).toBe('second prompt');
    s = reduceComposer(s, { type: 'history_down' });
    expect(s.value).toBe('dra');               // draft restored, index back to -1
    expect(s.historyIndex).toBe(-1);
  });

  it('filters by browse mode captured at start (bang switches to bash entries)', () => {
    let s = createComposerState(HIST);
    s = reduceComposer(s, { type: 'insert', text: '!' });
    s = reduceComposer(s, { type: 'history_up' });
    // History stores bare submitted values; browsing restores them bare.
    // The captured browse mode keeps subsequent Up/Down in bash entries.
    expect(s.value).toBe('ls -la');
    expect(s.historyBrowseMode).toBe('bash');
  });

  it('submit_done resets the buffer and records newest-first without dupes', () => {
    let s = createComposerState(HIST);
    s = reduceComposer(s, { type: 'insert', text: 'brand new' });
    s = reduceComposer(s, { type: 'submit_done', entry: { mode: 'prompt', value: 'brand new' } });
    expect(s.value).toBe('');
    expect(s.cursor).toBe(0);
    expect(s.historyIndex).toBe(-1);
    expect(s.history[0]).toEqual({ mode: 'prompt', value: 'brand new' });
    s = reduceComposer(s, { type: 'submit_done', entry: { mode: 'prompt', value: 'brand new' } });
    expect(s.history.filter((e) => e.value === 'brand new')).toHaveLength(1);
  });
});

describe('key translation', () => {
  it('maps navigation/editing keys to commands', () => {
    expect(translateKey('', { upArrow: true })).toEqual({ type: 'history_up' });
    expect(translateKey('', { downArrow: true })).toEqual({ type: 'history_down' });
    expect(translateKey('', { leftArrow: true })).toEqual({ type: 'left' });
    expect(translateKey('', { rightArrow: true })).toEqual({ type: 'right' });
    expect(translateKey('', { return: true })).toEqual({ type: 'submit' });
    expect(translateKey('', { return: true, shift: true })).toEqual({ type: 'newline' });
    expect(translateKey('', { backspace: true })).toEqual({ type: 'backspace' });
    expect(translateKey('', { delete: true })).toEqual({ type: 'backspace' });
    expect(translateKey('a', {})).toEqual({ type: 'insert', text: 'a' });
    expect(translateKey('a', { ctrl: true })).toEqual({ type: 'home' });
    expect(translateKey('e', { ctrl: true })).toEqual({ type: 'end' });
    expect(translateKey('k', { ctrl: true })).toEqual({ type: 'kill_to_end' });
    expect(translateKey('u', { ctrl: true })).toEqual({ type: 'kill_to_start' });
    expect(translateKey('w', { ctrl: true })).toEqual({ type: 'delete_word_back' });
    expect(translateKey('z', { ctrl: true })).toEqual({ type: 'undo' });
    expect(translateKey('_', { ctrl: true })).toEqual({ type: 'undo' });
    expect(translateKey('', { escape: true })).toEqual({ type: 'abort' });
    expect(translateKey('c', { ctrl: true })).toEqual({ type: 'exit' });
  });

  it('expandedValue joins stored pastes into submitted text', () => {
    const s = createComposerState();
    const p = processPaste('BIG\nTEXT', 0);
    let t = reduceComposer(s, { type: 'insert', text: `${p.inserted}!` });
    t = { ...t, pastedContents: { ...(t.pastedContents ?? {}), ...(p.stored ? { [p.stored.id]: p.stored.content } : {}) } };
    expect(expandedValue(t)).toBe('BIG\nTEXT!');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/composerState.test.ts`
Expected: FAIL — cannot resolve `../../ui/composer/composerState.js`.

- [ ] **Step 3: Write minimal implementation**

```ts
// src/ui/composer/paste.ts
/**
 * Large-paste handling (reference inputPaste contract, Brain-branded copy):
 * ≥ TRUNCATION_THRESHOLD chars collapse to a short counted placeholder; the
 * full text rides along in pastedContents and expands back at submit time.
 */

export const TRUNCATION_THRESHOLD = 10_000;

export interface StoredPaste {
  id: string;
  content: string;
}

export function placeholderFor(idNumber: number, content: string): string {
  const lines = content.split('\n').length;
  return `[Pasted text #${idNumber} +${lines} lines]`;
}

export function processPaste(
  text: string,
  pasteCounter: number,
): { inserted: string; stored?: StoredPaste; nextCounter: number } {
  if (text.length < TRUNCATION_THRESHOLD) {
    return { inserted: text, nextCounter: pasteCounter };
  }
  const idNumber = pasteCounter + 1;
  return {
    inserted: placeholderFor(idNumber, text),
    stored: { id: `paste_${idNumber}`, content: text },
    nextCounter: idNumber,
  };
}

const PLACEHOLDER_RE = /\[Pasted text #(\d+) \+\d+ lines\]/g;

export function expandPastedPlaceholders(
  value: string,
  pastedContents: Record<string, string>,
): string {
  return value.replace(PLACEHOLDER_RE, (match, num: string) => {
    const stored = pastedContents[`paste_${num}`];
    return stored !== undefined ? stored : match;
  });
}
```

```ts
// src/ui/composer/historyStore.ts
/**
 * Prompt history persistence: ~/.brain/history.jsonl, one JSON entry per line
 * ({mode,value}), newest LAST on disk, loaded reversed so index 0 is newest.
 */
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import type { PromptInputMode } from '../../contracts/input.js';

export const HISTORY_MAX_ITEMS = 100;

export interface HistoryEntry {
  mode: PromptInputMode;
  value: string;
}

export function historyPath(): string {
  return path.join(os.homedir(), '.brain', 'history.jsonl');
}

export function loadHistory(): HistoryEntry[] {
  try {
    const raw = fs.readFileSync(historyPath(), 'utf8');
    const entries: HistoryEntry[] = [];
    for (const line of raw.split('\n')) {
      if (!line.trim()) continue;
      try {
        const e = JSON.parse(line) as Partial<HistoryEntry>;
        if ((e.mode === 'prompt' || e.mode === 'bash') && typeof e.value === 'string') {
          entries.push({ mode: e.mode, value: e.value });
        }
      } catch {}
    }
    return entries.reverse(); // disk oldest→newest; memory newest-first
  } catch {
    return [];
  }
}

export function appendHistory(entry: HistoryEntry): void {
  try {
    const existing = loadHistory().reverse(); // oldest→newest
    const last = existing[existing.length - 1];
    if (last && last.mode === entry.mode && last.value === entry.value) return;
    const next = [...existing, entry].slice(-HISTORY_MAX_ITEMS);
    const file = historyPath();
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const body = next.map((e) => JSON.stringify(e)).join('\n') + '\n';
    const tmp = `${file}.tmp`;
    fs.writeFileSync(tmp, body, 'utf8');
    fs.renameSync(tmp, file);
  } catch {
    // History is best-effort; never surface I/O errors into the UI loop.
  }
}
```

```ts
// src/ui/composer/translateKey.ts
/** Pure keybinding translation: ink's (input, key) → editor command. */

export interface KeyInfo {
  upArrow?: boolean;
  downArrow?: boolean;
  leftArrow?: boolean;
  rightArrow?: boolean;
  return?: boolean;
  escape?: boolean;
  ctrl?: boolean;
  meta?: boolean;
  shift?: boolean;
  backspace?: boolean;
  delete?: boolean;
}

export type KeyCommand =
  | { type: 'insert'; text: string }
  | { type: 'backspace' }
  | { type: 'left' }
  | { type: 'right' }
  | { type: 'home' }
  | { type: 'end' }
  | { type: 'kill_to_end' }
  | { type: 'kill_to_start' }
  | { type: 'delete_word_back' }
  | { type: 'undo' }
  | { type: 'history_up' }
  | { type: 'history_down' }
  | { type: 'newline' }
  | { type: 'submit' }
  | { type: 'abort' }
  | { type: 'exit' }
  | { type: 'noop' };

export function translateKey(input: string, key: KeyInfo): KeyCommand {
  if (key.escape) return { type: 'abort' };
  if (key.return) {
    return key.shift ? { type: 'newline' } : { type: 'submit' };
  }
  if (key.upArrow) return { type: 'history_up' };
  if (key.downArrow) return { type: 'history_down' };
  if (key.leftArrow) return { type: 'left' };
  if (key.rightArrow) return { type: 'right' };
  if (key.backspace || key.delete) return { type: 'backspace' };
  if (key.ctrl) {
    switch (input) {
      case 'a': return { type: 'home' };
      case 'e': return { type: 'end' };
      case 'k': return { type: 'kill_to_end' };
      case 'u': return { type: 'kill_to_start' };
      case 'w': return { type: 'delete_word_back' };
      case 'z':
      case '_': return { type: 'undo' };
      case 'c': return { type: 'exit' };
      default: break;
    }
  }
  if (input && !key.ctrl && !key.meta) return { type: 'insert', text: input };
  return { type: 'noop' };
}
```

```ts
// src/ui/composer/composerState.ts
/**
 * Pure composer reducer: editing buffer, cursor ops, undo stack, large-paste
 * truncation, and per-mode history navigation. No I/O, no React — fully
 * unit-testable; PromptInput binds translateKey → reduceComposer.
 */
import type { PromptInputMode } from '../../contracts/input.js';
import { processPaste, expandPastedPlaceholders } from './paste.js';
import type { HistoryEntry } from './historyStore.js';

export type { HistoryEntry } from './historyStore.js';

export interface ComposerSnapshot {
  value: string;
  cursor: number;
}

export interface ComposerState extends ComposerSnapshot {
  pastedContents: Record<string, string>;
  pasteCounter: number;
  undoStack: ComposerSnapshot[];
  history: HistoryEntry[];      // newest-first
  historyIndex: number;         // -1 while composing
  historyDraft: string;
  /** Mode captured when browsing began; keeps '!'-started browsing in bash entries. */
  historyBrowseMode?: PromptInputMode;
}

export type ComposerAction =
  | { type: 'insert'; text: string }
  | { type: 'newline' }
  | { type: 'backspace' }
  | { type: 'left' }
  | { type: 'right' }
  | { type: 'home' }
  | { type: 'end' }
  | { type: 'kill_to_end' }
  | { type: 'kill_to_start' }
  | { type: 'delete_word_back' }
  | { type: 'undo' }
  | { type: 'history_up' }
  | { type: 'history_down' }
  | { type: 'submit_done'; entry: HistoryEntry };

const UNDO_LIMIT = 50;

export function createComposerState(history: HistoryEntry[] = []): ComposerState {
  return {
    value: '',
    cursor: 0,
    pastedContents: {},
    pasteCounter: 0,
    undoStack: [],
    history,
    historyIndex: -1,
    historyDraft: '',
  };
}

export function modeOf(value: string): PromptInputMode {
  return value.startsWith('!') ? 'bash' : 'prompt';
}

export function wordBackStart(value: string, cursor: number): number {
  let i = cursor;
  while (i > 0 && /\s/.test(value[i - 1]!)) i--;
  while (i > 0 && !/\s/.test(value[i - 1]!)) i--;
  return i;
}

function pushUndo(state: ComposerState, prev: ComposerState): ComposerState {
  return {
    ...state,
    undoStack: [...state.undoStack.slice(-(UNDO_LIMIT - 1)), { value: prev.value, cursor: prev.cursor }],
  };
}

/** Submit-time view of the buffer: placeholders replaced with their full text. */
export function expandedValue(state: ComposerState): string {
  return expandPastedPlaceholders(state.value, state.pastedContents);
}

function insertRaw(state: ComposerState, text: string): ComposerState {
  const result = processPaste(text, state.pasteCounter);
  const pastedContents =
    result.stored !== undefined
      ? { ...state.pastedContents, [result.stored.id]: result.stored.content }
      : state.pastedContents;
  return {
    ...state,
    value: state.value.slice(0, state.cursor) + result.inserted + state.value.slice(state.cursor),
    cursor: state.cursor + result.inserted.length,
    pastedContents,
    pasteCounter: result.nextCounter,
  };
}

function candidatesFor(state: ComposerState, mode: PromptInputMode): number[] {
  const idx: number[] = [];
  state.history.forEach((entry, i) => {
    if (entry.mode === mode) idx.push(i);
  });
  return idx;
}

export function reduceComposer(state: ComposerState, action: ComposerAction): ComposerState {
  switch (action.type) {
    case 'insert': {
      if (action.text.length === 0) return state;
      return pushUndo(insertRaw(state, action.text), state);
    }
    case 'newline':
      return pushUndo(insertRaw(state, '\n'), state);
    case 'backspace': {
      if (state.cursor === 0) return state;
      return pushUndo(
        {
          ...state,
          value: state.value.slice(0, state.cursor - 1) + state.value.slice(state.cursor),
          cursor: state.cursor - 1,
        },
        state,
      );
    }
    case 'left':
      return { ...state, cursor: Math.max(0, state.cursor - 1) };
    case 'right':
      return { ...state, cursor: Math.min(state.value.length, state.cursor + 1) };
    case 'home':
      return { ...state, cursor: 0 };
    case 'end':
      return { ...state, cursor: state.value.length };
    case 'kill_to_end':
      if (state.cursor >= state.value.length) return state;
      return pushUndo({ ...state, value: state.value.slice(0, state.cursor) }, state);
    case 'kill_to_start': {
      if (state.cursor === 0) return state;
      return pushUndo({ ...state, value: state.value.slice(state.cursor), cursor: 0 }, state);
    }
    case 'delete_word_back': {
      const start = wordBackStart(state.value, state.cursor);
      if (start === state.cursor) return state;
      return pushUndo(
        { ...state, value: state.value.slice(0, start) + state.value.slice(state.cursor), cursor: start },
        state,
      );
    }
    case 'undo': {
      const prev = state.undoStack[state.undoStack.length - 1];
      if (!prev) return state;
      return { ...state, value: prev.value, cursor: prev.cursor, undoStack: state.undoStack.slice(0, -1) };
    }
    case 'history_up': {
      // Browse mode is captured once, at the moment browsing starts: a user
      // who typed '!' and pressed Up browses bash entries even though the
      // restored bare values no longer start with '!'.
      const browseMode =
        state.historyIndex === -1
          ? modeOf(state.value)
          : state.historyBrowseMode ?? modeOf(state.value);
      const cands = candidatesFor(state, browseMode);
      if (cands.length === 0) return state;
      const pos = state.historyIndex === -1 ? -1 : cands.indexOf(state.historyIndex);
      const newPos = pos < 0 ? 0 : Math.min(pos + 1, cands.length - 1);
      const chosenIdx = cands[newPos]!;
      const chosen = state.history[chosenIdx]!;
      return {
        ...state,
        value: chosen.value,
        cursor: chosen.value.length,
        historyBrowseMode: browseMode,
        historyDraft: state.historyIndex === -1 ? state.value : state.historyDraft,
        historyIndex: chosenIdx,
      };
    }
    case 'history_down': {
      if (state.historyIndex === -1) return state;
      const cands = candidatesFor(state, state.historyBrowseMode ?? modeOf(state.value));
      const pos = cands.indexOf(state.historyIndex);
      if (pos <= 0) {
        return {
          ...state,
          value: state.historyDraft,
          cursor: state.historyDraft.length,
          historyIndex: -1,
          historyBrowseMode: undefined,
        };
      }
      const nextIdx = cands[pos - 1]!;
      const chosen = state.history[nextIdx]!;
      return { ...state, value: chosen.value, cursor: chosen.value.length, historyIndex: nextIdx };
    }
    case 'submit_done':
      return {
        ...state,
        value: '',
        cursor: 0,
        pasteCounter: 0,
        pastedContents: {},
        undoStack: [],
        historyIndex: -1,
        historyDraft: '',
        historyBrowseMode: undefined,
        history: [action.entry, ...state.history.filter((e) => !(e.mode === action.entry.mode && e.value === action.entry.value))],
      };
    default:
      return state;
  }
}
```

> History convention (pinned): entries store **bare submitted values** (`echo hi`, not `!echo hi`); mode rides the entry's `mode` field; submit strips a leading `!`; browsing restores bare text into the buffer while `historyBrowseMode` (captured at first Up) keeps navigation filtered to the mode browsing started in.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/composerState.test.ts`
Expected: PASS (all describes).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/ui/composer/composerState.ts packages/brain-shell/src/ui/composer/paste.ts packages/brain-shell/src/ui/composer/translateKey.ts packages/brain-shell/src/ui/composer/historyStore.ts packages/brain-shell/src/test/ui/composerState.test.ts && \
git commit -m "feat(shell): composer input state machine — modes, undo, paste truncation, history nav

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: PromptInput view + interactive component

**Files:**
- Create: `src/ui/composer/PromptInput.tsx`
- Test: `src/test/ui/promptInputView.test.ts`

**Interfaces:**
- Consumes: `reduceComposer/createComposerState/modeOf/expandedValue` (Task 2), `loadHistory/appendHistory/HistoryEntry` (Task 2), `translateKey/KeyInfo/KeyCommand` (Task 2), `Box/Text/useInput/useTheme` from `src/compat/index.js`, `Key` type from compat.
- Produces:
  - `PromptInputView(props: { value: string; cursor: number; busy: boolean }): React.ReactElement` — pure; renders mode glyph (`❯` prompt / `!` bash, colored `tokens.promptBorder` active / `tokens.promptBorderInactive` idle), value with inverse-video block cursor at `cursor`.
  - `PromptInput(props: { disabled?: boolean; busy?: boolean; onSubmit: (value: string) => void; onAbort?: () => void }): React.ReactElement` — owns composer/hookup state internally; loads history once on mount; handles exit (ctrl+c → `process.exit(0)`), abort (esc → `onAbort?.()`), submit (enter → `onSubmit(expandedValue(state).trim())` when non-empty, else noop; then `submit_done` with bare value + `appendHistory`).

- [ ] **Step 1: Write the failing test**

```tsx
// src/test/ui/promptInputView.test.tsx
import * as React from 'react';
import { describe, it, expect } from 'bun:test';
import { ThemeProvider } from '../../state/themeContext.js';
import { PALETTES } from '../../state/palettes.js';
import { PromptInputView } from '../../ui/composer/PromptInput.js';

function textOf(node: React.ReactNode): string {
  if (node == null || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(textOf).join('');
  if (React.isValidElement(node)) return textOf((node.props as { children?: React.ReactNode }).children);
  return '';
}

function render(node: React.ReactElement): string {
  return textOf(node);
}

const dark = PALETTES.dark();

describe('PromptInputView', () => {
  it('renders the ❯ glyph with prompt text and block cursor at end', () => {
    const tree = (
      <ThemeProvider palettes={PALETTES} initial="dark">
        <PromptInputView value="hello brain" cursor={11} busy={false} />
      </ThemeProvider>
    );
    const out = render(tree);
    expect(out).toContain('❯');
    expect(out).toContain('hello brain');
  });

  it('renders the ! glyph when the buffer is in bash mode', () => {
    const tree = (
      <ThemeProvider palettes={PALETTES} initial="dark">
        <PromptInputView value="!git status" cursor={11} busy={false} />
      </ThemeProvider>
    );
    const out = render(tree);
    expect(out).toContain('!');
    expect(out).toContain('git status');
  });

  it('shows an idle (inactive) prompt while a turn streams', () => {
    const tree = (
      <ThemeProvider palettes={PALETTES} initial="dark">
        <PromptInputView value="" cursor={0} busy={true} />
      </ThemeProvider>
    );
    const out = render(tree);
    expect(out).toContain('❯');
  });
});

void dark;
```

> Check `ThemeProvider`'s actual prop names against `src/state/themeContext.tsx` before writing the test (it may take `initialTheme`/`themeSetting` rather than `initial`, and palettes may come from context defaults). Mirror the usage found in existing tests (`grep -rn "ThemeProvider" src/test | head`). The assertions themselves stand regardless.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/promptInputView.test.tsx`
Expected: FAIL — cannot resolve `../../ui/composer/PromptInput.js`.

- [ ] **Step 3: Write minimal implementation**

```tsx
// src/ui/composer/PromptInput.tsx
import * as React from 'react';
import { Box, Text, useInput, useTheme } from '../../compat/index.js';
import type { Key } from '../../compat/index.js';
import {
  createComposerState, reduceComposer, modeOf, expandedValue,
} from './composerState.js';
import type { ComposerState } from './composerState.js';
import { translateKey } from './translateKey.js';
import type { KeyInfo } from './translateKey.js';
import { loadHistory, appendHistory } from './historyStore.js';

function asKeyInfo(key: Key): KeyInfo {
  return {
    upArrow: key.upArrow,
    downArrow: key.downArrow,
    leftArrow: key.leftArrow,
    rightArrow: key.rightArrow,
    return: key.return,
    escape: key.escape,
    ctrl: key.ctrl,
    meta: key.meta,
    shift: key.shift,
    backspace: (key as { backspace?: boolean }).backspace,
    delete: key.delete,
  };
}

/** Pure view: mode glyph + buffer with block cursor. Tests assert this directly. */
export function PromptInputView(props: {
  value: string;
  cursor: number;
  busy: boolean;
}): React.ReactElement {
  const { tokens } = useTheme();
  const mode = modeOf(props.value);
  const glyph = mode === 'bash' ? '!' : '❯';
  const glyphColor = props.busy ? tokens.promptBorderInactive : tokens.promptBorder;
  const before = props.value.slice(0, props.cursor);
  const at = props.value.slice(props.cursor, props.cursor + 1);
  const after = props.value.slice(props.cursor + 1);
  return (
    <Box>
      <Text color={glyphColor}>{glyph} </Text>
      <Text>
        {before}
        <Text inverse>{at.length > 0 ? at : ' '}</Text>
        {after}
      </Text>
    </Box>
  );
}

export function PromptInput(props: {
  disabled?: boolean;
  busy?: boolean;
  onSubmit: (value: string) => void;
  onAbort?: () => void;
}): React.ReactElement {
  const [state, setState] = React.useState<ComposerState>(() =>
    createComposerState(loadHistory()),
  );

  useInput((input, key) => {
    if (props.disabled) return;
    const cmd = translateKey(input, asKeyInfo(key));
    if (cmd.type === 'exit') {
      process.exit(0);
      return;
    }
    if (cmd.type === 'abort') {
      props.onAbort?.();
      return;
    }
    if (cmd.type === 'submit') {
      // Reading `state` (not the updater form) is safe here: ink serializes
      // keystrokes through one handler, so `state` is fresh at each event.
      const value = expandedValue(state).trim();
      if (value.length === 0) return;
      const wasBash = modeOf(value) === 'bash';
      const bare = wasBash ? value.slice(1).trimStart() : value;
      const entry = { mode: wasBash ? ('bash' as const) : ('prompt' as const), value: bare };
      setState((s) => reduceComposer(s, { type: 'submit_done', entry }));
      appendHistory(entry);
      props.onSubmit(bare);
      return;
    }
    if (cmd.type !== 'noop') {
      setState((s) => reduceComposer(s, cmd));
    }
  });

  return <PromptInputView value={state.value} cursor={state.cursor} busy={props.busy ?? false} />;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/promptInputView.test.tsx`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/ui/composer/PromptInput.tsx packages/brain-shell/src/test/ui/promptInputView.test.tsx && \
git commit -m "feat(shell): PromptInput — prompt/bash glyphs, block cursor, keybinding hookup

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Markdown renderer (pure subset)

**Files:**
- Create: `src/ui/transcript/markdown.ts`
- Create: `src/ui/transcript/Markdown.tsx`
- Test: `src/test/ui/markdown.test.ts`

**Interfaces:**
- Consumes: `Box/Text/useTheme` from compat.
- Produces:
  - `markdown.ts`: `type MdStyle = 'plain'|'bold'|'italic'|'code'|'codeBlock'|'header'|'bulletMarker'|'linkText'|'linkUrl'`; `interface MdSegment { text: string; style: MdStyle }`; `interface MdLine { segments: MdSegment[] }`; `parseMarkdown(source: string): MdLine[]`; `parseInline(text: string): MdSegment[]`.
  - `Markdown.tsx`: `MarkdownView(props: { lines: MdLine[] }): React.ReactElement`; `Markdown(props: { source: string }): React.ReactElement`.

Supported subset (Inc 1): fenced code blocks (``` toggles; markers hidden), `#`…`######` headers (bold brand), `-`/`*` bullets (dim `•` marker), `1.`/`1)` ordered lists (dim `·` marker), inline `**bold**`, `*italic*`, `` `code` ``, `[text](url)` (underlined text + dim url suffix). Unknown syntax passes through plain — malformed input must never throw.

- [ ] **Step 1: Write the failing test**

```ts
// src/test/ui/markdown.test.ts
import { describe, it, expect } from 'bun:test';
import { parseMarkdown, parseInline } from '../../ui/transcript/markdown.js';

describe('parseInline', () => {
  it('styles bold, italic, code spans', () => {
    const segs = parseInline('a **bold** b *it* c `code` d');
    expect(segs).toEqual([
      { text: 'a ', style: 'plain' },
      { text: 'bold', style: 'bold' },
      { text: ' b ', style: 'plain' },
      { text: 'it', style: 'italic' },
      { text: ' c ', style: 'plain' },
      { text: 'code', style: 'code' },
      { text: ' d', style: 'plain' },
    ]);
  });

  it('renders links as text plus dim url suffix', () => {
    const segs = parseInline('see [docs](https://brain.local/x) now');
    expect(segs).toEqual([
      { text: 'see ', style: 'plain' },
      { text: 'docs', style: 'linkText' },
      { text: ' (https://brain.local/x)', style: 'linkUrl' },
      { text: ' now', style: 'plain' },
    ]);
  });

  it('returns plain passthrough for unstyled text and never throws on oddities', () => {
    expect(parseInline('just words')).toEqual([{ text: 'just words', style: 'plain' }]);
    expect(parseInline('**unclosed')).toEqual([{ text: '**unclosed', style: 'plain' }]);
    expect(parseInline('')).toEqual([]);
  });
});

describe('parseMarkdown blocks', () => {
  it('headers become single bold segments regardless of level', () => {
    const lines = parseMarkdown('# Title\n### Sub');
    expect(lines[0]!.segments).toEqual([{ text: 'Title', style: 'header' }]);
    expect(lines[1]!.segments).toEqual([{ text: 'Sub', style: 'header' }]);
  });

  it('fenced code blocks mark every inner line codeBlock and hide fences', () => {
    const lines = parseMarkdown('before\n```ts\nconst x = 1;\nreturn x;\n```\nafter');
    expect(lines).toHaveLength(4);
    expect(lines[0]!.segments).toEqual([{ text: 'before', style: 'plain' }]);
    expect(lines[1]!.segments.map((s) => s.style)).toEqual(['codeBlock']);
    expect(lines[1]!.segments[0]!.text).toContain('const x = 1;');
    expect(lines[2]!.segments.map((s) => s.style)).toEqual(['codeBlock']);
    expect(lines[3]!.segments).toEqual([{ text: 'after', style: 'plain' }]);
  });

  it('bullets and ordered lists get dim markers and inline-parsed bodies', () => {
    const lines = parseMarkdown('- plain item\n* has **bold**\n1. numbered');
    expect(lines[0]!.segments[0]).toEqual({ text: '• ', style: 'bulletMarker' });
    expect(lines[0]!.segments[1]).toEqual({ text: 'plain item', style: 'plain' });
    expect(lines[1]!.segments.some((s) => s.style === 'bold')).toBe(true);
    expect(lines[2]!.segments[0]).toEqual({ text: '· ', style: 'bulletMarker' });
    expect(lines[2]!.segments[1]).toEqual({ text: 'numbered', style: 'plain' });
  });

  it('blank lines produce empty lines (spacing preserved)', () => {
    const lines = parseMarkdown('a\n\nb');
    expect(lines).toHaveLength(3);
    expect(lines[1]!.segments).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/markdown.test.ts`
Expected: FAIL — cannot resolve `../../ui/transcript/markdown.js`.

- [ ] **Step 3: Write minimal implementation**

```ts
// src/ui/transcript/markdown.ts
/**
 * Terminal markdown subset renderer (pure): source → styled segments.
 * Deliberately small: headings, fenced code, lists, bold/italic/code/links.
 * Anything unrecognized passes through as plain text; parsing never throws.
 */

export type MdStyle =
  | 'plain'
  | 'bold'
  | 'italic'
  | 'code'
  | 'codeBlock'
  | 'header'
  | 'bulletMarker'
  | 'linkText'
  | 'linkUrl';

export interface MdSegment {
  text: string;
  style: MdStyle;
}

export interface MdLine {
  segments: MdSegment[];
}

export function parseInline(text: string): MdSegment[] {
  if (text.length === 0) return [];
  const segs: MdSegment[] = [];
  const re =
    /(\*\*([^*]+)\*\*)|(`([^`]+)`)|(\*([^*]+)\*)|(\[([^\]]+)\]\(([^)]+)\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) segs.push({ text: text.slice(last, m.index), style: 'plain' });
    if (m[2] !== undefined) segs.push({ text: m[2], style: 'bold' });
    else if (m[4] !== undefined) segs.push({ text: m[4], style: 'code' });
    else if (m[6] !== undefined) segs.push({ text: m[6], style: 'italic' });
    else if (m[8] !== undefined) {
      segs.push({ text: m[8], style: 'linkText' });
      segs.push({ text: ` (${m[9]})`, style: 'linkUrl' });
    }
    last = re.lastIndex;
  }
  if (last < text.length) segs.push({ text: text.slice(last), style: 'plain' });
  return segs;
}

export function parseMarkdown(source: string): MdLine[] {
  const out: MdLine[] = [];
  let inFence = false;
  for (const raw of source.split('\n')) {
    if (/^```/.test(raw.trim())) {
      inFence = !inFence;
      continue;
    }
    if (inFence) {
      out.push({ segments: [{ text: raw.length > 0 ? raw : ' ', style: 'codeBlock' }] });
      continue;
    }
    const header = /^#{1,6}\s+(.*)$/.exec(raw);
    if (header) {
      out.push({ segments: [{ text: header[1]!, style: 'header' }] });
      continue;
    }
    const bullet = /^(\s*)[-*]\s+(.*)$/.exec(raw);
    if (bullet) {
      out.push({
        segments: [{ text: `${bullet[1]}• `, style: 'bulletMarker' }, ...parseInline(bullet[2]!)],
      });
      continue;
    }
    const ordered = /^(\s*)\d+[.)]\s+(.*)$/.exec(raw);
    if (ordered) {
      out.push({
        segments: [{ text: `${ordered[1]}· `, style: 'bulletMarker' }, ...parseInline(ordered[2]!)],
      });
      continue;
    }
    out.push({ segments: parseInline(raw) });
  }
  return out;
}
```

```tsx
// src/ui/transcript/Markdown.tsx
import * as React from 'react';
import { Text, useTheme } from '../../compat/index.js';
import { parseMarkdown } from './markdown.js';
import type { MdLine, MdStyle } from './markdown.js';

function flagsFor(style: MdStyle): {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  dimColor?: boolean;
} {
  switch (style) {
    case 'bold': return { bold: true };
    case 'italic': return { italic: true };
    case 'header': return { bold: true };
    case 'bulletMarker': return { dimColor: true };
    case 'linkText': return { underline: true };
    case 'linkUrl': return { dimColor: true };
    default: return {};
  }
}

function colorFor(style: MdStyle, tokens: ReturnType<typeof useTheme>['tokens']): string | undefined {
  switch (style) {
    case 'header': return tokens.brand;
    case 'code': return tokens.accent;
    case 'codeBlock': return tokens.subtle;
    default: return undefined;
  }
}

export function MarkdownView(props: { lines: MdLine[] }): React.ReactElement {
  const { tokens } = useTheme();
  return (
    <>
      {props.lines.map((line, li) => (
        <Text key={li}>
          {line.segments.length === 0
            ? ' '
            : line.segments.map((seg, si) => (
                <Text key={si} {...flagsFor(seg.style)} color={colorFor(seg.style, tokens)}>
                  {seg.text}
                </Text>
              ))}
          {'\n'}
        </Text>
      ))}
    </>
  );
}

export function Markdown(props: { source: string }): React.ReactElement {
  return <MarkdownView lines={parseMarkdown(props.source)} />;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/markdown.test.ts`
Expected: PASS (all describes).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/ui/transcript/markdown.ts packages/brain-shell/src/ui/transcript/Markdown.tsx packages/brain-shell/src/test/ui/markdown.test.ts && \
git commit -m "feat(shell): terminal markdown subset renderer bound to theme tokens

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: TranscriptRow taxonomy + turnToRows + MessageRow dispatch

**Files:**
- Modify: `src/contracts/messages.ts` (append at end — no existing lines changed)
- Create: `src/ui/transcript/toRows.ts`
- Create: `src/ui/transcript/MessageRow.tsx`
- Test: `src/test/ui/toRows.test.ts`
- Test: `src/test/ui/messageRowView.test.ts`

**Interfaces:**
- Consumes: `BrainTurnViewModel`, `ToolExecutionView` from `src/adapter/BrainViewModels.js`; `Markdown` (Task 4); `Box/Text/useTheme` from compat.
- Produces:
  - `contracts/messages.ts` additions (dependency-free — defines its own `ToolCardData`, structurally compatible with adapter's `ToolExecutionView`):
    ```ts
    export interface ToolCardData {
      callId: string;
      toolName: string;
      input: Record<string, unknown>;
      status: 'pending' | 'running' | 'completed' | 'failed' | 'denied' | 'cancelled';
      durationMs?: number;
    }

    export type TranscriptRow =
      | { kind: 'user'; id: string; text: string }
      | { kind: 'assistant'; id: string; markdown: string }
      | { kind: 'thinking'; id: string; text: string; durationMs?: number }
      | { kind: 'tool'; id: string; tool: ToolCardData }
      | { kind: 'error'; id: string; text: string };
    ```
  - `toRows.ts`: `turnToRows(turn: BrainTurnViewModel): TranscriptRow[]` — order: thinking (if text), tools (one row per ToolExecutionView, `permission_required` mapped to `pending`), assistant markdown (only when content trims non-empty), error (when turn.error). Row ids `${turn.id}:${kind}:${index}`. Memory provenance is NOT rendered in Inc 1 (documented deferral).
  - `MessageRow.tsx`: `MessageRow(props: { row: TranscriptRow; expanded: boolean }): React.ReactElement` memoized via `React.memo` comparing `row` identity + `expanded`; exports `UserRowView`, `AssistantRowView`, `ThinkingRowView`, `ToolRowView`, `ErrorRowView` (each `(props)` pure).
    - UserRowView: `❯ ` in `tokens.brand` + text.
    - AssistantRowView: `<Markdown source={row.markdown} />`.
    - ThinkingRowView: `✻ ` + italic dim text; when `durationMs` present renders `✻ Thought for {(ms/1000).toFixed(1)}s` header line then dim text.
    - ToolRowView: line 1 `⏺ ` brand + bold toolName + muted `(${summarizeToolInput(input)})`; line 2 indented `⎿ ` status-colored + statusLabel OR (expanded) pretty-printed input JSON in `tokens.subtle`. Status colors: pending/running → `tokens.brand`; completed → `tokens.success`; failed/denied/cancelled → `tokens.error`. `summarizeToolInput` exported: first string-valued field of {command,file_path,path,query,pattern,url,prompt}, trimmed to 60 chars, else compact JSON ≤60.
    - ErrorRowView: `⚠ ` in `tokens.warning` + text in `tokens.error`.

- [ ] **Step 1: Write the failing tests**

```ts
// src/test/ui/toRows.test.ts
import { describe, it, expect } from 'bun:test';
import { turnToRows } from '../../ui/transcript/toRows.js';
import type { BrainTurnViewModel, ToolExecutionView } from '../../adapter/BrainViewModels.js';

function vm(patch: Partial<BrainTurnViewModel>): BrainTurnViewModel {
  return {
    id: 'turn_1',
    role: 'assistant',
    content: '',
    status: 'completed',
    durationMs: 100,
    ...patch,
  };
}

describe('turnToRows', () => {
  it('emits thinking, tool, assistant, error rows in stable order', () => {
    const tool: ToolExecutionView = {
      callId: 'call_1',
      toolName: 'read_file',
      input: { path: '/tmp/a.txt' },
      status: 'permission_required',
    };
    const rows = turnToRows(vm({
      thinking: { text: 'pondering', isComplete: true, durationMs: 1200 },
      tools: [tool],
      content: '# Answer\nBody text',
      error: 'boom',
    }));
    expect(rows.map((r) => r.kind)).toEqual(['thinking', 'tool', 'assistant', 'error']);
    expect(rows[0]).toMatchObject({ kind: 'thinking', text: 'pondering', durationMs: 1200 });
    expect(rows[1]!.kind === 'tool' && rows[1]!.tool.status).toBe('pending'); // permission_required → pending
    expect(rows[2]!.kind === 'assistant' && rows[2]!.markdown.startsWith('# Answer')).toBe(true);
    expect(rows[3]).toMatchObject({ kind: 'error', text: 'boom' });
    expect(rows.every((r) => r.id.startsWith('turn_1:'))).toBe(true);
  });

  it('omits empty content, absent sections, and memory provenance silently', () => {
    const rows = turnToRows(vm({ memories: [{ nodeId: 'n1', label: 'L', score: 1, source: 's' }] }));
    expect(rows).toEqual([]);
  });
});
```

```tsx
// src/test/ui/messageRowView.test.tsx
import * as React from 'react';
import { describe, it, expect } from 'bun:test';
import { ThemeProvider } from '../../state/themeContext.js';
import { PALETTES } from '../../state/palettes.js';
import { UserRowView, ThinkingRowView, ToolRowView, ErrorRowView, summarizeToolInput } from '../../ui/transcript/MessageRow.js';
import type { TranscriptRow } from '../../contracts/messages.js';

function textOf(node: React.ReactNode): string {
  if (node == null || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(textOf).join('');
  if (React.isValidElement(node)) return textOf((node.props as { children?: React.ReactNode }).children);
  return '';
}
const render = (node: React.ReactElement) => textOf(node);
const tree = (el: React.ReactElement) => (
  <ThemeProvider palettes={PALETTES} initial="dark">{el}</ThemeProvider>
);

describe('row views', () => {
  it('user row echoes with the ❯ glyph', () => {
    const out = render(tree(<UserRowView row={{ kind: 'user', id: 'u1', text: 'hello there' }} />));
    expect(out).toContain('❯');
    expect(out).toContain('hello there');
  });

  it('thinking row renders the ✻ marker and duration when complete', () => {
    const out = render(tree(
      <ThinkingRowView row={{ kind: 'thinking', id: 't1', text: 'hmm', durationMs: 1500 }} />,
    ));
    expect(out).toContain('✻');
    expect(out).toContain('Thought for 1.5s');
    expect(out).toContain('hmm');
  });

  it('tool row collapsed shows name, summary, and running status', () => {
    const row: TranscriptRow = {
      kind: 'tool',
      id: 'c1',
      tool: { callId: 'c1', toolName: 'read_file', input: { path: '/tmp/brain-demo.txt' }, status: 'pending' },
    };
    const out = render(tree(<ToolRowView row={row} expanded={false} />));
    expect(out).toContain('read_file');
    expect(out).toContain('/tmp/brain-demo.txt');
    expect(out).toContain('Running…');
    expect(out).not.toContain('"path"');   // collapsed hides structured input
  });

  it('tool row expanded reveals pretty-printed input JSON', () => {
    const row: TranscriptRow = {
      kind: 'tool',
      id: 'c1',
      tool: { callId: 'c1', toolName: 'read_file', input: { path: '/tmp/brain-demo.txt' }, status: 'pending' },
    };
    const out = render(tree(<ToolRowView row={row} expanded={true} />));
    expect(out).toContain('"path"');
    expect(out).toContain('/tmp/brain-demo.txt');
  });

  it('error row carries the warning glyph', () => {
    const out = render(tree(<ErrorRowView row={{ kind: 'error', id: 'e1', text: 'socket lost' }} />));
    expect(out).toContain('⚠');
    expect(out).toContain('socket lost');
  });
});

describe('summarizeToolInput', () => {
  it('prefers well-known keys and truncates to 60 chars', () => {
    expect(summarizeToolInput({ path: '/a/b.txt', other: 1 })).toBe('/a/b.txt');
    expect(summarizeToolInput({ command: 'x'.repeat(80) })).toHaveLength(60);
    expect(summarizeToolInput({ zebra: 'last resort' })).toBe('last resort');
    expect(summarizeToolInput({})).toBe('');
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/toRows.test.ts src/test/ui/messageRowView.test.tsx`
Expected: FAIL — modules don't exist yet.

- [ ] **Step 3: Write minimal implementation**

Append to `src/contracts/messages.ts`:

```ts
// ── Transcript rows (Inc 1) ────────────────────────────────────────────────
// Presentation taxonomy derived from adapter view models. Kept dependency-
// free: ToolCardData mirrors adapter/BrainViewModels.ToolExecutionView
// structurally (assignment works both ways without an import).

export interface ToolCardData {
  callId: string;
  toolName: string;
  input: Record<string, unknown>;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'denied' | 'cancelled';
  durationMs?: number;
}

export type TranscriptRow =
  | { kind: 'user'; id: string; text: string }
  | { kind: 'assistant'; id: string; markdown: string }
  | { kind: 'thinking'; id: string; text: string; durationMs?: number }
  | { kind: 'tool'; id: string; tool: ToolCardData }
  | { kind: 'error'; id: string; text: string };
```

```ts
// src/ui/transcript/toRows.ts
import type { BrainTurnViewModel } from '../../adapter/BrainViewModels.js';
import type { TranscriptRow, ToolCardData } from '../../contracts/messages.js';

function toolCard(t: BrainTurnViewModel['tools'] extends (infer T)[] | undefined ? T : never): ToolCardData {
  return {
    callId: t.callId,
    toolName: t.toolName,
    input: t.input ?? {},
    status: t.status === 'permission_required' ? 'pending' : t.status,
    durationMs: t.durationMs,
  };
}

/** Frozen-row projection of a completed turn. Memory provenance deferred. */
export function turnToRows(turn: BrainTurnViewModel): TranscriptRow[] {
  const rows: TranscriptRow[] = [];
  let i = 0;
  const id = (kind: string) => `${turn.id}:${kind}:${i++}`;
  if (turn.thinking && turn.thinking.text.trim().length > 0) {
    rows.push({ kind: 'thinking', id: id('thinking'), text: turn.thinking.text, durationMs: turn.thinking.durationMs });
  }
  for (const t of turn.tools ?? []) {
    rows.push({ kind: 'tool', id: id('tool'), tool: toolCard(t) });
  }
  if (turn.content.trim().length > 0) {
    rows.push({ kind: 'assistant', id: id('assistant'), markdown: turn.content });
  }
  if (turn.error) {
    rows.push({ kind: 'error', id: id('error'), text: turn.error });
  }
  return rows;
}
```

```tsx
// src/ui/transcript/MessageRow.tsx
import * as React from 'react';
import { Box, Text, useTheme } from '../../compat/index.js';
import type { TranscriptRow, ToolCardData } from '../../contracts/messages.js';
import { Markdown } from './Markdown.js';

export function UserRowView(props: { row: Extract<TranscriptRow, { kind: 'user' }> }): React.ReactElement {
  const { tokens } = useTheme();
  return (
    <Text>
      <Text color={tokens.brand}>❯ </Text>
      {props.row.text}
    </Text>
  );
}

export function AssistantRowView(props: { row: Extract<TranscriptRow, { kind: 'assistant' }> }): React.ReactElement {
  return <Markdown source={props.row.markdown} />;
}

export function ThinkingRowView(props: { row: Extract<TranscriptRow, { kind: 'thinking' }> }): React.ReactElement {
  const { row } = props;
  return (
    <Box flexDirection="column">
      {row.durationMs !== undefined ? (
        <Text dimColor>✻ Thought for {(row.durationMs / 1000).toFixed(1)}s</Text>
      ) : null}
      <Text dimColor italic>
        {'✻ '}
        {row.text}
      </Text>
    </Box>
  );
}

export function summarizeToolInput(input: Record<string, unknown>): string {
  const preferred = ['command', 'file_path', 'path', 'query', 'pattern', 'url', 'prompt'];
  for (const key of preferred) {
    const v = input[key];
    if (typeof v === 'string' && v.trim().length > 0) return v.trim().slice(0, 60);
  }
  for (const v of Object.values(input)) {
    if (typeof v === 'string' && v.trim().length > 0) return v.trim().slice(0, 60);
  }
  return '';
}

function statusMeta(status: ToolCardData['status'], durationMs: number | undefined): { glyph: string; label: string } {
  switch (status) {
    case 'pending':
    case 'running':
      return { glyph: '⏳', label: 'Running…' };
    case 'completed':
      return { glyph: '✓', label: durationMs !== undefined ? `Done in ${(durationMs / 1000).toFixed(1)}s` : 'Done' };
    case 'failed':
      return { glyph: '✗', label: 'Failed' };
    case 'denied':
      return { glyph: '✗', label: 'Permission denied' };
    case 'cancelled':
      return { glyph: '⏹', label: 'Cancelled' };
  }
}

export function ToolRowView(props: { row: Extract<TranscriptRow, { kind: 'tool' }>; expanded: boolean }): React.ReactElement {
  const { tokens } = useTheme();
  const t = props.row.tool;
  const meta = statusMeta(t.status, t.durationMs);
  const summary = summarizeToolInput(t.input);
  const statusColor =
    t.status === 'completed' ? tokens.success
    : t.status === 'failed' || t.status === 'denied' || t.status === 'cancelled' ? tokens.error
    : tokens.brand;
  return (
    <Box flexDirection="column">
      <Text>
        <Text color={tokens.brand}>⏺ </Text>
        <Text bold>{t.toolName}</Text>
        {summary.length > 0 ? <Text color={tokens.muted}>{`(${summary})`}</Text> : null}
      </Text>
      <Text>
        {'  '}
        <Text color={statusColor}>⎿ {meta.glyph}</Text>
        {props.expanded ? (
          <Text color={tokens.subtle}>
            {'\n     '}
            {JSON.stringify(t.input, null, 2).split('\n').join('\n     ')}
          </Text>
        ) : (
          <Text color={tokens.subtle}>{` ${meta.label}`}</Text>
        )}
      </Text>
    </Box>
  );
}

export function ErrorRowView(props: { row: Extract<TranscriptRow, { kind: 'error' }> }): React.ReactElement {
  const { tokens } = useTheme();
  return (
    <Text>
      <Text color={tokens.warning}>⚠ </Text>
      <Text color={tokens.error}>{props.row.text}</Text>
    </Text>
  );
}

/** Memoized dispatch: completed rows keep identity, so frozen rows skip re-render. */
export const MessageRow = React.memo(
  function MessageRow(props: { row: TranscriptRow; expanded: boolean }): React.ReactElement {
    switch (props.row.kind) {
      case 'user': return <UserRowView row={props.row} />;
      case 'assistant': return <AssistantRowView row={props.row} />;
      case 'thinking': return <ThinkingRowView row={props.row} />;
      case 'tool': return <ToolRowView row={props.row} expanded={props.expanded} />;
      case 'error': return <ErrorRowView row={props.row} />;
    }
  },
  (a, b) => a.row === b.row && a.expanded === b.expanded,
);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/toRows.test.ts src/test/ui/messageRowView.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/contracts/messages.ts packages/brain-shell/src/ui/transcript/toRows.ts packages/brain-shell/src/ui/transcript/MessageRow.tsx packages/brain-shell/src/test/ui/toRows.test.ts packages/brain-shell/src/test/ui/messageRowView.test.tsx && \
git commit -m "feat(shell): TranscriptRow taxonomy, turn projection, MessageRow dispatch

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Chunk mapper + SessionController + snapshot hook

**Files:**
- Create: `src/adapter/chunkToTurnEvents.ts`
- Create: `src/state/sessionController.ts`
- Create: `src/ui/shell/useShellSnapshot.ts`
- Test: `src/test/adapter/chunkToTurnEvents.test.ts`
- Test: `src/test/state/sessionController.test.ts`

**Interfaces:**
- Consumes: `BrainBackendClient`, `BrainGenerationRequest`, `BrainStreamChunk` from `src/client/BrainBackendClient.js`; `normalizeMessagesForBrain` from `src/adapter/brainCallModel.js`; `createUserMessage` from `src/contracts/messages.js`; `BrainTurnEvent` from `src/adapter/BrainTurnEvents.js`; `BrainTurnTransformer` from `src/adapter/BrainTurnTransformer.js`; `turnToRows` (Task 5); `TwoStageTypewriterQueue` + cadence constants (Task 1); `LiveStreamView/StreamPhase/TypewriterQueue` (Task 1); `TranscriptRow` (Task 5).
- Produces:
  - `chunkToTurnEvent(chunk: BrainStreamChunk): BrainTurnEvent | null` — token→`text_delta`; thinking/redacted_thinking→`thinking_delta` (redacted emits `[redacted thinking]`); tool_use→`tool_call_requested` (input defaults `{}`); error→`turn_error` (message defaults `'Unknown daemon error'`); finished→`turn_complete` with `stopReason: chunk.status`; unknown/empty → `null`.
  - `ShellSnapshot { rows: TranscriptRow[]; live: LiveStreamView; busy: boolean; connectionError?: string }`
  - `class SessionController` — constructor `(client: BrainBackendClient)`; methods `subscribe(fn): unsubscribe`, `getSnapshot(): ShellSnapshot` (stable identity between emissions), `async submit(text: string): Promise<void>` (no-op while busy), `abort(): void`.
  - `useShellSnapshot(controller: SessionController): ShellSnapshot` — thin `useSyncExternalStore(controller.subscribe, controller.getSnapshot)`.

Controller semantics (the heart of Inc 1):
- `submit`: guard busy → push user row immediately → reset per-turn state (`events=[turn_start]`, fresh queue, `live={phase:'responding'…}`, new AbortController) → lazy `createSession()` once → iterate `client.streamText({sessionId,messages:[user],signal})` mapping each chunk through `chunkToTurnEvent`, appending events, routing text deltas into the queue and non-text events into `live` mutations → `finishTurn` on stream end or thrown error.
- Ticker: `setInterval(TYPEWRITER_TICK_MS)` drains `TYPEWRITER_CHARS_PER_TICK` chars into `live.responseText` while pending.
- `finishTurn(status)`: stop ticker, flush queue remainder into a synthetic final text_delta event (so frozen rows contain the full response even if drain lagged), transform accumulated events via `BrainTurnTransformer.transform(events)`, project via `turnToRows`, filter out empty assistant rows, append to `rows`, clear `live` to idle, emit. Errors also set `connectionError` when the message matches `/Could not connect|socket error|disconnected/`.
- `abort()`: `aborter.abort()` — the client turns this into a cancel frame + cancelled-finished chunk; the loop finishes normally.

- [ ] **Step 1: Write the failing tests**

```ts
// src/test/adapter/chunkToTurnEvents.test.ts
import { describe, it, expect } from 'bun:test';
import { chunkToTurnEvent } from '../../adapter/chunkToTurnEvents.js';

describe('chunkToTurnEvent', () => {
  it('maps tokens to text deltas and drops empties', () => {
    expect(chunkToTurnEvent({ type: 'token', token: 'Hi' })).toEqual({ type: 'text_delta', delta: 'Hi' });
    expect(chunkToTurnEvent({ type: 'token', token: '' })).toBeNull();
    expect(chunkToTurnEvent({ type: 'token' })).toBeNull();
  });

  it('maps thinking and redacted thinking to thinking deltas', () => {
    expect(chunkToTurnEvent({ type: 'thinking', thinking: 'hm' })).toEqual({ type: 'thinking_delta', delta: 'hm' });
    expect(chunkToTurnEvent({ type: 'redacted_thinking' })).toEqual({ type: 'thinking_delta', delta: '[redacted thinking]' });
  });

  it('maps tool_use preserving id/name/input', () => {
    expect(chunkToTurnEvent({ type: 'tool_use', toolUse: { id: 'call_9', name: 'search', input: { q: 'x' } } }))
      .toEqual({ type: 'tool_call_requested', callId: 'call_9', toolName: 'search', input: { q: 'x' } });
    expect(chunkToTurnEvent({ type: 'tool_use' })).toBeNull();
  });

  it('maps error and finished terminators', () => {
    expect(chunkToTurnEvent({ type: 'error', error: 'socket lost' })).toEqual({ type: 'turn_error', error: 'socket lost' });
    expect(chunkToTurnEvent({ type: 'finished', status: 'completed' })).toEqual({ type: 'turn_complete', stopReason: 'completed' });
    expect(chunkToTurnEvent({ type: 'finished' })).toEqual({ type: 'turn_complete', stopReason: undefined });
  });
});
```

> Verify exact `BrainTurnEvent` discriminant/field spellings against `src/adapter/BrainTurnEvents.ts` FIRST (`sed -n '1,80p' src/adapter/BrainTurnEvents.ts`) and adjust expectations to the real union — the plan's spelling is reconstructed from recon notes.

```ts
// src/test/state/sessionController.test.ts
import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type { BrainBackendClient, BrainStreamChunk, BrainGenerationRequest } from '../../client/BrainBackendClient.js';

/** Fake client: replays scripted chunks, records requests. */
function fakeClient(chunks: BrainStreamChunk[]) {
  const requests: BrainGenerationRequest[] = [];
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      requests.push(request);
      for (const c of chunks) yield c;
    },
  } as unknown as BrainBackendClient;
  return { client, requests };
}

const SCRIPT: BrainStreamChunk[] = [
  { type: 'thinking', thinking: 'recalling…' },
  { type: 'token', token: 'Hello ' },
  { type: 'token', token: 'from Brain.' },
  { type: 'tool_use', toolUse: { id: 'call_1', name: 'read_file', input: { path: '/tmp/x' } } },
  { type: 'finished', status: 'completed' },
];

describe('SessionController', () => {
  it('starts idle, freezes rows after a turn, exposes stable snapshots', async () => {
    const { client } = fakeClient(SCRIPT);
    const ctl = new SessionController(client);
    expect(ctl.getSnapshot().busy).toBe(false);
    expect(ctl.getSnapshot().rows).toEqual([]);

    await ctl.submit('hi there');

    const snap = ctl.getSnapshot();
    expect(snap.busy).toBe(false);
    expect(snap.rows[0]).toMatchObject({ kind: 'user', text: 'hi there' });
    const kinds = snap.rows.slice(1).map((r) => r.kind);
    expect(kinds).toContain('thinking');
    expect(kinds).toContain('assistant');
    expect(kinds).toContain('tool');
    expect(snap.live.phase).toBe('idle');
    // Snapshot identity is stable until the next emission.
    expect(ctl.getSnapshot()).toBe(snap);
  });

  it('routes text through the typewriter queue during the turn', async () => {
    const { client } = fakeClient([
      { type: 'token', token: 'abcdefgh' },
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    let sawPartial = false;
    const done = ctl.submit('q');
    await new Promise((r) => setTimeout(r, 5)); // let first chunk land
    if (ctl.getSnapshot().busy) {
      sawPartial = ctl.getSnapshot().live.responseText.length > 0 || ctl.getSnapshot().live.phase === 'responding';
    }
    await done;
    expect(sawPartial || ctl.getSnapshot().rows.some((r) => r.kind === 'assistant')).toBe(true);
  });

  it('surfaces connection failures as connectionError and an error row', async () => {
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        yield { type: 'error', error: 'Could not connect to Brain daemon at /tmp/nope.sock (ENOENT)' } as BrainStreamChunk;
      },
    } as unknown as BrainBackendClient;
    const ctl = new SessionController(client);
    await ctl.submit('ping');
    const snap = ctl.getSnapshot();
    expect(snap.connectionError).toBeTruthy();
    expect(snap.busy).toBe(false);
    expect(snap.rows.some((r) => r.kind === 'error')).toBe(true);
  });

  it('ignores submits while busy', async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => { release = r; });
    const client = {
      async createSession() {
        return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
      },
      async *streamText(): AsyncIterable<BrainStreamChunk> {
        await gate;
        yield { type: 'finished', status: 'completed' } as BrainStreamChunk;
      },
    } as unknown as BrainBackendClient;
    const ctl = new SessionController(client);
    const first = ctl.submit('one');
    await new Promise((r) => setTimeout(r, 5));
    expect(ctl.getSnapshot().busy).toBe(true);
    const second = ctl.submit('two');           // must no-op
    release();
    await Promise.all([first, second]);
    expect(ctl.getSnapshot().rows.filter((r) => r.kind === 'user')).toHaveLength(1);
  });
});
```

> Timing note: the ticker runs at 16 ms real time; tests sleep 5 ms which races. That is why assertions accept either a partial drain OR the frozen outcome. Do not tighten these into flakiness.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/adapter/chunkToTurnEvents.test.ts src/test/state/sessionController.test.ts`
Expected: FAIL — modules don't exist.

- [ ] **Step 3: Write minimal implementation**

```ts
// src/adapter/chunkToTurnEvents.ts
import type { BrainStreamChunk } from '../client/BrainBackendClient.js';
import type { BrainTurnEvent } from './BrainTurnEvents.js';

/**
 * Pure projection: transport chunk → presentation turn event.
 * Unknown chunk shapes return null (renderer never crashes on bad frames).
 */
export function chunkToTurnEvent(chunk: BrainStreamChunk): BrainTurnEvent | null {
  switch (chunk.type) {
    case 'token':
      return typeof chunk.token === 'string' && chunk.token.length > 0
        ? { type: 'text_delta', delta: chunk.token }
        : null;
    case 'thinking':
      return typeof chunk.thinking === 'string' && chunk.thinking.length > 0
        ? { type: 'thinking_delta', delta: chunk.thinking }
        : null;
    case 'redacted_thinking':
      return { type: 'thinking_delta', delta: '[redacted thinking]' };
    case 'tool_use':
      return chunk.toolUse
        ? { type: 'tool_call_requested', callId: chunk.toolUse.id, toolName: chunk.toolUse.name, input: chunk.toolUse.input ?? {} }
        : null;
    case 'error':
      return { type: 'turn_error', error: chunk.error ?? 'Unknown daemon error' };
    case 'finished':
      return { type: 'turn_complete', stopReason: chunk.status };
    default:
      return null;
  }
}
```

```ts
// src/state/sessionController.ts
/**
 * Turn-loop owner for the shell. Non-React class exposing an immutable
 * ShellSnapshot through subscribe/getSnapshot (useSyncExternalStore seam).
 * UI actions enter here; socket access stays behind the client seam.
 */
import type { BrainBackendClient, BrainGenerationRequest, BrainStreamChunk } from '../client/BrainBackendClient.js';
import { normalizeMessagesForBrain } from '../adapter/brainCallModel.js';
import { createUserMessage } from '../contracts/messages.js';
import type { TranscriptRow } from '../contracts/messages.js';
import type { LiveStreamView } from '../contracts/streaming.js';
import { TwoStageTypewriterQueue, TYPEWRITER_TICK_MS, TYPEWRITER_CHARS_PER_TICK } from '../adapter/streaming/TwoStageTypewriterQueue.js';
import { chunkToTurnEvent } from '../adapter/chunkToTurnEvents.js';
import { BrainTurnTransformer } from '../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../adapter/BrainTurnEvents.js';
import { turnToRows } from '../ui/transcript/toRows.js';

export interface ShellSnapshot {
  rows: TranscriptRow[];
  live: LiveStreamView;
  busy: boolean;
  connectionError?: string;
}

const IDLE_LIVE: LiveStreamView = { phase: 'idle', thinkingText: '', responseText: '' };
const CONNECTION_RE = /Could not connect|socket error|disconnected/i;

export class SessionController {
  private listeners = new Set<() => void>();
  private rows: TranscriptRow[] = [];
  private live: LiveStreamView = IDLE_LIVE;
  private busy = false;
  private connectionError: string | undefined;
  private sessionId: string | undefined;
  private aborter: AbortController | null = null;
  private queue = new TwoStageTypewriterQueue();
  private ticker: ReturnType<typeof setInterval> | null = null;
  private events: BrainTurnEvent[] = [];
  private turnSeq = 0;
  private snapshot: ShellSnapshot = { rows: [], live: IDLE_LIVE, busy: false };

  constructor(private client: BrainBackendClient) {}

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  getSnapshot = (): ShellSnapshot => this.snapshot;

  abort(): void {
    this.aborter?.abort();
  }

  async submit(text: string): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.connectionError = undefined;
    const turnId = `turn_${++this.turnSeq}`;
    this.rows = [...this.rows, { kind: 'user', id: `user:${turnId}`, text }];
    this.events = [{ type: 'turn_start', turnId, role: 'assistant' }];
    this.queue = new TwoStageTypewriterQueue();
    this.live = { phase: 'responding', thinkingText: '', responseText: '' };
    this.aborter = new AbortController();
    this.emit();
    this.startTicker();
    try {
      if (this.sessionId === undefined) {
        this.sessionId = (await this.client.createSession()).sessionId;
      }
      const request: BrainGenerationRequest = {
        sessionId: this.sessionId,
        messages: normalizeMessagesForBrain([createUserMessage(text)]),
        signal: this.aborter.signal,
      };
      for await (const chunk of this.client.streamText(request)) {
        this.handleChunk(chunk);
      }
      this.finishTurn('completed');
    } catch (err) {
      this.finishTurn('error', err instanceof Error ? err.message : String(err));
    }
  }

  private handleChunk(chunk: BrainStreamChunk): void {
    if (chunk.type === 'error' && chunk.error && CONNECTION_RE.test(chunk.error)) {
      this.connectionError = chunk.error;
    }
    const event = chunkToTurnEvent(chunk);
    if (event === null) return;
    this.events.push(event);
    if (event.type === 'text_delta') {
      this.queue.push(event.delta);
      // First response text ends the thinking phase visually.
      if (this.live.phase === 'thinking' || this.live.phase === 'responding') {
        this.live = { ...this.live, phase: 'responding' };
      }
    } else if (event.type === 'thinking_delta') {
      this.live = {
        ...this.live,
        phase: 'thinking',
        thinkingText: this.live.thinkingText + event.delta,
      };
    } else if (event.type === 'tool_call_requested') {
      this.live = { ...this.live, phase: 'tool', activeToolName: event.toolName };
    } else if (event.type === 'turn_error') {
      this.live = { ...this.live, phase: 'error', errorText: event.error };
    }
    this.emit();
  }

  private startTicker(): void {
    this.ticker = setInterval(() => {
      if (this.queue.pending === 0) return;
      const out = this.queue.drain(TYPEWRITER_CHARS_PER_TICK);
      this.live = { ...this.live, responseText: this.live.responseText + out };
      this.emit();
    }, TYPEWRITER_TICK_MS);
  }

  private stopTicker(): void {
    if (this.ticker !== null) {
      clearInterval(this.ticker);
      this.ticker = null;
    }
  }

  private finishTurn(status: 'completed' | 'error', errorText?: string): void {
    this.stopTicker();
    // Flush any undrained typewriter text so frozen rows carry the whole answer.
    const remainder = this.queue.pending > 0 ? this.queue.drain(this.queue.pending) : '';
    if (remainder.length > 0) {
      this.events.push({ type: 'text_delta', delta: remainder });
    }
    if (status === 'error' && errorText !== undefined) {
      this.events.push({ type: 'turn_error', error: errorText });
    }
    this.events.push({ type: 'thinking_end' });
    this.events.push({ type: 'turn_complete' });
    try {
      const vm = BrainTurnTransformer.transform(this.events);
      const projected = turnToRows(vm).filter(
        (r) => !(r.kind === 'assistant' && r.markdown.trim().length === 0),
      );
      this.rows = [...this.rows, ...projected];
    } catch {
      // Transformer mismatch must never kill the shell; keep prior rows.
    }
    this.live = IDLE_LIVE;
    this.busy = false;
    this.aborter = null;
    this.emit();
  }

  private emit(): void {
    this.snapshot = {
      rows: this.rows,
      live: this.live,
      busy: this.busy,
      connectionError: this.connectionError,
    };
    for (const fn of this.listeners) fn();
  }
}
```

> Verify `BrainTurnEvent` spellings (`thinking_end` payload, `turn_complete` optionality, whether `role` is required on `turn_start`) against `src/adapter/BrainTurnEvents.ts` and adapt. Also confirm `createUserMessage(text)` produces the `{ role:'user', …}` shape `normalizeMessagesForBrain` expects (it consumes `Message[]` from contracts/messages.ts).

```ts
// src/ui/shell/useShellSnapshot.ts
import { useSyncExternalStore } from 'react';
import type { SessionController, ShellSnapshot } from '../../state/sessionController.js';

export function useShellSnapshot(controller: SessionController): ShellSnapshot {
  return useSyncExternalStore(controller.subscribe, controller.getSnapshot);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/adapter/chunkToTurnEvents.test.ts src/test/state/sessionController.test.ts`
Expected: PASS. Then the sweep: `bun test 2>&1 | tail -3` — record pass/fail counts; must equal pre-task baseline + new passes.

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/adapter/chunkToTurnEvents.ts packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/ui/shell/useShellSnapshot.ts packages/brain-shell/src/test/adapter/chunkToTurnEvents.test.ts packages/brain-shell/src/test/state/sessionController.test.ts && \
git commit -m "feat(shell): session controller — chunk mapping, typewriter drain, snapshot store

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Spinner + AppShell composition + entrypoint swap

**Files:**
- Create: `src/ui/shell/Spinner.tsx`
- Create: `src/ui/shell/AppShell.tsx`
- Modify: `src/main.tsx` (AppSkeleton → AppShell)
- Modify: `src/test/architectureFitness.test.ts` (expect `AppShell`)
- Delete: `src/ui/shell/AppSkeleton.tsx`
- Test: `src/test/ui/spinnerView.test.ts`

**Interfaces:**
- Consumes: everything above; `UdsBrainBackendClient` from `src/client/UdsBrainBackendClient.js`; `useMainLoopModel` from `src/contracts/model.js`; `useTerminalSize/Box/Text/useInput/useTheme` from compat; `BrainMark` from `./BrainMark.js`.
- Produces:
  - `spinnerFrames: readonly string[]` — palindrome bounce `['✢','✳','∗','✻','✻','∗','✳','✢']`; `spinnerFrameAt(elapsedMs: number): string` = `frames[floor(ms/120) % len]`; `spinnerLabel(live: LiveStreamView): string` — thinking→`Thinking…`, responding→`Composing…`, tool→`${activeToolName}…`, error→`Failed`, idle→``.
  - `SpinnerView(props: { elapsedMs: number; label: string }): React.ReactElement` (pure); `Spinner(props: { label: string })` (interval timer, 120 ms).
  - `AppShell()` layout column: `<BrainMark />` → frozen rows (`snapshot.rows.map(MessageRow)`, memoized identity) → live block when busy (Spinner + last thinking line dim + streamed `responseText`) → connection banner (`⚠ ` + `connectionError` in error color) → `<PromptInput />` → footer `model: {model} · ctrl+c exit · ! bash · ↑↓ history · esc stop · ctrl+o tools` dim.
  - Global keys in AppShell: `ctrl+c` exit (kept from skeleton), `ctrl+o` toggle expandTools.
  - Submit handler: `controller.submit(value)` (bash stripping already handled inside PromptInput).
  - Width: outer `Box width={columns}` from `useTerminalSize()`; NO outer border (full-height app frame lands in Inc 3); compact-width (<80 cols) naturally flows since all children are single-column.

- [ ] **Step 1: Write the failing test**

```ts
// src/test/ui/spinnerView.test.ts
import { describe, it, expect } from 'bun:test';
import { spinnerFrameAt, spinnerLabel, spinnerFrames } from '../../ui/shell/Spinner.js';
import type { LiveStreamView } from '../../contracts/streaming.js';

const live = (patch: Partial<LiveStreamView>): LiveStreamView => ({
  phase: 'responding', thinkingText: '', responseText: '', ...patch,
});

describe('SpinnerView math', () => {
  it('cycles palindrome frames at 120ms', () => {
    expect(spinnerFrameAt(0)).toBe(spinnerFrames[0]);
    expect(spinnerFrameAt(119)).toBe(spinnerFrames[0]);
    expect(spinnerFrameAt(120)).toBe(spinnerFrames[1]);
    expect(spinnerFrameAt(120 * spinnerFrames.length)).toBe(spinnerFrames[0]); // wraps
    // Palindrome bounce: frame[len-1] mirrors frame[1]
    expect(spinnerFrames[spinnerFrames.length - 1]).toBe(spinnerFrames[1]);
  });

  it('labels phases including the active tool name', () => {
    expect(spinnerLabel(live({}))).toBe('Composing…');
    expect(spinnerLabel(live({ phase: 'thinking' }))).toBe('Thinking…');
    expect(spinnerLabel(live({ phase: 'tool', activeToolName: 'read_file' }))).toBe('read_file…');
    expect(spinnerLabel(live({ phase: 'error' }))).toBe('Failed');
    expect(spinnerLabel(live({ phase: 'idle' }))).toBe('');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/spinnerView.test.ts`
Expected: FAIL — module missing.

- [ ] **Step 3: Write minimal implementation**

```tsx
// src/ui/shell/Spinner.tsx
import * as React from 'react';
import { Text } from '../../compat/index.js';
import { useTheme } from '../../compat/index.js';
import type { LiveStreamView } from '../../contracts/streaming.js';

/** Palindrome bounce, 120 ms cadence (reference Spinner contract). */
export const spinnerFrames: readonly string[] = ['✢', '✳', '∗', '✻', '✻', '∗', '✳', '✢'];
const FRAME_MS = 120;

export function spinnerFrameAt(elapsedMs: number): string {
  const idx = Math.floor(Math.max(0, elapsedMs) / FRAME_MS) % spinnerFrames.length;
  return spinnerFrames[idx]!;
}

export function spinnerLabel(live: LiveStreamView): string {
  switch (live.phase) {
    case 'thinking': return 'Thinking…';
    case 'responding': return 'Composing…';
    case 'tool': return `${live.activeToolName ?? 'Working'}…`;
    case 'error': return 'Failed';
    default: return '';
  }
}

export function SpinnerView(props: { elapsedMs: number; label: string }): React.ReactElement {
  const { tokens } = useTheme();
  return (
    <Text>
      <Text color={tokens.brandShimmer}>{spinnerFrameAt(props.elapsedMs)}</Text>
      {props.label.length > 0 ? <Text dimColor>{` ${props.label}`}</Text> : null}
    </Text>
  );
}

export function Spinner(props: { label: string }): React.ReactElement {
  const [start] = React.useState(() => Date.now());
  const [, forceTick] = React.useReducer((n: number) => n + 1, 0);
  React.useEffect(() => {
    const t = setInterval(forceTick, FRAME_MS);
    return () => clearInterval(t);
  }, []);
  void start;
  return <SpinnerView elapsedMs={Date.now() - start} label={props.label} />;
}
```

```tsx
// src/ui/shell/AppShell.tsx
import * as React from 'react';
import { Box, Text, useInput, useTerminalSize } from '../../compat/index.js';
import { BrainMark } from './BrainMark.js';
import { PromptInput } from '../composer/PromptInput.js';
import { MessageRow } from '../transcript/MessageRow.js';
import { Spinner, spinnerLabel } from './Spinner.js';
import { useMainLoopModel } from '../../contracts/model.js';
import { useTheme } from '../../compat/index.js';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';
import { SessionController } from '../../state/sessionController.js';
import { useShellSnapshot } from './useShellSnapshot.js';

export function AppShell(): React.ReactElement {
  const { columns } = useTerminalSize();
  const { tokens } = useTheme();
  const model = useMainLoopModel(); // hoisted hooks — never inside JSX conditionals
  const [expandTools, setExpandTools] = React.useState(false);

  const controller = React.useMemo(
    () => new SessionController(new UdsBrainBackendClient()),
    [],
  );
  const snapshot = useShellSnapshot(controller);

  useInput((input, key) => {
    if (key.ctrl && input === 'c') process.exit(0);
    if (key.ctrl && input === 'o') setExpandTools((e) => !e);
  });

  const handleSubmit = React.useCallback(
    (value: string) => {
      void controller.submit(value);
    },
    [controller],
  );

  const thinkingTail = snapshot.live.thinkingText.trimEnd();
  const lastThinkingLine = thinkingTail.length > 0 ? thinkingTail.split('\n').slice(-1)[0]! : '';

  return (
    <Box flexDirection="column" width={columns}>
      <BrainMark />
      <Box flexDirection="column" marginTop={1}>
        {snapshot.rows.map((row) => (
          <MessageRow key={row.id} row={row} expanded={expandTools} />
        ))}
      </Box>
      {snapshot.busy ? (
        <Box flexDirection="column" marginTop={1}>
          <Spinner label={spinnerLabel(snapshot.live)} />
          {lastThinkingLine.length > 0 ? (
            <Text dimColor italic>{`✻ ${lastThinkingLine}`}</Text>
          ) : null}
          {snapshot.live.responseText.length > 0 ? (
            <Text>{snapshot.live.responseText}</Text>
          ) : null}
        </Box>
      ) : null}
      {snapshot.connectionError !== undefined ? (
        <Text color={tokens.error}>{`⚠ ${snapshot.connectionError}`}</Text>
      ) : null}
      <Box marginTop={1}>
        <PromptInput
          disabled={snapshot.busy}
          busy={snapshot.busy}
          onSubmit={handleSubmit}
          onAbort={() => controller.abort()}
        />
      </Box>
      <Box marginTop={1}>
        <Text dimColor>
          {`model: ${model} · ctrl+c exit · ! bash · ↑↓ history · esc stop · ctrl+o ${expandTools ? 'collapse' : 'expand'} tools`}
        </Text>
      </Box>
    </Box>
  );
}
```

Modify `src/main.tsx` — swap one identifier:

```tsx
import { AppShell } from './ui/shell/AppShell.js';
// …render(React.createElement(AppShell), { patchConsole: false });
```

Modify `src/test/architectureFitness.test.ts`:

```ts
expect(mainContent).toContain('AppShell');
```

Delete `src/ui/shell/AppSkeleton.tsx` via explicit path.

- [ ] **Step 4: Run tests + fitness gate**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/spinnerView.test.ts src/test/architectureFitness.test.ts`
Expected: PASS.
Then full sweep: `bun test 2>&1 | tail -3` — zero new failures vs baseline.

- [ ] **Step 5: Bundle gate**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun build src/main.tsx --outdir dist --target bun >/dev/null && echo BUILD_OK`
Expected: `BUILD_OK`. On vendor-storm symptoms, `rm -rf ~/Library/Caches/bun` and retry.

- [ ] **Step 6: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add packages/brain-shell/src/ui/shell/Spinner.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/ui/shell/useShellSnapshot.ts packages/brain-shell/src/main.tsx packages/brain-shell/src/test/architectureFitness.test.ts packages/brain-shell/src/test/ui/spinnerView.test.ts && \
git rm packages/brain-shell/src/ui/shell/AppSkeleton.tsx && \
git commit -m "feat(shell): AppShell static/live split — spinner, frozen transcript, composer mount

Co-Authored-By: Claude <noreply@anthropic.com>"
```

> `useShellSnapshot.ts` was created in Task 6 but committed there only if listed; it IS listed in Task 6's commit. If it accidentally landed uncommitted, stage it here explicitly.

---

### Task 8: PTY smoke — launch / mid-stream / expanded tool card / bash strip

**Files:**
- Create: `scripts/ptySmokeInc1.py` (repo-root `scripts/` — same location as the existing `check_perf.py` / `check_soak_gates.py` runners)
- Create: `src/test/fixtures/pty/inc1/{launch,expanded,transcript}.txt` (written by the harness's `snapshot()`; committed artifacts)

**Interfaces:**
- Consumes: built binary via `bun run src/main.tsx` under a PTY; env `BRAIN_SOCKET_PATH` (honored by `UdsBrainBackendClient` constructor default).
- Produces: exit 0 on success; sanitized fixture transcripts under `src/test/fixtures/pty/inc1/{launch,midstream,expanded}.txt`.

Protocol facts baked into the stub (verified against `UdsBrainBackendClient.ts`):
- RPC request frame: `{"id","action","payload","body"}\n`; success reply must carry `"status":"success"` and `body` (JSON or object) — create-session uses action `v1/session/create`, replies `{session_id}`.
- Stream request frame: action `v1/generation/stream`, `payload.messages` array; reply frames are newline-delimited JSON with strict sequence numbers starting at 0 (gap ⇒ client raises protocol violation); `{"type":"token","token":"…","sequence":N}`, `{"type":"thinking","thinking":"…","sequence":N}`, `{"type":"tool_use","toolUse":{"id","name","input"},"sequence":N}`, terminator `{"type":"finished","status":"completed","sequence":N}`.

- [ ] **Step 1: Write the harness**

```python
#!/usr/bin/env python3
"""Increment 1 PTY smoke: launch frame / mid-stream / expanded tool card / bash strip."""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc1-smoke.sock"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
FRAMES_FILE = "/tmp/brain-inc1-smoke-requests.jsonl"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

# ── stub daemon ────────────────────────────────────────────────────────────
STREAM_FRAMES = [
    {"type": "thinking", "thinking": "Recalling memories…"},
    {"type": "thinking", "thinking": " Drafting."},
    {"type": "token", "token": "Hello "},
    {"type": "token", "token": "from the "},
    {"type": "token", "token": "Brain daemon stream."},
    {"type": "tool_use", "toolUse": {"id": "call_1", "name": "read_file",
                                     "input": {"path": "/tmp/brain-demo.txt"}}},
    {"type": "token", "token": " Read the demo file fine."},
]

def serve():
    if os.path.exists(SOCK):
        os.remove(SOCK)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(8)
    seq_by_conn = {}
    while True:
        conn, _ = srv.accept()
        fobj = conn.makefile("rw")
        def handle(conn=conn, fobj=fobj):
            seq = 0
            try:
                for line in fobj:
                    req = json.loads(line)
                    with open(FRAMES_FILE, "a") as log:
                        log.write(json.dumps(req) + "\n")
                    action = req.get("action")
                    rid = req.get("id")
                    if action == "v1/session/create":
                        fobj.write(json.dumps({"id": rid, "status": "success",
                                               "body": {"session_id": "stub-session-1"}}) + "\n")
                        fobj.flush()
                    elif action == "v1/generation/stream":
                        for i, frame in enumerate(STREAM_FRAMES):
                            out = dict(frame); out["sequence"] = seq; seq += 1
                            fobj.write(json.dumps(out) + "\n"); fobj.flush()
                            if frame["type"] == "tool_use":
                                time.sleep(1.2)          # window for mid-stream asserts
                        out = {"type": "finished", "status": "completed",
                               "sequence": seq}; seq += 1
                        fobj.write(json.dumps(out) + "\n"); fobj.flush()
            except Exception:
                pass
            finally:
                try: conn.close()
                except Exception: pass
        threading.Thread(target=handle, daemon=True).start()

threading.Thread(target=serve, daemon=True).start()
if os.path.exists(FRAMES_FILE):
    os.remove(FRAMES_FILE)

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.chdir("/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell")
    os.execvp("bun", ["bun", "run", "src/main.tsx"])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
```

```python
buf = b""
def pump(seconds):
    global buf
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if fd in r:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk

FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc1"

def snapshot(name):
    os.makedirs(FIXTURE_DIR, exist_ok=True)
    open(f"{FIXTURE_DIR}/{name}.txt", "w").write(clean(buf))

def expect(label, needle, timeout=8.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print(f"PASS {label}")
            return True
    print(f"FAIL {label}: {needle!r} not seen")
    return False

ok = True

# ── Flow A: launch frame ──────────────────────────────────────────────────
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-tagline", "memory-first agent workspace")
ok &= expect("launch-composer", "❯")
snapshot("launch")

# ── Flow B: mid-stream turn ───────────────────────────────────────────────
# The stub sleeps 1.2 s AFTER the tool_use frame, giving a deterministic
# mid-turn window. The live spinner's tool label can ONLY exist during that
# window — after finishTurn the live region unmounts and the frozen card
# renders 'Done' instead — so it doubles as proof that the stream was
# observed mid-flight rather than after completion.
os.write(fd, b"tell me something\r")
ok &= expect("mid-stream-thinking", "Recalling memories…")
ok &= expect("mid-stream-text", "Hello from the Brain daemon stream.")
ok &= expect("live-tool-label", "read_file…")

# Turn completes: frozen transcript carries the merged answer + done card.
ok &= expect("final-frozen-answer", "Read the demo file fine.")
ok &= expect("tool-card-collapsed-done", "Done")

# ── Flow C: ctrl+o expands frozen tool cards ──────────────────────────────
os.write(fd, b"\x0f")
ok &= expect("tool-card-expanded", '"path": "/tmp/brain-demo.txt"')
snapshot("expanded")
os.write(fd, b"\x0f")  # restore collapsed

# ── Flow D: bash-mode strip ('!echo hi' submits bare 'echo hi') ───────────
# Runs last: the composer ignores input while busy, and the previous turn
# has fully completed by now.
os.write(fd, b"!echo hi\r")
deadline = time.time() + 8
stripped = False
while time.time() < deadline:
    pump(0.1)
    try:
        for line in open(FRAMES_FILE):
            req = json.loads(line)
            msgs = (req.get("payload") or {}).get("messages") or []
            if msgs and isinstance(msgs[-1].get("content"), str) and msgs[-1]["content"].strip() == "echo hi":
                stripped = True
    except FileNotFoundError:
        pass
    if stripped:
        break
print(("PASS" if stripped else "FAIL") + " bash-strip")
ok &= stripped

os.write(fd, b"\x03")   # ctrl+c
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
snapshot("transcript")

sys.exit(0 if ok else 1)
```

- [ ] **Step 2: Run the smoke**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc1.py`
Expected: all `PASS` lines, exit 0. If the composer glyph never appears, suspect winsize (the 0×0 trap) or a startup crash — check `/tmp/brain_crash.log`. Re-run once after any failure before investigating: first-run transpile cache misses can slow boot past a timeout.

- [ ] **Step 3: Commit fixtures + harness**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && \
git add scripts/ptySmokeInc1.py packages/brain-shell/src/test/fixtures/pty/inc1/launch.txt packages/brain-shell/src/test/fixtures/pty/inc1/expanded.txt packages/brain-shell/src/test/fixtures/pty/inc1/transcript.txt && \
git commit -m "test(shell): inc1 PTY smoke — launch, mid-stream, expanded tool card, bash strip

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Final gates (all must hold before finishing-a-development-branch)

1. `cd packages/brain-shell && bun test 2>&1 | tail -3` — zero NEW failures vs the 106-pass/5-baseline-fail reference; record numbers.
2. `bun build src/main.tsx --outdir dist --target bun` → BUILD_OK (purge full cache on vendor storm).
3. `python3 <smoke>.py` → all PASS, exit 0.
4. `grep -ri "claude\|anthropic\|vendor" packages/brain-shell/src --include="*.ts" --include="*.tsx" -l | grep -v claude-upstream | grep -v test/ || echo CLEAN` → CLEAN (test files may mention the fitness rules; source must not).
5. `git log --oneline main..HEAD` — every commit touched only explicitly-listed paths.

## Documented scope decisions (spec-conformant, recorded here so reviewers don't re-litigate)

- **Static/live split via memoization, not ink `<Static>`**: past rows must re-render on ctrl+o (collapsed→expanded), which `<Static>`'s freeze-once semantics forbid. Rows are frozen behaviorally — `React.memo` + stable row identities mean completed work never recomputes; the terminal's own scrollback (non-alt-screen ink) provides history. This reproduces the reference's observable behavior on stock Ink.
- **Tool output display deferred**: `BrainStreamChunk` has no result frame today (only `tool_use`; results flow via the `tool/feedback` adapter seam later). Expanded cards show structured input + status. `chunkToTurnEvent` gains a branch when the daemon grows a result frame.
- **`!` bash mode routes to the daemon as a normal prompt** (stripped of `!`). Local shell execution belongs to the command surface (Inc 2).
- **Memory-provenance display deferred**: view-model carries it; no Inc 1 row kind. Add in Inc 3 alongside the status line.
- **Reconnect/backoff deferred**: spec §7 disconnect banner ships now (connectionError line); exponential reconnect lands with the session frame (Inc 3) where banners belong visually.
