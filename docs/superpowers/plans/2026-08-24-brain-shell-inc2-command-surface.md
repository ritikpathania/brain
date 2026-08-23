# Brain Shell Increment 2 — Command Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the shell a slash-command surface — a registry with fuzzy matching, an arrow-navigable palette above the composer, a context-scoped keybinding framework, and working `/help`, `/clear`, `/quit`.

**Architecture:** Pure core first (`commands/registry.ts` matcher, `keybindings/resolve.ts` stroke→action resolver, a pure palette decision/window layer), then thin interactive shells over them: `PaletteView` renders inside `PromptInput` when the buffer is exactly `/query`; `AppShell` routes `/name` submits to a local executor instead of the daemon. The keybinding framework becomes load-bearing by replacing `AppShell`'s raw `useInput` block.

**Tech Stack:** Bun 1.4 + React 19 + stock Ink 7 via `src/compat/index.js`; no new dependencies (the reference tree uses fuse.js — we ship our own ~40-line scorer instead).

**Spec:** `docs/superpowers/specs/2026-08-23-brain-shell-contracts-first-design.md` §5 row 2 ("Command surface: slash registry + fuzzy palette, keybinding framework, `/help`"), §8 testing strategy.

## Global Constraints

Verbatim from spec §1 Hard constraints:

1. **No copied source.** The reference tree at `/Users/ritikpathania/Developer/claude-code` is *implementation archaeology only*: extract observable UX contracts, write original code. Nothing from that tree is vendored, committed, or redistributed. It stays outside this repository forever.
2. **No Anthropic product concepts.** No Claude/Anthropic models, APIs, authentication, pricing, billing, or LLM-vendor-specific product surfaces in Brain's UI.
3. **Brain runtime is authoritative.** The Rust daemon remains the composition root; all UI data flows through existing adapter/client seams.
4. **Preserve Brain architecture.** Domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, and adapter boundaries are untouched by frontend work.
5. **Incremental delivery.** Stack stays Bun + React 19 + Ink 7 + yoga-layout; no framework changes.

From spec §9 Governance + AGENTS.md TUI rules:

- Reference tree never copied into repo, referenced by path in code, or bundled.
- Theme tokens everywhere; interactive panels/popups use rounded borders (`╭ ─ ╮ │ ╯ ╰`); SIGWINCH-safe flex layouts; compact widths (<80 cols) handled.
- Every commit contains only explicitly-added paths (`git add <paths>` — never `git add .`).
- Commit trailer: `Co-Authored-By: Claude <noreply@anthropic.com>`.

## Reference behavior contract (archaeology notes, not code to copy)

Extracted from the read-only reference tree (paths quoted for provenance only):

- Suggestion overlay caps at 5 items.
- Fuzzy scoring weights name highest, then aliases, description lowest; prefix matches preferred over scattered ones; deterministic ordering.
- History navigation is suppressed while suggestions show (↑↓ drive the menu instead); esc hides suggestions.
- Tab completes the highlighted suggestion into the buffer.
- Keybinding framework separates *binding table* (keystroke → namespaced action id like `app:toggleTodos`) from *handlers* (components); resolution walks active contexts most-specific-first with global as fallback.

## Inc 1 baseline (current `main` @ `de66878`)

- `bun test`: **154 pass / 5 fail** — the 5 are documented baseline failures (`visualCellParity` ×2, `sessionSemanticIntegration`, `brainMemoryIntegration`, `brainTurnTransformer`). Zero NEW failures may be introduced; record final numbers.
- Bundle gate: `bun build src/main.tsx --outdir dist --target bun` → BUILD_OK.
- Smoke gate: `python3 scripts/ptySmokeInc2.py` → all PASS, exit 0.

## Toolchain gotchas (from memory + Inc 1 experience)

- cwd resets between Bash calls: always `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && …`.
- macOS APFS case-folds imports: never keep two files in one dir whose names differ only by case.
- Ink parses ONE stdin chunk as ONE keypress: PTY harnesses write text and Enter separately, pump ≥0.3 s between distinct keystrokes. (Multi-char text as ONE chunk is fine — it inserts as if pasted.)
- PTY needs `fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))` before exec; strip ANSI before matching rendered text.
- Repo test pattern: invoke PURE view functions directly (never mount reconcilers/hooked wrappers); live rendering verified only via PTY smoke.
- `error: daemon terminated` noise from git/bash is harmless.

---

### Task 1: Command registry + fuzzy matcher (pure)

**Files:**
- Create: `src/commands/registry.ts`
- Test: `src/test/commands/registry.test.ts`

**Interfaces:**
- Consumes: nothing (leaf module).
- Produces: `BrainCommand {name: string; description: string; aliases?: readonly string[]}`, `COMMANDS` (help/clear/quit — quit aliases `['q']`), `CommandMatch {command: BrainCommand; score: number}`, `parseCommandQuery(value: string): string | null` (returns the query WITHOUT leading `/`, or null when the buffer isn't a bare lowercase slash-token), `fuzzyMatchCommands(query: string, commands?: readonly BrainCommand[]): CommandMatch[]` (score desc, ties by name asc; empty query lists everything alphabetically).

- [ ] **Step 1: Write the failing test**

```ts
// src/test/commands/registry.test.ts
import { describe, it, expect } from 'bun:test';
import {
  COMMANDS,
  parseCommandQuery,
  fuzzyMatchCommands,
} from '../../commands/registry.js';

describe('parseCommandQuery', () => {
  it('accepts a bare slash token and rejects everything else', () => {
    expect(parseCommandQuery('/')).toBe('');
    expect(parseCommandQuery('/he')).toBe('he');
    expect(parseCommandQuery('/clear')).toBe('clear');
    expect(parseCommandQuery('help')).toBeNull();      // no slash
    expect(parseCommandQuery('/he rest')).toBeNull();  // args started → menu closed
    expect(parseCommandQuery('/HE')).toBeNull();       // queries are lowercase tokens
    expect(parseCommandQuery('x/he')).toBeNull();      // slash must lead
  });
});

describe('fuzzyMatchCommands', () => {
  const cmds = [
    { name: 'help', description: 'List available slash commands' },
    { name: 'clear', description: 'Clear the transcript' },
    { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
  ];

  it('lists everything alphabetically on empty query', () => {
    const names = fuzzyMatchCommands('', cmds).map((m) => m.command.name);
    expect(names).toEqual(['clear', 'help', 'quit']);
  });

  it('ranks prefixes above subsequences and breaks ties by name', () => {
    const extra = [...cmds, { name: 'clone', description: 'zzz' }];
    const names = fuzzyMatchCommands('c', extra).map((m) => m.command.name);
    // 'c' prefixes both clear and clone (tie → alphabetical); never quit.
    expect(names.indexOf('clone')).toBeGreaterThan(names.indexOf('clear'));
    expect(names[0]).toBe('clear');
    expect(names).not.toContain('quit'); // 'c' is not inside 'quit'
  });

  it('matches aliases and rejects misses deterministically', () => {
    expect(fuzzyMatchCommands('q', cmds)[0]!.command.name).toBe('quit');   // alias exact
    expect(fuzzyMatchCommands('hlp', cmds)[0]!.command.name).toBe('help'); // subsequence
    expect(fuzzyMatchCommands('zzz', cmds)).toEqual([]);                   // miss → []
    // description-word match still surfaces, below any name match
    const descOnly = [{ name: 'xyzzy', description: 'Transcript tool' }];
    expect(fuzzyMatchCommands('tran', descOnly)[0]!.command.name).toBe('xyzzy');
  });

  it('ships the three Inc 2 commands', () => {
    expect(COMMANDS.map((c) => c.name).sort()).toEqual(['clear', 'help', 'quit']);
    expect(COMMANDS.find((c) => c.name === 'quit')!.aliases).toEqual(['q']);
  });
});
```

Note for the tie test: `'c'` DOES match `help` via its description word "commands" (score 30) — the assertions deliberately tolerate that; they only pin clear/clone ordering and quit's absence.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/commands/registry.test.ts`
Expected: FAIL — module `commands/registry.js` not found.

- [ ] **Step 3: Write the implementation**

```ts
// src/commands/registry.ts
/**
 * Slash-command registry and fuzzy matcher. Pure data + functions: no I/O,
 * no React. Execution lives with the shell layer; this module only answers
 * "what matches what the user typed".
 */

export interface BrainCommand {
  /** Name without the leading '/'. Lowercase `[a-z0-9_-]+`. */
  name: string;
  /** One-line description shown in the palette and /help output. */
  description: string;
  aliases?: readonly string[];
}

export interface CommandMatch {
  command: BrainCommand;
  score: number;
}

/** The Inc 2 command set. Later increments register more here. */
export const COMMANDS: readonly BrainCommand[] = [
  { name: 'help', description: 'List available slash commands' },
  { name: 'clear', description: 'Clear the transcript' },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
];

/**
 * The palette is open iff the whole buffer is a bare slash token:
 * '/', '/c', '/clear' — never '/clear now' (args started) or 'x/y'.
 * Returns the query text without the leading '/', or null when closed.
 */
export function parseCommandQuery(value: string): string | null {
  const m = /^\/([a-z0-9_-]*)$/.exec(value);
  return m ? m[1]! : null;
}

/** Every char of needle appears in hay in order. */
function isSubsequence(needle: string, hay: string): boolean {
  let i = 0;
  for (const ch of hay) {
    if (ch === needle[i]) i++;
    if (i === needle.length) return true;
  }
  return needle.length === 0;
}

/**
 * Score tiers (higher wins): name exact 100 > alias exact 85 > name prefix
 * 80 > alias prefix 70 > name subsequence 60 > alias subsequence 50 >
 * description word prefix 30 > description word subsequence 20; '' matches
 * everything at 10. Ties break by name ascending — deterministic regardless
 * of registry order.
 */
export function fuzzyMatchCommands(
  query: string,
  commands: readonly BrainCommand[] = COMMANDS,
): CommandMatch[] {
  const q = query.toLowerCase();
  const matches: CommandMatch[] = [];
  for (const command of commands) {
    let score = 10; // empty query lists all
    if (q.length > 0) {
      const name = command.name.toLowerCase();
      const aliases = (command.aliases ?? []).map((a) => a.toLowerCase());
      if (name === q) score = 100;
      else if (aliases.includes(q)) score = 85;
      else if (name.startsWith(q)) score = 80;
      else if (aliases.some((a) => a.startsWith(q))) score = 70;
      else if (isSubsequence(q, name)) score = 60;
      else if (aliases.some((a) => isSubsequence(q, a))) score = 50;
      else {
        const words = command.description.toLowerCase().split(/\s+/).filter(Boolean);
        if (words.some((w) => w.startsWith(q))) score = 30;
        else if (words.some((w) => isSubsequence(q, w))) score = 20;
        else score = 0;
      }
    }
    if (score > 0) matches.push({ command, score });
  }
  return matches.sort(
    (a, b) => b.score - a.score || a.command.name.localeCompare(b.command.name),
  );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun test src/test/commands/registry.test.ts`
Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/commands/registry.ts packages/brain-shell/src/test/commands/registry.test.ts && git commit -m "feat(brain-shell): slash-command registry with deterministic fuzzy matcher

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Keybinding resolver + stroke normalization (pure)

**Files:**
- Create: `src/keybindings/resolve.ts`
- Modify: `src/ui/composer/translateKey.ts` (add optional `tab` to `KeyInfo`)
- Test: `src/test/keybindings/resolve.test.ts`

**Interfaces:**
- Consumes: `KeyInfo` from `ui/composer/translateKey.js`.
- Produces: `type KeybindingContextName = 'global' | 'composer' | 'palette'`; `interface BindingRule {action: string; context: KeybindingContextName; key: string}`; `DEFAULT_BINDINGS`; `strokeToKey(input: string, key: KeyInfo): string` (canonical ids: `'ctrl+c'`, `'ctrl+o'`, `'return'`, `'escape'`, `'tab'`, `'up'`, `'down'`, `'left'`, `'right'`, `'backspace'`, `'delete'`, else the literal char e.g. `'a'`, `'?'`); `resolveAction(bindings, contexts, keyId): string | null` — contexts ordered most-specific first, first hit wins, `'global'` consulted last.

- [ ] **Step 1: Extend KeyInfo with `tab`**

In `src/ui/composer/translateKey.ts`, add to the `KeyInfo` interface (after `delete`):

```ts
  tab?: boolean;
```

(`translateKey` itself stays unchanged — tab falls through to `noop`, which is correct for the editor; the palette layer consumes the flag directly.)

- [ ] **Step 2: Write the failing test**

```ts
// src/test/keybindings/resolve.test.ts
import { describe, it, expect } from 'bun:test';
import {
  DEFAULT_BINDINGS,
  resolveAction,
  strokeToKey,
} from '../../keybindings/resolve.js';
import type { KeyInfo } from '../../ui/composer/translateKey.js';

const key = (patch: Partial<KeyInfo>): KeyInfo => ({ ...patch });

describe('strokeToKey', () => {
  it('normalizes ink events into canonical key ids', () => {
    expect(strokeToKey('c', key({ ctrl: true }))).toBe('ctrl+c');
    expect(strokeToKey('o', key({ ctrl: true }))).toBe('ctrl+o');
    expect(strokeToKey('\r', key({ return: true }))).toBe('return');
    expect(strokeToKey('', key({ escape: true }))).toBe('escape');
    expect(strokeToKey('', key({ tab: true }))).toBe('tab');
    expect(strokeToKey('', key({ upArrow: true }))).toBe('up');
    expect(strokeToKey('', key({ downArrow: true }))).toBe('down');
    expect(strokeToKey('', key({ backspace: true }))).toBe('backspace');
    expect(strokeToKey('?', key({}))).toBe('?');
  });
});

describe('resolveAction', () => {
  it('resolves defaults with context precedence: specific beats global', () => {
    // ctrl+c is bound global-only; resolvable from any context list.
    expect(resolveAction(DEFAULT_BINDINGS, [], 'ctrl+c')).toBe('shell:exit');
    expect(resolveAction(DEFAULT_BINDINGS, ['palette'], 'ctrl+c')).toBe('shell:exit');
    // Unknown stroke → null.
    expect(resolveAction(DEFAULT_BINDINGS, ['composer'], '?')).toBeNull();
  });

  it('earlier (more specific) contexts win over later ones and global', () => {
    const bindings = [
      { action: 'composer:submit', context: 'composer' as const, key: 'return' },
      { action: 'palette:complete', context: 'palette' as const, key: 'tab' },
      { action: 'shell:exit', context: 'global' as const, key: 'ctrl+c' },
    ];
    expect(resolveAction(bindings, ['palette', 'composer'], 'tab')).toBe('palette:complete');
    expect(resolveAction(bindings, ['composer', 'palette'], 'return')).toBe('composer:submit');
    expect(resolveAction(bindings, ['palette'], 'ctrl+c')).toBe('shell:exit');
    expect(resolveAction(bindings, [], 'tab')).toBeNull();
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `bun test src/test/keybindings/resolve.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 4: Write the implementation**

```ts
// src/keybindings/resolve.ts
/**
 * Context-scoped keybinding resolver. Binding table (keystroke → namespaced
 * action id) is data; handlers stay in components. Resolution walks the
 * caller's context list most-specific-first and consults 'global' last.
 */
import type { KeyInfo } from '../ui/composer/translateKey.js';

export type KeybindingContextName = 'global' | 'composer' | 'palette';

export interface BindingRule {
  action: string;
  context: KeybindingContextName;
  /** Canonical key id from strokeToKey, e.g. 'ctrl+c', 'return', 'tab'. */
  key: string;
}

/** The shell's default table. Later increments extend, never reorder. */
export const DEFAULT_BINDINGS: readonly BindingRule[] = [
  { action: 'shell:exit', context: 'global', key: 'ctrl+c' },
  { action: 'shell:toggleTools', context: 'global', key: 'ctrl+o' },
  { action: 'composer:submit', context: 'composer', key: 'return' },
  { action: 'composer:abort', context: 'composer', key: 'escape' },
];

/**
 * Canonicalize an ink (input, key) event into a key id. Modifier prefixes
 * win, then named keys, then the literal character.
 */
export function strokeToKey(input: string, key: KeyInfo): string {
  if (key.ctrl && input.length === 1 && /[a-z]/.test(input)) return `ctrl+${input}`;
  if (key.escape) return 'escape';
  if (key.return) return 'return';
  if (key.tab) return 'tab';
  if (key.backspace) return 'backspace';
  if (key.delete) return 'delete';
  if (key.upArrow) return 'up';
  if (key.downArrow) return 'down';
  if (key.leftArrow) return 'left';
  if (key.rightArrow) return 'right';
  return input.length > 0 ? input : '';
}

export function resolveAction(
  bindings: readonly BindingRule[],
  contexts: readonly KeybindingContextName[],
  keyId: string,
): string | null {
  if (keyId.length === 0) return null;
  const order: KeybindingContextName[] = [...contexts, 'global'];
  for (const ctx of order) {
    const hit = bindings.find((b) => b.context === ctx && b.key === keyId);
    if (hit !== undefined) return hit.action;
  }
  return null;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `bun test src/test/keybindings/resolve.test.ts src/test/ui/composerState.test.ts`
Expected: resolve tests PASS; composerState tests still PASS (KeyInfo extension is additive).

- [ ] **Step 6: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/keybindings/resolve.ts packages/brain-shell/src/test/keybindings/resolve.test.ts packages/brain-shell/src/ui/composer/translateKey.ts && git commit -m "feat(brain-shell): context-scoped keybinding resolver with canonical strokes

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: `useBoundInput` hook + AppShell adoption

**Files:**
- Create: `src/keybindings/useBoundInput.ts`
- Modify: `src/ui/shell/AppShell.tsx` (swap the raw `useInput` block for the hook)
- Test: `src/test/keybindings/useBoundInput.test.tsx` (pins the dispatch rule; the hook wiring itself is verified by the PTY smoke per repo convention)

**Interfaces:**
- Consumes: `strokeToKey`, `resolveAction`, `DEFAULT_BINDINGS`, `BindingRule`, `KeybindingContextName` from Task 2; `useInput`, `Key` from `compat/index.js`.
- Produces: `useBoundInput(opts: {contexts: KeybindingContextName[]; bindings?: readonly BindingRule[]; isActive?: boolean; onAction: (action: string, input: string, key: Key) => void}): void`. Strokes that resolve fire `onAction` once and stop; unresolved strokes are ignored by the hook.

- [ ] **Step 1: Write the dispatch-rule test**

The hook is a thin composition of useInput + strokeToKey + resolveAction; the unit-testable surface is the dispatch decision it makes (resolved → handler, else ignore):

```tsx
// src/test/keybindings/useBoundInput.test.tsx
import { describe, it, expect } from 'bun:test';
import { DEFAULT_BINDINGS, resolveAction, strokeToKey } from '../../keybindings/resolve.js';

describe('useBoundInput dispatch rule', () => {
  const decide = (input: string, key: Parameters<typeof strokeToKey>[1]): string | null =>
    resolveAction(DEFAULT_BINDINGS, ['global'], strokeToKey(input, key));

  it('fires shell actions on their bound strokes', () => {
    expect(decide('c', { ctrl: true })).toBe('shell:exit');
    expect(decide('o', { ctrl: true })).toBe('shell:toggleTools');
  });

  it('ignores unbound strokes so they cannot double-handle', () => {
    expect(decide('x', { ctrl: true })).toBeNull();
    expect(decide('t', {})).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test**

Run: `bun test src/test/keybindings/useBoundInput.test.tsx`
Expected: this pins Task 2 exports, so it passes immediately — its job is guarding the contract the hook must preserve. Continue.

- [ ] **Step 3: Write the hook**

```ts
// src/keybindings/useBoundInput.ts
import { useInput } from '../compat/index.js';
import type { Key } from '../compat/index.js';
import { DEFAULT_BINDINGS, resolveAction, strokeToKey } from './resolve.js';
import type { BindingRule, KeybindingContextName } from './resolve.js';

/**
 * React seam over the keybinding resolver: fires onAction for bound strokes
 * in the given contexts, ignores everything else. Handlers stay in the
 * component; the table stays data.
 */
export function useBoundInput(opts: {
  contexts: KeybindingContextName[];
  bindings?: readonly BindingRule[];
  isActive?: boolean;
  onAction: (action: string, input: string, key: Key) => void;
}): void {
  const { contexts, bindings = DEFAULT_BINDINGS, isActive = true, onAction } = opts;
  useInput(
    (input, key) => {
      const keyId = strokeToKey(input, key);
      const action = resolveAction(bindings, contexts, keyId);
      if (action !== null) onAction(action, input, key);
    },
    { isActive },
  );
}
```

- [ ] **Step 4: Swap AppShell's raw useInput for the hook**

In `src/ui/shell/AppShell.tsx`, replace exactly this block (currently lines 28–35):

```tsx
  useInput((input, key) => {
    if (key.ctrl && input === 'c') {
      process.exit(0);
    }
    if (key.ctrl && input === 'o') {
      setExpandTools((v) => !v);
    }
  });
```

with:

```tsx
  useBoundInput({
    contexts: ['global'],
    onAction: (action) => {
      if (action === 'shell:exit') process.exit(0);
      if (action === 'shell:toggleTools') setExpandTools((v) => !v);
    },
  });
```

and update the import line from `{ Box, Text, useInput, useTerminalSize }` to `{ Box, Text, useTerminalSize }`, adding:

```tsx
import { useBoundInput } from '../../keybindings/useBoundInput.js';
```

- [ ] **Step 5: Verify no regressions + commit**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/keybindings/ && bun build src/main.tsx --outdir dist --target bun 2>&1 | tail -1`

Expected: tests pass; bundle prints a size line (BUILD_OK sanity — full gates rerun in Task 8).

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/keybindings/useBoundInput.ts packages/brain-shell/src/test/keybindings/useBoundInput.test.tsx packages/brain-shell/src/ui/shell/AppShell.tsx && git commit -m "feat(brain-shell): useBoundInput hook; AppShell binds through the table

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: `system` transcript row kind

**Files:**
- Modify: `src/contracts/messages.ts` (extend `TranscriptRow`, currently lines 191–195)
- Modify: `src/ui/transcript/MessageRow.tsx` (add `SystemRowView` + dispatch case)
- Test: `src/test/ui/messageRowView.test.tsx` (append case)

**Interfaces:**
- Consumes: existing `TranscriptRow` union; `MessageRow` memo dispatcher (switch on `row.kind`, no default — TypeScript exhaustiveness forces the new case).
- Produces: row shape `{kind: 'system'; id: string; text: string}` (`text` may be multi-line); `SystemRowView(props: {row, tokens})` rendering `ℹ ` dim glyph + subtle body.

- [ ] **Step 1: Write the failing test** — update the view import block in `messageRowView.test.tsx` to include `SystemRowView`, then append inside the existing `describe('row views')` (reuses its `textOf` walker and `TOKENS`):

```tsx
import {
  UserRowView,
  ThinkingRowView,
  ToolRowView,
  ErrorRowView,
  SystemRowView,
  summarizeToolInput,
} from '../../ui/transcript/MessageRow.js';
```

```tsx
  it('system row carries the ℹ glyph and dim body', () => {
    const out = textOf(
      SystemRowView({
        row: { kind: 'system', id: 'sys:1', text: 'Slash commands\n/help — List available slash commands' },
        tokens: TOKENS,
      }),
    );
    expect(out).toContain('ℹ');
    expect(out).toContain('/help — List available slash commands');
  });
```

(Dispatch through `MessageRow` is intentionally NOT unit-tested — it's a hooked memo wrapper and repo convention reserves live rendering for the PTY smoke. TypeScript's exhaustive switch makes a missing case a compile error, and Task 8's smoke shows system rows on screen.)

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/ui/messageRowView.test.tsx`
Expected: FAIL — `SystemRowView` not exported / union lacks `system`.

- [ ] **Step 3: Implement**

`src/contracts/messages.ts` — extend the union (beside `error`):

```ts
  | { kind: 'system'; id: string; text: string }
```

`src/ui/transcript/MessageRow.tsx` — add view (after `ErrorRowView`) and dispatch case (after `case 'error'`):

```tsx
export function SystemRowView(props: {
  row: Extract<TranscriptRow, { kind: 'system' }>;
  tokens: BrainTokens;
}): React.ReactElement {
  return (
    <Text>
      <Text dimColor>ℹ </Text>
      <Text color={props.tokens.subtle}>{props.row.text}</Text>
    </Text>
  );
}
```

Dispatcher switch gains:

```tsx
      case 'system':
        return <SystemRowView row={props.row} tokens={tokens} />;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun test src/test/ui/messageRowView.test.tsx src/test/ui/toRows.test.ts`
Expected: PASS (toRows untouched — confirms the union extension is backward-compatible).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/contracts/messages.ts packages/brain-shell/src/ui/transcript/MessageRow.tsx packages/brain-shell/src/test/ui/messageRowView.test.tsx && git commit -m "feat(brain-shell): system notice row kind for locally-generated output

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Controller `clear()` + `notice()`

**Files:**
- Modify: `src/state/sessionController.ts`
- Test: `src/test/state/sessionController.test.ts` (append inside the existing `describe('SessionController')`)

**Interfaces:**
- Consumes: existing `SessionController` rows/emit machinery; test helper `fakeClient(chunks)` returning `{client, requests}` (already defined in the test file).
- Produces: `clear(): void` — resets frozen rows to `[]` (busy/live untouched) and emits; `notice(text: string): void` — appends a `{kind:'system', id:'sys:<n>', text}` row and emits. Neither touches the wire.

- [ ] **Step 1: Write the failing tests** — append inside `describe('SessionController')`:

```ts
  it('records local notices and wipes the transcript on clear', async () => {
    const { client } = fakeClient(SCRIPT);
    const ctl = new SessionController(client);
    await ctl.submit('hi there');
    expect(ctl.getSnapshot().rows.length).toBeGreaterThan(0);

    ctl.notice('Slash commands');
    const withNotice = ctl.getSnapshot();
    const sys = withNotice.rows.find((r) => r.kind === 'system');
    expect(sys).toMatchObject({ kind: 'system', text: 'Slash commands' });

    ctl.clear();
    const after = ctl.getSnapshot();
    expect(after.rows).toEqual([]);
    expect(after.busy).toBe(false);
    expect(after).not.toBe(withNotice); // identity changed → UI updates
  });

  it('assigns unique ids across multiple notices', () => {
    const { client } = fakeClient([]);
    const ctl = new SessionController(client);
    ctl.notice('one');
    ctl.notice('two');
    const sysRows = ctl.getSnapshot().rows.filter((r) => r.kind === 'system');
    expect(sysRows).toHaveLength(2);
    expect(sysRows[0]!.id).not.toBe(sysRows[1]!.id);
  });
```

- [ ] **Step 2: Run to verify failure**

Run: `bun test src/test/state/sessionController.test.ts`
Expected: FAIL — `ctl.notice`/`ctl.clear` are not functions.

- [ ] **Step 3: Implement** — add to the `SessionController` class (after `abort()`):

```ts
  /** Wipe the frozen transcript (slash command /clear). */
  clear(): void {
    this.rows = [];
    this.emit();
  }

  private sysSeq = 0;

  /** Locally-generated shell output (/help, warnings). Never hits the wire. */
  notice(text: string): void {
    this.rows = [...this.rows, { kind: 'system', id: `sys:${++this.sysSeq}`, text }];
    this.emit();
  }
```

- [ ] **Step 4: Run to verify pass**

Run: `bun test src/test/state/sessionController.test.ts`
Expected: PASS (all, including prior settlement tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionController.test.ts && git commit -m "feat(brain-shell): controller clear() and notice() seams for local commands

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Palette pure layer — windowing, decisions, `replace_all`

**Files:**
- Create: `src/ui/composer/paletteLogic.ts`
- Modify: `src/ui/composer/composerState.ts` (add `replace_all` action)
- Test: `src/test/ui/paletteLogic.test.ts` (new), plus a case appended to `src/test/ui/composerState.test.ts`

**Interfaces:**
- Consumes: nothing yet (leaf pure module); `ComposerAction` union in composerState.
- Produces:
  - `PALETTE_MAX_ITEMS = 5`;
  - `paletteWindow(itemCount: number, selected: number, max?: number): {start: number; end: number}` (end exclusive; scroll keeps selection visible);
  - `type PaletteDecision = {kind:'move'; next:number} | {kind:'complete'; index:number} | {kind:'close'} | {kind:'passthrough'}`;
  - `paletteKeyDecision(opts: {open: boolean; cmdType: string; tab: boolean; selected: number; count: number}): PaletteDecision` — up/down move clamped without wrapping ONLY while open; tab completes ONLY while open (index = clamped selected); escape closes ONLY while open; everything else (including enter/submit and editing keys) passes through untouched. `cmdType` is a `KeyCommand['type']` value from translateKey; `tab` comes from `KeyInfo.tab`.

- [ ] **Step 1: Write the failing tests**

```ts
// src/test/ui/paletteLogic.test.ts
import { describe, it, expect } from 'bun:test';
import {
  paletteWindow,
  paletteKeyDecision,
  PALETTE_MAX_ITEMS,
} from '../../ui/composer/paletteLogic.js';

describe('paletteWindow', () => {
  it('keeps all items visible when under the cap', () => {
    expect(paletteWindow(3, 0)).toEqual({ start: 0, end: 3 });
    expect(paletteWindow(PALETTE_MAX_ITEMS, PALETTE_MAX_ITEMS - 1)).toEqual({
      start: 0,
      end: PALETTE_MAX_ITEMS,
    });
  });
  it('scrolls to keep the selection inside the window', () => {
    expect(paletteWindow(9, 0)).toEqual({ start: 0, end: 5 });
    expect(paletteWindow(9, 4)).toEqual({ start: 0, end: 5 });
    expect(paletteWindow(9, 5)).toEqual({ start: 1, end: 6 });
    expect(paletteWindow(9, 8)).toEqual({ start: 4, end: 9 });
  });
});

describe('paletteKeyDecision', () => {
  const decide = (cmdType: string, extra?: Partial<Parameters<typeof paletteKeyDecision>[0]>) =>
    paletteKeyDecision({ open: true, cmdType, tab: false, selected: 0, count: 3, ...extra });

  it('moves within bounds without wrapping while open', () => {
    expect(decide('history_up')).toEqual({ kind: 'move', next: 0 }); // clamped at top
    expect(decide('history_up', { selected: 1 })).toEqual({ kind: 'move', next: 0 });
    expect(decide('history_down')).toEqual({ kind: 'move', next: 1 });
    expect(decide('history_down', { selected: 2 })).toEqual({ kind: 'move', next: 2 }); // clamped at bottom
  });

  it('completes on tab and closes on escape while open', () => {
    expect(decide('noop', { tab: true })).toEqual({ kind: 'complete', index: 0 });
    expect(decide('noop', { tab: true, selected: 2 })).toEqual({ kind: 'complete', index: 2 });
    expect(decide('abort')).toEqual({ kind: 'close' });
  });

  it('passes submit and editing keys through while open', () => {
    expect(decide('submit')).toEqual({ kind: 'passthrough' }); // enter runs the command
    expect(decide('insert')).toEqual({ kind: 'passthrough' });
    expect(decide('backspace')).toEqual({ kind: 'passthrough' });
    expect(decide('exit')).toEqual({ kind: 'passthrough' });   // ctrl+c still exits
  });

  it('passes everything through while closed', () => {
    const closed = (cmdType: string, extra?: Partial<Parameters<typeof paletteKeyDecision>[0]>) =>
      paletteKeyDecision({ open: false, cmdType, tab: false, selected: 0, count: 0, ...extra });
    expect(closed('history_up')).toEqual({ kind: 'passthrough' });
    expect(closed('abort')).toEqual({ kind: 'passthrough' }); // esc aborts the turn elsewhere
    expect(closed('noop', { tab: true })).toEqual({ kind: 'passthrough' });
  });
});
```

Append to `src/test/ui/composerState.test.ts` (its imports already include `createComposerState`/`reduceComposer`):

```ts
describe('replace_all action', () => {
  it('replaces the whole buffer, parks cursor at end, pushes undo', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'old text' });
    s = reduceComposer(s, { type: 'replace_all', value: '/help ' });
    expect(s.value).toBe('/help ');
    expect(s.cursor).toBe(6);
    s = reduceComposer(s, { type: 'undo' });
    expect(s.value).toBe('old text');
  });
});
```

- [ ] **Step 2: Run to verify failures**

Run: `bun test src/test/ui/paletteLogic.test.ts src/test/ui/composerState.test.ts`
Expected: FAIL — paletteLogic missing; `replace_all` hits the reducer default (value stays 'old text').

- [ ] **Step 3: Implement**

```ts
// src/ui/composer/paletteLogic.ts
/**
 * Pure decision layer for the slash-command palette. The component applies
 * these verdicts; all branching lives here so it stays unit-testable.
 */
export const PALETTE_MAX_ITEMS = 5;

export interface PaletteWindow {
  start: number;
  end: number; // exclusive
}

export function paletteWindow(
  itemCount: number,
  selected: number,
  max: number = PALETTE_MAX_ITEMS,
): PaletteWindow {
  if (itemCount <= max) return { start: 0, end: itemCount };
  const start = Math.max(0, Math.min(selected - max + 1, itemCount - max));
  return { start, end: start + max };
}

export type PaletteDecision =
  | { kind: 'move'; next: number }
  | { kind: 'complete'; index: number }
  | { kind: 'close' }
  | { kind: 'passthrough' };

export function paletteKeyDecision(opts: {
  open: boolean;
  cmdType: string;
  tab: boolean;
  selected: number;
  count: number;
}): PaletteDecision {
  const { open, cmdType, tab, selected, count } = opts;
  if (!open || count === 0) return { kind: 'passthrough' };
  if (cmdType === 'history_up') return { kind: 'move', next: Math.max(0, selected - 1) };
  if (cmdType === 'history_down')
    return { kind: 'move', next: Math.min(count - 1, selected + 1) };
  if (tab) return { kind: 'complete', index: Math.min(selected, count - 1) };
  if (cmdType === 'abort') return { kind: 'close' };
  return { kind: 'passthrough' };
}
```

In `src/ui/composer/composerState.ts` — extend `ComposerAction` with:

```ts
  | { type: 'replace_all'; value: string }
```

and add the reducer case beside `undo`:

```ts
    case 'replace_all':
      return pushUndo(
        { ...state, value: action.value, cursor: action.value.length },
        state,
      );
```

- [ ] **Step 4: Run to verify pass**

Run: `bun test src/test/ui/paletteLogic.test.ts src/test/ui/composerState.test.ts`
Expected: PASS (existing composer cases plus the new ones).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/composer/paletteLogic.ts packages/brain-shell/src/ui/composer/composerState.ts packages/brain-shell/src/test/ui/paletteLogic.test.ts packages/brain-shell/src/test/ui/composerState.test.ts && git commit -m "feat(brain-shell): palette windowing + key-decision layer; replace_all edit op

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: `PaletteView` + PromptInput integration

**Files:**
- Create: `src/ui/composer/PaletteView.tsx`
- Modify: `src/ui/composer/PromptInput.tsx`
- Test: `src/test/ui/paletteView.test.tsx`

**Interfaces:**
- Consumes: Tasks 1 & 6 (`parseCommandQuery`, `fuzzyMatchCommands`, `paletteWindow`, `paletteKeyDecision`, `PALETTE_MAX_ITEMS`); `BrainTokens`/`PALETTES` from `state/palettes.js`; `Box`/`Text` from compat.
- Produces:
  - `interface PaletteItemVM {name: string; description: string}`;
  - `PaletteView(props: {items: PaletteItemVM[]; selectedIndex: number; maxColumns: number; tokens: BrainTokens}): React.ReactElement | null` — rounded-border panel listing up to `PALETTE_MAX_ITEMS` windowed rows; selected row prefixed `❯ ` and inverse-video, others two spaces; lines truncated to `maxColumns`; `null` for empty items.
  - `PromptInput` props UNCHANGED (`disabled?, busy?, onSubmit, onAbort?`) — the palette is visual/navigational only; submission still flows through `onSubmit(bare)` and AppShell routes it (Task 8).

- [ ] **Step 1: Write the failing test**

```tsx
// src/test/ui/paletteView.test.tsx
import { describe, it, expect } from 'bun:test';
import * as React from 'react';
import { PaletteView } from '../../ui/composer/PaletteView.js';
import { PALETTES } from '../../state/palettes.js';

function textOf(el: React.ReactNode): string {
  if (el === null || el === undefined || typeof el === 'boolean') return '';
  if (typeof el === 'string' || typeof el === 'number') return String(el);
  if (Array.isArray(el)) return el.map(textOf).join('');
  if (typeof el === 'object' && el !== null && 'props' in el) {
    return textOf((el as React.ReactElement).props.children);
  }
  return '';
}

const TOKENS = PALETTES.dark;

describe('PaletteView', () => {
  const items = [
    { name: 'help', description: 'List available slash commands' },
    { name: 'clear', description: 'Clear the transcript' },
    { name: 'quit', description: 'Exit Brain shell' },
  ];

  it('renders nothing for an empty list', () => {
    expect(PaletteView({ items: [], selectedIndex: 0, maxColumns: 80, tokens: TOKENS })).toBeNull();
  });

  it('marks the selected row with ❯ and shows descriptions', () => {
    const out = textOf(PaletteView({ items, selectedIndex: 1, maxColumns: 80, tokens: TOKENS })!);
    expect(out).toContain('❯ /clear');
    expect(out).toContain('Clear the transcript');
    expect(out).toContain('/help');
    expect(out).toContain('/quit');
  });

  it('windows long lists around the selection', () => {
    const many = Array.from({ length: 9 }, (_, i) => ({ name: `cmd${i}`, description: 'd' }));
    const out = textOf(PaletteView({ items: many, selectedIndex: 6, maxColumns: 80, tokens: TOKENS })!);
    expect(out).toContain('/cmd6');
    expect(out).not.toContain('/cmd0'); // scrolled off the front
    expect(out).toContain('❯');
  });

  it('truncates rows to maxColumns', () => {
    const wide = [{ name: 'help', description: 'x'.repeat(200) }];
    const out = textOf(PaletteView({ items: wide, selectedIndex: 0, maxColumns: 40, tokens: TOKENS })!);
    expect(out.length).toBeLessThanOrEqual(41); // 40 cols + ellipsis tolerance
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `bun test src/test/ui/paletteView.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement PaletteView**

```tsx
// src/ui/composer/PaletteView.tsx
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import { paletteWindow, PALETTE_MAX_ITEMS } from './paletteLogic.js';

export interface PaletteItemVM {
  name: string;
  description: string;
}

/**
 * Rounded-border suggestion panel rendered ABOVE the composer while a slash
 * query is active. Pure view: explicit tokens, direct invocation in tests.
 */
export function PaletteView(props: {
  items: PaletteItemVM[];
  selectedIndex: number;
  maxColumns: number;
  tokens: BrainTokens;
}): React.ReactElement | null {
  const { items, selectedIndex, maxColumns, tokens } = props;
  if (items.length === 0) return null;
  const sel = Math.min(Math.max(0, selectedIndex), items.length - 1);
  const { start, end } = paletteWindow(items.length, sel);
  return (
    <Box flexDirection="column" borderStyle="round" paddingX={1}>
      {items.slice(start, end).map((item, i) => {
        const idx = start + i;
        const isSelected = idx === sel;
        const label = `${isSelected ? '❯' : ' '} /${item.name} — ${item.description}`;
        const shown =
          label.length > maxColumns
            ? `${label.slice(0, Math.max(1, maxColumns - 1))}…`
            : label;
        return (
          <Text key={item.name} inverse={isSelected}>
            {shown}
          </Text>
        );
      })}
    </Box>
  );
}

export { PALETTE_MAX_ITEMS };
```

- [ ] **Step 4: Run to verify pass**

Run: `bun test src/test/ui/paletteView.test.tsx`
Expected: PASS.

- [ ] **Step 5: Integrate into PromptInput**

Five edits to `src/ui/composer/PromptInput.tsx`:

**(a)** Add `useTerminalSize` to the compat import line (it currently reads `import { Box, Text, useInput, useTheme } from '../../compat/index.js';`) and add new module imports:

```tsx
import { Box, Text, useInput, useTheme, useTerminalSize } from '../../compat/index.js';
```

```tsx
import { parseCommandQuery, fuzzyMatchCommands } from '../../commands/registry.js';
import { paletteKeyDecision } from './paletteLogic.js';
import { PaletteView } from './PaletteView.js';
import type { PaletteItemVM } from './PaletteView.js';
```

**(b)** Pass ink's `tab` flag through `asKeyInfo` (ink sets `key.tab`; without this, completion can't fire):

```ts
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
    tab: (key as { tab?: boolean }).tab,
  };
}
```

**(c)** Inside `PromptInput`, hoist palette state and derivation ABOVE the `useInput` call (hooks never inside handlers/JSX):

```tsx
  const { columns } = useTerminalSize();
  const [selected, setSelected] = React.useState(0);
  const [suppressed, setSuppressed] = React.useState(false);

  // Palette is open iff the whole buffer is a bare slash query. Esc sets
  // `suppressed` (esc means abort once the menu is dismissed); clearing the
  // buffer re-arms it.
  const query = parseCommandQuery(state.value);
  const matches =
    query !== null && !suppressed && !(props.busy ?? false)
      ? fuzzyMatchCommands(query)
      : [];
  const paletteItems: PaletteItemVM[] = matches.map((m) => ({
    name: m.command.name,
    description: m.command.description,
  }));
  const paletteOpen = paletteItems.length > 0;
  React.useEffect(() => {
    setSelected(0);
  }, [query]);
  React.useEffect(() => {
    if (state.value.length === 0) setSuppressed(false);
  }, [state.value]);
```

**(d)** At the TOP of the `useInput` handler — immediately after `const cmd = translateKey(input, asKeyInfo(key));` and BEFORE the `if (cmd.type === 'exit')` branch (the intercept must precede the abort branch so esc closes the menu instead of aborting a turn) — insert:

```tsx
    const info = asKeyInfo(key);
    const decision = paletteKeyDecision({
      open: paletteOpen,
      cmdType: cmd.type,
      tab: info.tab ?? false,
      selected: Math.min(selected, Math.max(0, matches.length - 1)),
      count: matches.length,
    });
    if (decision.kind === 'move') {
      setSelected(decision.next);
      return;
    }
    if (decision.kind === 'complete') {
      const chosen = matches[decision.index]!.command;
      setState((s) => reduceComposer(s, { type: 'replace_all', value: `/${chosen.name} ` }));
      return;
    }
    if (decision.kind === 'close') {
      setSuppressed(true);
      return;
    }
```

If applying the efficiency nit, also change the existing translate call site to reuse the same `info`: `const cmd = translateKey(input, info);`.

Behavior preserved: enter (`submit`) passes through and submits as before; ctrl+c (`exit`) passes through; up/down drive the menu only while it shows matches (history nav otherwise); esc closes the menu when open and aborts the turn when closed.

**(e)** Replace the return statement with the palette stacked above the existing view:

```tsx
  return (
    <Box flexDirection="column">
      {paletteOpen ? (
        <PaletteView
          items={paletteItems}
          selectedIndex={Math.min(selected, paletteItems.length - 1)}
          maxColumns={columns}
          tokens={tokens}
        />
      ) : null}
      <PromptInputView value={state.value} cursor={state.cursor} busy={props.busy ?? false} tokens={tokens} />
    </Box>
  );
```

- [ ] **Step 6: Targeted sweep**

Run: `bun test src/test/ui/ src/test/commands/ src/test/keybindings/`
Expected: all PASS — especially promptInputView cases (view signature untouched) and composerState (new `replace_all`).

- [ ] **Step 7: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/composer/PaletteView.tsx packages/brain-shell/src/ui/composer/PromptInput.tsx packages/brain-shell/src/test/ui/paletteView.test.tsx && git commit -m "feat(brain-shell): slash palette above the composer — filter, navigate, tab-complete

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: AppShell command routing + footer + PTY smoke

**Files:**
- Modify: `src/ui/shell/AppShell.tsx`
- Append: `src/test/commands/registry.test.ts` (executor lookup contract)
- Create: `scripts/ptySmokeInc2.py` (repo-root `scripts/`)
- Create (generated by harness): `packages/brain-shell/src/test/fixtures/pty/inc2/{palette,executed,cleared}.txt`
- Test: full suite + smoke run

**Interfaces:**
- Consumes: `COMMANDS` (Task 1), `controller.notice/clear/submit` (Task 5), PromptInput palette (Task 7), `useBoundInput` (Task 3, already wired).
- Produces: local executor `runCommand(rawValue)` resolving in order — exact name/alias → unique name-prefix match → ambiguous warning → unknown warning. `/help` prints a multi-line system notice built from `COMMANDS`; `/clear` calls `controller.clear()`; `/quit` exits. Footer gains `/ commands`.

- [ ] **Step 1: Pin the lookup contract** — append to `src/test/commands/registry.test.ts`:

```ts
describe('executor lookup contract', () => {
  it('finds commands by name or alias; prefix resolves only when unique', () => {
    const find = (token: string) =>
      COMMANDS.find((c) => c.name === token || (c.aliases ?? []).includes(token));
    expect(find('help')?.name).toBe('help');
    expect(find('q')?.name).toBe('quit');     // alias-exact
    expect(find('zzz')).toBeUndefined();      // unknown → notice path
    const prefixHits = COMMANDS.filter((c) => c.name.startsWith('he'));
    expect(prefixHits.map((c) => c.name)).toEqual(['help']); // unique prefix
  });
});
```

- [ ] **Step 2: Wire routing in AppShell**

Add import:

```tsx
import { COMMANDS } from '../../commands/registry.js';
```

Inside `AppShell`, define the executor and route submits (replacing the current inline `onSubmit={(text) => void controller.submit(text)}`):

```tsx
  const helpText = (): string =>
    ['Slash commands:', ...COMMANDS.map((c) => `/${c.name} — ${c.description}`)].join('\n');

  const runCommand = (rawValue: string): void => {
    const token = rawValue.trim().slice(1).toLowerCase(); // strip '/', tolerate trailing space
    if (token.length === 0) return;
    const exact = COMMANDS.find(
      (c) => c.name === token || (c.aliases ?? []).includes(token),
    );
    let chosen = exact;
    if (chosen === undefined) {
      const prefixHits = COMMANDS.filter((c) => c.name.startsWith(token));
      if (prefixHits.length === 1) chosen = prefixHits[0];
      else if (prefixHits.length > 1) {
        controller.notice(`Ambiguous command: /${token}`);
        return;
      } else {
        controller.notice(`Unknown command: /${token}`);
        return;
      }
    }
    if (chosen.name === 'help') controller.notice(helpText());
    else if (chosen.name === 'clear') controller.clear();
    else if (chosen.name === 'quit') process.exit(0);
  };

  const handleSubmit = (text: string): void => {
    if (text.trimStart().startsWith('/')) runCommand(text);
    else void controller.submit(text);
  };
```

PromptInput invocation becomes:

```tsx
        <PromptInput
          disabled={false}
          busy={snapshot.busy}
          onSubmit={handleSubmit}
          onAbort={() => controller.abort()}
        />
```

Footer `<Text dimColor>` line becomes:

```tsx
      <Text dimColor>
        model: {model} · ! bash · / commands · ↑↓ history · esc stop · ctrl+o{' '}
        {expandTools ? 'collapse' : 'expand'} tools · ctrl+c exit
      </Text>
```

- [ ] **Step 3: Write the PTY smoke harness** — `scripts/ptySmokeInc2.py`, verbatim (authoritative):

```python
#!/usr/bin/env python3
"""Increment 2 PTY smoke: slash palette, navigation, tab-completion,
/help execution, /clear wipe, and the bash-strip wire regression.

Discipline (carried from ptySmokeInc1.py): stub UDS daemon, winsize ioctl
before exec, discrete keystroke writes with >=0.3 s pumps between distinct
keys (ink parses one stdin chunk as one keypress), ANSI-stripped matching.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc2-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc2-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc2"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

# ── Stub daemon: session-create + instant finished frame (only Flow D talks to it).
def serve():
    if os.path.exists(SOCK):
        os.remove(SOCK)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(8)
    while True:
        conn, _ = srv.accept()
        def handle(conn=conn):
            fobj = conn.makefile("rw")
            try:
                for line in fobj:
                    req = json.loads(line)
                    with open(FRAMES_FILE, "a") as log:
                        log.write(json.dumps(req) + "\n")
                    rid = req.get("id")
                    if req.get("action") == "v1/session/create":
                        fobj.write(json.dumps({"id": rid, "status": "success",
                                               "body": {"session_id": "stub-session-2"}}) + "\n")
                        fobj.flush()
                    elif req.get("action") == "v1/generation/stream":
                        fobj.write(json.dumps({"type": "finished", "status": "completed",
                                               "sequence": 0}) + "\n")
                        fobj.flush()
            except Exception:
                pass
            finally:
                try:
                    conn.close()
                except Exception:
                    pass
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

def snapshot(name):
    os.makedirs(FIXTURE_DIR, exist_ok=True)
    with open(os.path.join(FIXTURE_DIR, name + ".txt"), "w") as f:
        f.write(clean(buf))

def expect(label, needle, timeout=8.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print("PASS " + label)
            return True
    print("FAIL %s: %r not seen" % (label, needle))
    return False

ok = True

# ── Flow A: launch, open palette, navigate ─────────────────────────────────
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")
os.write(fd, b"/")                       # opens palette with ALL commands
ok &= expect("palette-listed", "/help")
ok &= expect("palette-desc", "List available slash commands")
os.write(fd, b"\x1b[B")                  # ↓ selection moves to /clear
ok &= expect("palette-nav-clear", "❯ /clear")
os.write(fd, b"\x1b[A")                  # ↑ back to /help
ok &= expect("palette-nav-help", "❯ /help")
snapshot("palette")
os.write(fd, b"\x1b")                    # esc closes the menu…
pump(0.3)
os.write(fd, b"\x7f")                    # …then backspace empties '/'
pump(0.3)

# ── Flow B: filtered palette + execute /help via prefix resolution ─────────
os.write(fd, b"/he")
pump(0.3)
ok &= expect("palette-filtered", "/help")
os.write(fd, b"\r")                      # enter submits '/he' → unique prefix → help
ok &= expect("help-header", "Slash commands:")
ok &= expect("help-body", "/quit — Exit Brain shell")
snapshot("executed")

# ── Flow C: tab-completion + /clear wipes the transcript ──────────────────
os.write(fd, b"/cl")
pump(0.3)
os.write(fd, b"\t")                      # completes buffer to '/clear '
pump(0.3)
ok &= expect("tab-completed", "/clear ")
os.write(fd, b"\r")
deadline = time.time() + 8
cleared = False
while time.time() < deadline:
    pump(0.1)
    # Compare only the tail after the last mark render: earlier frames still
    # carry Flow B's help output, but /clear removes it from the live screen.
    tail = clean(buf).split("◆ BRAIN")[-1]
    if "Exit Brain shell" not in tail and "❯" in tail:
        cleared = True
        break
print(("PASS" if cleared else "FAIL") + " clear-executed")
ok &= cleared

# ── Flow D: regression — bash strip still hits the wire bare ──────────────
os.write(fd, b"!echo hi")                # multi-char chunk inserts as text
pump(0.3)
os.write(fd, b"\r")                      # enter is its own keystroke
deadline = time.time() + 8
stripped = False
while time.time() < deadline:
    pump(0.1)
    try:
        with open(FRAMES_FILE) as f:
            for line in f:
                req = json.loads(line)
                msgs = (req.get("payload") or {}).get("messages") or []
                if msgs and isinstance(msgs[-1].get("content"), str) \
                        and msgs[-1]["content"].strip() == "echo hi":
                    stripped = True
    except FileNotFoundError:
        pass
    if stripped:
        break
print(("PASS" if stripped else "FAIL") + " bash-strip")
ok &= stripped

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
snapshot("cleared")

sys.exit(0 if ok else 1)
```

- [ ] **Step 4: Full suite + smoke**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "pass|fail"`
Expected: zero NEW failures vs the 154-pass/5-baseline reference.

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc2.py; echo EXIT=$?`
Expected: every flow PASS, EXIT=0. On first-run failure, retry once (transpile-cache warmup) before investigating.

- [ ] **Step 5: Final gates**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun build src/main.tsx --outdir dist --target bun 2>&1 | tail -2 && echo BUILD_OK
git grep -il "claude\|anthropic\|vendor" HEAD -- 'packages/brain-shell/src/*.ts' 'packages/brain-shell/src/*.tsx' 'packages/brain-shell/src/**/*.ts' 'packages/brain-shell/src/**/*.tsx' | grep -v "test/" || echo CLEAN
```
Expected: recorded numbers, BUILD_OK, CLEAN.

- [ ] **Step 6: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/commands/registry.test.ts scripts/ptySmokeInc2.py packages/brain-shell/src/test/fixtures/pty/inc2/palette.txt packages/brain-shell/src/test/fixtures/pty/inc2/executed.txt packages/brain-shell/src/test/fixtures/pty/inc2/cleared.txt && git commit -m "test(shell): inc2 command surface — palette nav, prefix execution, /clear, bash regression

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Scope decisions (documented deferrals)

- **No fuse.js**: dependency-free tiered scorer replaces it; deterministic tie-breaking by name.
- **Chords** (`ctrl+k ctrl+s`) deferred — the resolver is per-stroke; the table shape accepts later chord support without breaking callers.
- **History interplay**: while the palette shows ≥1 match, ↑↓ drive the menu; history nav resumes when the menu closes (esc, non-slash edit, or execution).
- **Enter-with-open-menu submits the buffer**, and the executor's unique-prefix rule makes `/he`⏎ run `/help` — matching the reference's "enter runs the highlighted suggestion" feel without coupling execution into the composer.
- **Commands don't touch the daemon**: `/help` and warnings render as local `system` rows; `/clear` wipes frozen rows client-side; `/quit` exits.
- **Vim mode, custom user commands, dynamic registries**: out of scope per spec §5.

## Final gates (all must hold before finishing-a-development-branch)

1. `bun test` — zero NEW failures vs **154-pass/5-baseline-fail**; record numbers.
2. `bun build src/main.tsx --outdir dist --target bun` → BUILD_OK (full-cache purge on vendor storm).
3. `python3 scripts/ptySmokeInc2.py` → all flows PASS, exit 0.
4. Vendor grep on committed tree → CLEAN.
5. Explicit-path audit of every branch commit.
