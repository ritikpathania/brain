# Brain Shell Inc 21 — Command Surface II (/doctor, /memory, Canonical Registry) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `/doctor` and `/memory` through a single canonical command registry, retiring the duplicate matcher catalog, with zero IPC/schema changes.

**Architecture:** `commands/registry.ts` becomes the pure catalog + declarative-result contract (`text | none | action | overlay`); `commands/builtin.ts` registers all eight commands; AppShell interprets results into shell state exactly like the theme/resume overlays it already hosts. Two new pure overlay views (`ui/overlays/DoctorOverlayView.tsx`, `ui/overlays/MemoryOverlayView.tsx`) plus a shared `ModalFrame.tsx` replace superseded orphaned components; `/memory` rides the existing `searchMemory` client method over the existing `v1/memory/search` RPC.

**Tech Stack:** Bun + React 19 + Ink 7 + TypeScript; Python 3 PTY harness. **No Rust surface — zero cargo scope.**

**Spec:** `docs/superpowers/specs/2026-08-26-brain-shell-inc21-command-surface-doctor-memory-design.md` (committed `aa9a3554`). The plan argues from the spec; executors read both.

## Global Constraints

- Every commit contains ONLY explicitly-added paths (`git add <paths>`, never `git add .`); trailer `Co-Authored-By: Claude <noreply@anthropic.com>` on every commit.
- Working-tree user WIP (~3.7k dirty paths) is never staged, stashed, or reverted. Specifically NEVER create/import/reuse these untracked-WIP paths: `packages/brain-shell/src/components/*`, `packages/brain-shell/src/adapter/BrainMemoryService.*`.
- `packages/brain-shell/src/adapter/doctorProbe.ts` is NOT modified in this increment (its line-3 comment contains a vendor word; untouched file keeps the added-lines vendor scan clean).
- `packages/brain-shell/src/test/contracts/shell.test.tsx` is TRACKED but USER-WIP-DIRTY. If a task must touch it: first `git diff HEAD -- <file>` to inspect foreign hunks; if foreign hunks coexist with needed edits, use the HEAD-blob rebuild recipe (write `git show HEAD:<path>` to temp, apply only planned edits with uniqueness assertions, `git hash-object -w` + `git update-index --cacheinfo 100644,<hash>,<path>`), then verify `git diff --cached` line-by-line before commit.
- Vendor gate after every task: `git diff aa9a3554..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'` → expect `0`.
- Bun suite gate: failure identities ⊆ documented five (visualCellParity ×2, sessionSemanticIntegration, brainMemoryIntegration, brainTurnTransformer). Absolute pass-count drift is fine; identity drift is not.
- tsc gate: `cd packages/brain-shell && bun x tsc --noEmit 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -c "error TS"` — absolute count must stay ≤ baseline 434 + ambient-family deltas only.
- Bundle gate: `cd packages/brain-shell && bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK`.
- PTY harness rules (all smokes): TIOCSWINSZ before exec; one keystroke per write with ≥0.3 s pumps (0.15 s within typed strings); strip ANSI before matching; occurrence-count waits for repeated UI; prefer behavioral assertions over tail scans (Ink differential rendering gotcha).
- Branch: `feature/brain-shell-inc21-command-surface-doctor-memory` off `main` @ `aa9a3554`. Session works IN PLACE — no worktree.

## File Structure

| Path | Create/Modify/Delete | Responsibility |
|---|---|---|
| `packages/brain-shell/src/commands/registry.ts` | Modify (rewrite) | Catalog Map + `Command`/`CommandResult` contracts |
| `packages/brain-shell/src/commands/builtin.ts` | Create | Eight built-in registrations (six in T1; doctor in T4; memory in T5) |
| `packages/brain-shell/src/commands/matcher.ts` | Modify | Pure palette functions only; `COMMANDS` deleted |
| `packages/brain-shell/src/ui/composer/PromptInput.tsx` | Modify (1 line) | Feed `getCommands()` to the palette matcher |
| `packages/brain-shell/src/ui/shell/AppShell.tsx` | Modify | Result interpreter; doctor/memory overlay state, effects, input blocks, renders |
| `packages/brain-shell/src/ui/overlays/ModalFrame.tsx` | Create | Shared bordered modal frame |
| `packages/brain-shell/src/ui/overlays/DoctorOverlayView.tsx` | Create | Pure diagnostics report view |
| `packages/brain-shell/src/ui/overlays/MemoryOverlayView.tsx` | Create | Pure knowledge-search view |
| `packages/brain-shell/src/ui/overlays/memoryOverlayLogic.ts` | Create | Pure score/clamp helpers (result type lives in the client contract) |
| `packages/brain-shell/src/state/sessionController.ts` | Modify | `searchMemories()` liveness wrapper |
| `packages/brain-shell/src/client/UdsBrainBackendClient.ts` | Modify (1 line) | Preserve `relations` in `searchMemory` mapping |
| `packages/brain-shell/src/client/BrainBackendClient.ts` | Modify | `MemorySearchResult` liveness type beside `RetrievedMemory` |
| `packages/brain-shell/src/commands/doctor/DoctorCommand.tsx` | Delete (T4) | Superseded orphaned component |
| `packages/brain-shell/src/commands/memory/MemoryCommand.tsx` | Delete (T5) | Superseded orphaned component |
| `packages/brain-shell/src/test/contracts/commandRegistry.test.ts` | Modify (rewrite, T1; extend T4/T5) | Registry + builtin coverage |
| `packages/brain-shell/src/test/commands/matcher.test.ts` | Modify (rewrite, T2) | Parameterized matcher coverage |
| `packages/brain-shell/src/test/ui/overlays/modalFrame.test.tsx` | Create (T3) | Frame view coverage |
| `packages/brain-shell/src/test/ui/overlays/doctorOverlayView.test.tsx` | Create (T4) | Report view coverage |
| `packages/brain-shell/src/test/ui/overlays/memoryOverlayView.test.tsx` | Create (T5) | Search view coverage |
| `packages/brain-shell/src/test/ui/overlays/memoryOverlayLogic.test.ts` | Create (T5) | Helper coverage |
| `packages/brain-shell/src/test/client/memorySearchWire.test.ts` | Create (T5) | Scripted-daemon relations preservation |
| `scripts/ptySmokeInc21.py` | Create (T6) | Real-daemon end-to-end proof |

Untracked-WIP files that EXIST ON DISK at colliding names and must be left alone: `src/components/BrainModal.tsx`, `src/components/BrainSearchField.tsx`, `src/components/BrainTabHeader.tsx` (if present), `src/adapter/BrainMemoryService.ts`.

---

### Task 1: Canonical registry contract + six-command builtin catalog

**Files:**
- Modify (rewrite): `packages/brain-shell/src/commands/registry.ts`
- Create: `packages/brain-shell/src/commands/builtin.ts`
- Test (rewrite): `packages/brain-shell/src/test/contracts/commandRegistry.test.ts`

**Interfaces:**
- Consumes: `runPermissionsCommand(args: string[]): string` from `../state/permissionRules.js` (existing, sync).
- Produces: `registerCommand(cmd: Command)`, `getCommands(): Command[]` (name-sorted), `getCommand(name: string): Command | undefined`; types `CommandResult = {type:'text';value:string} | {type:'none'} | {type:'action';action:'clear'|'quit'|'resume'|'theme'} | {type:'overlay';overlay:'doctor'|'memory'}`, `CommandContext = {args: string[]; sessionId?: string}`, `Command`. Side-effect registration on first import of `./builtin.js`. Task 4 appends the `doctor` entry; Task 5 appends `memory`.

- [ ] **Step 1: Rewrite the failing test**

Replace the entire content of `packages/brain-shell/src/test/contracts/commandRegistry.test.ts`:

```ts
import { describe, expect, test } from 'bun:test';
import {
  getCommand,
  getCommands,
  registerCommand,
  type Command,
} from '../../commands/registry.js';
import '../commands/builtin.js'; // side-effect: registers the built-in catalog

const NAMES = ['help', 'clear', 'resume', 'theme', 'permissions', 'quit'];

describe('contracts/commandRegistry (Inc 21)', () => {
  test('builtin catalog registers all six launch commands', () => {
    for (const n of NAMES) expect(getCommand(n)).toBeDefined();
  });

  test('alias q resolves to quit', () => {
    expect(getCommand('q')?.name).toBe('quit');
  });

  test('catalog is name-sorted and duplicate-free', () => {
    const all = getCommands().map((c) => c.name);
    expect(new Set(all).size).toBe(all.length);
    expect([...all].sort((a, b) => a.localeCompare(b))).toEqual(all);
  });

  test('help run returns text naming every catalog entry', () => {
    const out = getCommand('help')!.run({ args: [] });
    expect(out.type).toBe('text');
    if (out.type !== 'text') return;
    for (const n of getCommands().map((c) => c.name)) {
      expect(out.value).toContain(`/${n}`);
    }
  });

  test('clear/resume/theme/quit return declarative actions', () => {
    expect(getCommand('clear')!.run({ args: [] })).toEqual({ type: 'action', action: 'clear' });
    expect(getCommand('resume')!.run({ args: [] })).toEqual({ type: 'action', action: 'resume' });
    expect(getCommand('theme')!.run({ args: [] })).toEqual({ type: 'action', action: 'theme' });
    expect(getCommand('quit')!.run({ args: [] })).toEqual({ type: 'action', action: 'quit' });
  });

  test('permissions passes args to the rules engine and returns text', () => {
    const out = getCommand('permissions')!.run({ args: ['list'] });
    expect(out.type).toBe('text');
  });

  test('registerCommand replaces by name and resolves aliases', () => {
    const a: Command = { name: 'ping', description: 'v1', run: () => ({ type: 'none' }) };
    registerCommand(a);
    registerCommand({
      name: 'ping',
      description: 'v2',
      aliases: ['p'],
      run: () => ({ type: 'text', value: 'pong' }),
    });
    expect(getCommand('ping')?.description).toBe('v2');
    expect(getCommand('p')?.name).toBe('ping');
    expect(getCommands().filter((c) => c.name === 'ping')).toHaveLength(1);
    expect(getCommand('definitely-not-registered-xyz')).toBeUndefined();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd packages/brain-shell && bun test src/test/contracts/commandRegistry.test.ts 2>&1 | tail -20`
Expected: FAIL — `getCommand(...)` returns undefined / `run is not a function` (old async-handler contract).

- [ ] **Step 3: Rewrite `registry.ts`**

Replace the entire content of `packages/brain-shell/src/commands/registry.ts`:

```ts
/**
 * Brain-owned slash-command catalog — the single source of truth for the
 * palette, /help output, and command execution. Commands are pure data +
 * a sync `run` returning a declarative result; the shell interprets results
 * into state (Inc 21). Built-ins self-register from ./builtin.js.
 */

export type CommandAction = 'clear' | 'quit' | 'resume' | 'theme';
export type CommandOverlay = 'doctor' | 'memory';

export type CommandResult =
  | { type: 'text'; value: string }
  | { type: 'none' }
  | { type: 'action'; action: CommandAction }
  | { type: 'overlay'; overlay: CommandOverlay };

export interface CommandContext {
  args: string[];
  sessionId?: string;
}

export interface Command {
  /** Name without the leading '/'. Lowercase `[a-z0-9_-]+`. */
  name: string;
  /** One-line description shown in the palette and /help output. */
  description: string;
  aliases?: string[];
  argumentHint?: string;
  hidden?: boolean;
  run(ctx: CommandContext): CommandResult;
}

const registry = new Map<string, Command>();

export function registerCommand(cmd: Command): void {
  registry.set(cmd.name, cmd);
  for (const alias of cmd.aliases ?? []) registry.set(alias, cmd);
}

export function getCommands(): Command[] {
  return [...new Set(registry.values())].sort((a, b) => a.name.localeCompare(b.name));
}

export function getCommand(name: string): Command | undefined {
  return registry.get(name);
}
```

- [ ] **Step 4: Create `builtin.ts` (six commands)**

Create `packages/brain-shell/src/commands/builtin.ts`:

```ts
/** Built-in slash commands (Inc 21). Later increments extend this list. */

import { registerCommand, getCommands, type Command } from './registry.js';
import { runPermissionsCommand } from '../state/permissionRules.js';

const BUILTINS: Command[] = [
  {
    name: 'help',
    description: 'List available slash commands',
    run: () => ({
      type: 'text',
      value: [
        'Slash commands:',
        ...getCommands()
          .filter((c) => !c.hidden)
          .map((c) => `/${c.name} — ${c.description}`),
      ].join('\n'),
    }),
  },
  { name: 'clear', description: 'Clear the transcript', run: () => ({ type: 'action', action: 'clear' }) },
  { name: 'resume', description: 'Resume a previous session', run: () => ({ type: 'action', action: 'resume' }) },
  { name: 'theme', description: 'Change the color theme', run: () => ({ type: 'action', action: 'theme' }) },
  {
    name: 'permissions',
    description: 'List or remove always-allow rules',
    run: (ctx) => ({ type: 'text', value: runPermissionsCommand(ctx.args) }),
  },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'], run: () => ({ type: 'action', action: 'quit' }) },
];

for (const cmd of BUILTINS) registerCommand(cmd);
```

- [ ] **Step 5: Run to verify pass**

Run: `cd packages/brain-shell && bun test src/test/contracts/commandRegistry.test.ts 2>&1 | tail -8`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/brain-shell/src/commands/registry.ts \
  packages/brain-shell/src/commands/builtin.ts \
  packages/brain-shell/src/test/contracts/commandRegistry.test.ts
git commit -m "feat(shell): canonical command registry + six-command builtin catalog

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Matcher retirement + AppShell result interpreter

**Files:**
- Modify: `packages/brain-shell/src/commands/matcher.ts`
- Modify: `packages/brain-shell/src/ui/composer/PromptInput.tsx:5-6, :86`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx:15-16, :145-192`
- Test (rewrite): `packages/brain-shell/src/test/commands/matcher.test.ts`

**Interfaces:**
- Consumes: Task 1 exports. `fuzzyMatchCommands(query, commands)` loses its default parameter.
- Produces: `parseCommandQuery(value: string): string | null` (unchanged); `fuzzyMatchCommands(query: string, commands: readonly BrainCommand[]): CommandMatch[]` (required second arg). `BrainCommand`/`CommandMatch` unchanged. AppShell interprets all four `CommandResult` variants; `overlay` variant currently unreachable (no emitters until Tasks 4–5) but fully implemented.

- [ ] **Step 1: Rewrite the failing matcher test**

Replace `packages/brain-shell/src/test/commands/matcher.test.ts` entirely:

```ts
import { describe, expect, test } from 'bun:test';
import {
  parseCommandQuery,
  fuzzyMatchCommands,
  type BrainCommand,
} from '../../commands/matcher.js';
import { getCommands } from '../../commands/registry.js';
import '../commands/builtin.js'; // side-effect registration

const FIXTURE: readonly BrainCommand[] = [
  { name: 'help', description: 'List available slash commands' },
  { name: 'clear', description: 'Clear the transcript' },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
];

describe('parseCommandQuery', () => {
  test('open iff whole buffer is a bare slash token', () => {
    expect(parseCommandQuery('/')).toBe('');
    expect(parseCommandQuery('/c')).toBe('c');
    expect(parseCommandQuery('/clear')).toBe('clear');
    expect(parseCommandQuery('/clear now')).toBeNull(); // args started
    expect(parseCommandQuery('x/y')).toBeNull();
    expect(parseCommandQuery('clear')).toBeNull();
  });
});

describe('fuzzyMatchCommands', () => {
  test('exact name > alias exact > prefix > subsequence > description', () => {
    const hits = fuzzyMatchCommands('q', FIXTURE);
    expect(hits[0]!.command.name).toBe('quit'); // alias exact 85 beats subsequence
    const pre = fuzzyMatchCommands('cl', FIXTURE);
    expect(pre[0]!.command.name).toBe('clear');
    const desc = fuzzyMatchCommands('xq', [
      { name: 'omega', description: 'List available slash commands' },
    ]);
    expect(desc).toHaveLength(0); // 'xq' misses the name AND every description word
  });

  test('empty query lists everything at tier 10, ties break by name', () => {
    const hits = fuzzyMatchCommands('', FIXTURE);
    expect(hits.map((h) => h.command.name)).toEqual(['clear', 'help', 'quit']);
  });

  test('no matches yields empty array', () => {
    expect(fuzzyMatchCommands('zzzz', FIXTURE)).toHaveLength(0);
  });
});

describe('palette over the canonical registry', () => {
  test('registry catalog satisfies the palette contract', () => {
    const hits = fuzzyMatchCommands('', getCommands());
    expect(hits.length).toBeGreaterThanOrEqual(6);
    expect(hits.map((h) => h.command.name)).toContain('help');
    const narrow = fuzzyMatchCommands('res', getCommands());
    expect(narrow[0]!.command.name).toBe('resume');
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd packages/brain-shell && bun test src/test/commands/matcher.test.ts 2>&1 | tail -12`
Expected: FAIL — old signature has optional second param and COMMANDS still exists; registry import path works but catalog lacks integration expectations.

- [ ] **Step 3: Strip `matcher.ts` to pure functions**

Rewrite `packages/brain-shell/src/commands/matcher.ts`, keeping the interfaces and both functions but deleting `COMMANDS` and the default parameter:

```ts
/**
 * Slash-palette parsing + scoring over an injected command catalog.
 * Pure data + functions: no I/O, no React. The canonical catalog lives in
 * ./registry.ts and self-registers from ./builtin.js (Inc 21 retired the
 * duplicate static list that used to live here).
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
 * of list order.
 */
export function fuzzyMatchCommands(
  query: string,
  commands: readonly BrainCommand[],
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

(`Command` from registry structurally satisfies `BrainCommand`, so passing `getCommands()` typechecks.)

- [ ] **Step 4: Update PromptInput call site**

In `packages/brain-shell/src/ui/composer/PromptInput.tsx` line 86, change:

```ts
      ? fuzzyMatchCommands(query)
```
to:
```ts
      ? fuzzyMatchCommands(query, getCommands())
```
and extend line 5's import group by adding below it:

```ts
import { getCommands } from '../../commands/registry.js';
```

- [ ] **Step 5: Rewrite AppShell.runCommand as the result interpreter**

In `packages/brain-shell/src/ui/shell/AppShell.tsx`:

(a) Replace line 15 (`import { COMMANDS } from '../../commands/matcher.js';`) with:

```ts
import { getCommand, getCommands } from '../../commands/registry.js';
import '../commands/builtin.js';
```

(b) Delete the `helpText` arrow function (lines ~145-146).

(c) Replace the body of `runCommand` (lines ~148-192) with:

```ts
  const runCommand = (rawValue: string): void => {
    const words = rawValue.trim().slice(1).split(/\s+/); // strip '/', split args
    const token = (words[0] ?? '').toLowerCase();
    const args = words.slice(1);
    if (token.length === 0) return;
    let chosen = getCommand(token);
    if (chosen === undefined) {
      const prefixHits = getCommands().filter((c) => c.name.startsWith(token));
      if (prefixHits.length === 1) chosen = prefixHits[0];
      else if (prefixHits.length > 1) {
        controller.notice(`Ambiguous command: /${token}`);
        return;
      } else {
        controller.notice(`Unknown command: /${token}`);
        return;
      }
    }
    const res = chosen.run({ args, sessionId: controller.activeSessionId });
    switch (res.type) {
      case 'text':
        controller.notice(res.value);
        break;
      case 'none':
        break;
      case 'action':
        if (res.action === 'quit') process.exit(0);
        else if (res.action === 'clear') controller.clear();
        else if (res.action === 'theme') {
          setThemeOriginal(themeSetting);
          setThemeSelected(Math.max(0, THEME_CHOICES.findIndex((c) => c.setting === themeSetting)));
          setThemeOpen(true);
        } else if (res.action === 'resume') {
          if (snapshot.busy) {
            controller.notice('Busy — wait for the current turn to finish.');
            return;
          }
          void controller.listSessions().then((all) => {
            if (resumeChoices(all, Date.now()).length === 0) {
              controller.notice('No previous sessions found.');
              return;
            }
            setResumeSummaries(all);
            setResumeQuery('');
            setResumeSelected(0);
            setResumeOpen(true);
          });
        }
        break;
      case 'overlay':
        if (res.overlay === 'doctor') setDoctorOpen(true);
        else setMemoryOpen(true);
        break;
    }
  };
```

(d) Add placeholder-free minimal state so the file compiles before Tasks 4–5 land their views — declare next to `resumeOpen` (~line 54):

```ts
  const [doctorOpen, setDoctorOpen] = React.useState(false);
  const [memoryOpen, setMemoryOpen] = React.useState(false);
```

and render nothing yet (the flags are consumed in Tasks 4–5; between commits they are written-but-unread state, which TypeScript does not flag for useState pairs).

- [ ] **Step 6: Run shell suite subset**

Run: `cd packages/brain-shell && bun test src/test/commands/matcher.test.ts src/test/contracts/commandRegistry.test.ts src/test/contracts/shell.test.tsx 2>&1 | tail -25`
Expected: matcher + registry PASS. If `shell.test.tsx` asserts `/help` output ORDER (old array order vs new alphabetical), update ONLY the affected assertions in it — CHECK FOR FOREIGN HUNKS FIRST per Global Constraints (this file is user-WIP-dirty; use the HEAD-blob rebuild recipe if foreign hunks coexist).

Also run the full composer/contracts neighborhood: `bun test src/test/ui 2>&1 | tail -6` — expect no new identities beyond the documented five.

- [ ] **Step 7: Commit**

```bash
git add packages/brain-shell/src/commands/matcher.ts \
  packages/brain-shell/src/ui/composer/PromptInput.tsx \
  packages/brain-shell/src/ui/shell/AppShell.tsx \
  packages/brain-shell/src/test/commands/matcher.test.ts
# plus shell.test.tsx ONLY if step 6 required assertion updates and the
# staged diff was verified to contain zero foreign lines
git commit -m "feat(shell): retire static command list — registry-driven palette and interpreter

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: ModalFrame primitive

**Files:**
- Create: `packages/brain-shell/src/ui/overlays/ModalFrame.tsx`
- Test: `packages/brain-shell/src/test/ui/overlays/modalFrame.test.tsx`

**Interfaces:**
- Consumes: `Box, Text` from `../../compat/index.js`; `useTerminalSize` from `../../compat/hooks.js`.
- Produces: `ModalFrame(props: {title: string; subtitle?: string; footerHints?: string; width: number; children: React.ReactNode}): React.ReactElement` — consumed by Tasks 4–5.

- [ ] **Step 1: Write the failing view test**

Create `packages/brain-shell/src/test/ui/overlays/modalFrame.test.tsx`:

```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { ModalFrame } from '../../../ui/overlays/ModalFrame.js';

function textOf(el: React.ReactElement): string {
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    if (el2.props && typeof el2.props === 'object' && 'children' in el2.props) {
      return walk((el2.props as { children?: React.ReactNode }).children);
    }
    return '';
  };
  return walk(el);
}

describe('ModalFrame (Inc 21)', () => {
  test('renders title, subtitle, children, footer hints', () => {
    const out = textOf(
      ModalFrame({
        title: 'Brain System Doctor',
        subtitle: 'Subsystem health probes',
        footerHints: 'Enter / Esc to dismiss',
        width: 80,
        children: React.createElement('ink-box', null, 'BODY CONTENT'),
      }),
    );
    expect(out).toContain('Brain System Doctor');
    expect(out).toContain('Subsystem health probes');
    expect(out).toContain('BODY CONTENT');
    expect(out).toContain('Enter / Esc to dismiss');
  });

  test('omits subtitle/footer when absent', () => {
    const out = textOf(
      ModalFrame({ title: 'Only Title', width: 40, children: null }),
    );
    expect(out).toContain('Only Title');
    expect(out).not.toContain('dismiss');
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/modalFrame.test.tsx 2>&1 | tail -6`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement ModalFrame**

Create `packages/brain-shell/src/ui/overlays/ModalFrame.tsx`:

```tsx
/** Shared bordered frame for command overlays (/doctor, /memory). Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import { useTerminalSize } from '../../compat/hooks.js';

export function ModalFrame(props: {
  title: string;
  subtitle?: string;
  footerHints?: string;
  width: number;
  children: React.ReactNode;
}): React.ReactElement {
  void useTerminalSize; // width cap happens at the AppShell mount site
  return (
    <Box flexDirection="column" borderStyle="round" width={props.width} paddingX={1}>
      <Text bold>{props.title}</Text>
      {props.subtitle ? <Text dimColor>{props.subtitle}</Text> : null}
      {props.children}
      {props.footerHints ? <Text dimColor>{props.footerHints}</Text> : null}
    </Box>
  );
}
```

(The `void useTerminalSize` line is deliberate: the import is NOT actually needed since callers cap width against columns themselves — delete BOTH the import and the void line rather than keeping dead references.)

Final content therefore:

```tsx
/** Shared bordered frame for command overlays (/doctor, /memory). Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';

export function ModalFrame(props: {
  title: string;
  subtitle?: string;
  footerHints?: string;
  width: number;
  children: React.ReactNode;
}): React.ReactElement {
  return (
    <Box flexDirection="column" borderStyle="round" width={props.width} paddingX={1}>
      <Text bold>{props.title}</Text>
      {props.subtitle ? <Text dimColor>{props.subtitle}</Text> : null}
      {props.children}
      {props.footerHints ? <Text dimColor>{props.footerHints}</Text> : null}
    </Box>
  );
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/modalFrame.test.tsx 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/brain-shell/src/ui/overlays/ModalFrame.tsx \
  packages/brain-shell/src/test/ui/overlays/modalFrame.test.tsx
git commit -m "feat(shell): shared ModalFrame overlay primitive

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: /doctor — view, catalog entry, AppShell wiring

**Files:**
- Create: `packages/brain-shell/src/ui/overlays/DoctorOverlayView.tsx`
- Modify: `packages/brain-shell/src/commands/builtin.ts` (append doctor entry)
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (probe memo, fetch effect, input block, render, remove placeholder state pair from T2 step 5d)
- Delete: `packages/brain-shell/src/commands/doctor/DoctorCommand.tsx`
- Test: `packages/brain-shell/src/test/ui/overlays/doctorOverlayView.test.tsx`

**Interfaces:**
- Consumes: Task 1 `registerCommand`; Task 3 `ModalFrame`; existing `DoctorProbe`/`EngineDiagnosticReport` from `../../adapter/doctorProbe.js` (UNMODIFIED); resolver actions `overlay:cancel`/`overlay:commit` (enter maps to commit via existing binding table).
- Produces: `DoctorOverlayView(props: {loading: boolean; report: EngineDiagnosticReport | null; tokens: BrainTokens}): React.ReactElement` (PURE — no hooks, plain-call testable). AppShell owns `doctorOpen`, `doctorLoading`, `doctorReport` state and the `overlay:doctor` case now becomes reachable.

- [ ] **Step 1: Write the failing view test**

Create `packages/brain-shell/src/test/ui/overlays/doctorOverlayView.test.tsx` (reuse the same `textOf` walker as `modalFrame.test.tsx`):

```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { DoctorOverlayView } from '../../../ui/overlays/DoctorOverlayView.js';
import type { EngineDiagnosticReport } from '../../../adapter/doctorProbe.js';

function textOf(el: React.ReactElement): string {
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    if (el2.props && typeof el2.props === 'object' && 'children' in el2.props) {
      return walk((el2.props as { children?: React.ReactNode }).children);
    }
    return '';
  };
  return walk(el);
}

const tokens = PALETTES.dark;

const HEALTHY: EngineDiagnosticReport = {
  timestamp: '2026-08-26T00:00:00Z',
  overallHealthy: true,
  socketPath: '/tmp/x.sock',
  subsystems: [
    { subsystem: 'UDS Daemon Socket', status: 'healthy', latencyMs: 3, message: 'responding' },
    { subsystem: 'SQLite WAL Storage', status: 'healthy', message: 'initialized' },
  ],
};

const DEGRADED: EngineDiagnosticReport = {
  timestamp: '2026-08-26T00:00:00Z',
  overallHealthy: false,
  socketPath: '/tmp/x.sock',
  subsystems: [
    { subsystem: 'UDS Daemon Socket', status: 'unhealthy', message: 'timed out' },
  ],
};

describe('DoctorOverlayView (Inc 21)', () => {
  test('healthy banner, subsystem rows, latency, remediation-none', () => {
    const out = textOf(DoctorOverlayView({ loading: false, report: HEALTHY, tokens }));
    expect(out).toContain('HEALTHY');
    expect(out).toContain('UDS Daemon Socket');
    expect(out).toContain('(3ms)');
    expect(out).toContain('No remediation required');
  });

  test('degraded banner, ✖ row, start-daemon hint', () => {
    const out = textOf(DoctorOverlayView({ loading: false, report: DEGRADED, tokens }));
    expect(out).toContain('DEGRADED');
    expect(out).toContain('✖');
    expect(out).toContain('Daemon unreachable');
  });

  test('loading and failure states', () => {
    expect(textOf(DoctorOverlayView({ loading: true, report: null, tokens }))).toContain(
      'Running diagnostic health probes',
    );
    expect(textOf(DoctorOverlayView({ loading: false, report: null, tokens }))).toContain(
      'Failed to collect diagnostic signals',
    );
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/doctorOverlayView.test.tsx 2>&1 | tail -6`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement DoctorOverlayView**

Create `packages/brain-shell/src/ui/overlays/DoctorOverlayView.tsx`:

```tsx
/** /doctor overlay body: local probe results as a read-only report. Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { EngineDiagnosticReport } from '../../adapter/doctorProbe.js';
import { ModalFrame } from './ModalFrame.js';

export function DoctorOverlayView(props: {
  loading: boolean;
  report: EngineDiagnosticReport | null;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens; // reserved: rows gain semantic colors in later increments
  return (
    <ModalFrame
      title="Brain System Doctor"
      subtitle="Subsystem health probes, IPC socket latency, and SQLite storage verification"
      footerHints="Enter / Esc to dismiss"
      width={80}
    >
      {props.loading ? (
        <Text color="yellow">Running diagnostic health probes…</Text>
      ) : props.report === null ? (
        <Text color="red">Failed to collect diagnostic signals.</Text>
      ) : (
        <Box flexDirection="column" gap={1}>
          <Box flexDirection="row">
            <Text bold>Overall System Health: </Text>
            <Text color={props.report.overallHealthy ? 'green' : 'red'} bold>
              {props.report.overallHealthy ? '● HEALTHY' : '▲ DEGRADED / UNHEALTHY'}
            </Text>
          </Box>
          <Box flexDirection="column">
            <Text bold color="cyan">Observable Subsystem Probes:</Text>
            {props.report.subsystems.map((sub, idx) => (
              <Box key={idx} flexDirection="column">
                <Text>
                  <Text color={sub.status === 'healthy' ? 'green' : 'red'}>
                    {sub.status === 'healthy' ? '✔' : '✖'}
                  </Text>
                  {' '}{sub.subsystem}
                  {sub.latencyMs !== undefined ? ` (${sub.latencyMs}ms)` : ''}
                </Text>
                <Text dimColor>{'⎿ '}{sub.message}</Text>
              </Box>
            ))}
          </Box>
          <Box flexDirection="column">
            <Text bold color="cyan">Remediation Actions:</Text>
            {props.report.overallHealthy ? (
              <Text dimColor>{'⎿ '}No remediation required. All local subsystems operational.</Text>
            ) : (
              <Text color="yellow">⎿ Daemon unreachable. Run `brain daemon start` or `make dev` to start the service.</Text>
            )}
          </Box>
        </Box>
      )}
    </ModalFrame>
  );
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/doctorOverlayView.test.tsx 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Register the catalog entry**

Append to the `BUILTINS` array in `packages/brain-shell/src/commands/builtin.ts` (after the quit entry):

```ts
  { name: 'doctor', description: 'Run system diagnostics', run: () => ({ type: 'overlay', overlay: 'doctor' }) },
```

Extend `commandRegistry.test.ts`: add `'doctor'` handling —

```ts
  test('doctor opens the diagnostics overlay', () => {
    expect(getCommand('doctor')!.run({ args: [] })).toEqual({ type: 'overlay', overlay: 'doctor' });
  });
```

Run: `cd packages/brain-shell && bun test src/test/contracts/commandRegistry.test.ts 2>&1 | tail -4` → PASS.

- [ ] **Step 6: Wire AppShell (probe lifecycle + input + render)**

In `packages/brain-shell/src/ui/shell/AppShell.tsx`:

(a) Imports — add:

```ts
import { DoctorProbe, type EngineDiagnosticReport } from '../../adapter/doctorProbe.js';
import { DoctorOverlayView } from '../overlays/DoctorOverlayView.js';
```

(b) State — REPLACE the two placeholder lines from Task 2 step 5(d) with full doctor state (keep `memoryOpen` placeholder until Task 5):

```ts
  const [doctorOpen, setDoctorOpen] = React.useState(false);
  const [doctorLoading, setDoctorLoading] = React.useState(false);
  const [doctorReport, setDoctorReport] = React.useState<EngineDiagnosticReport | null>(null);
  const [memoryOpen, setMemoryOpen] = React.useState(false);
  const doctorProbe = React.useMemo(() => new DoctorProbe(), []);
```

(c) Fetch effect (probe runs once per open; place near the other overlay effects):

```ts
  React.useEffect(() => {
    if (!doctorOpen) return;
    let alive = true;
    setDoctorLoading(true);
    setDoctorReport(null);
    void doctorProbe
      .runDiagnostics()
      .then((rep) => {
        if (alive) {
          setDoctorReport(rep);
          setDoctorLoading(false);
        }
      })
      .catch(() => {
        if (alive) setDoctorLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [doctorOpen, doctorProbe]);
```

(d) Input block (mirrors the theme-picker grammar; dismissal always emits the system notice):

```ts
  // /doctor overlay: read-only report; enter or esc dismisses.
  useBoundInput({
    contexts: ['overlay'],
    isActive: doctorOpen,
    onAction: (action) => {
      const d = overlayListDecision(action, 0, 0);
      if (d.type === 'cancel' || d.type === 'commit') {
        setDoctorOpen(false);
        controller.notice('Completed system diagnostics');
      }
    },
  });
```

(e) Pause the composer while the overlay is open (house invariant — an open overlay means the composer must not consume keystrokes). In the JSX, change:

```tsx
          paused={themeOpen || resumeOpen || permission !== undefined}
```
to:
```tsx
          paused={themeOpen || resumeOpen || permission !== undefined || doctorOpen}
```

(f) Render — insert directly AFTER the permission-dialog block (`{permission ? (...) : null}`) and BEFORE the composer `<Box marginTop={1}><PromptInput …`:

```tsx
      {doctorOpen ? (
        <Box marginTop={1}>
          <DoctorOverlayView loading={doctorLoading} report={doctorReport} tokens={tokens} />
        </Box>
      ) : null}
```

(g) In the `case 'overlay':` arm of `runCommand`, the `setDoctorOpen(true)` line from Task 2 now drives real UI (no edit needed — confirm it compiles).

- [ ] **Step 7: Delete the superseded component + run suites**

```bash
git rm packages/brain-shell/src/commands/doctor/DoctorCommand.tsx
cd packages/brain-shell && bun test src/test/ui src/test/contracts 2>&1 | tail -8
```
Expected: no NEW failure identities (grep-zero confirms nothing imported the deleted file: `git grep -n "DoctorCommand" 3dc8db73 -- packages/brain-shell/src` showed only the file itself).

- [ ] **Step 8: Commit**

```bash
git add packages/brain-shell/src/ui/overlays/DoctorOverlayView.tsx \
  packages/brain-shell/src/commands/builtin.ts \
  packages/brain-shell/src/ui/shell/AppShell.tsx \
  packages/brain-shell/src/test/ui/overlays/doctorOverlayView.test.tsx \
  packages/brain-shell/src/test/contracts/commandRegistry.test.ts
git commit -m "feat(shell): wire /doctor through the canonical registry onto ModalFrame

Co-Authored-By: Claude <noreply@anthropic.com>"
```

(`git rm` already staged the deletion; the explicit add covers the rest. Verify `git status --porcelain` shows NO unrelated staged paths before committing.)

---

### Task 5: /memory — logic helpers, client fix, controller wrapper, view, wiring

**Files:**
- Modify: `packages/brain-shell/src/client/BrainBackendClient.ts` (add `MemorySearchResult` below `RetrievedMemory`)
- Create: `packages/brain-shell/src/ui/overlays/memoryOverlayLogic.ts`
- Modify: `packages/brain-shell/src/client/UdsBrainBackendClient.ts:~820-830` (one mapping line)
- Modify: `packages/brain-shell/src/state/sessionController.ts` (add `searchMemories`)
- Create: `packages/brain-shell/src/ui/overlays/MemoryOverlayView.tsx`
- Modify: `packages/brain-shell/src/commands/builtin.ts` (append memory entry)
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (state, debounce effect, input block, render, drop `memoryOpen` placeholder duplication)
- Delete: `packages/brain-shell/src/commands/memory/MemoryCommand.tsx`
- Test: `packages/brain-shell/src/test/ui/overlays/memoryOverlayLogic.test.ts`, `.../memoryOverlayView.test.tsx`, `packages/brain-shell/src/test/client/memorySearchWire.test.ts`

**Interfaces:**
- Consumes: `applyQueryEdit(query, action, input)` from `./resumePickerLogic.js` (B5, reused verbatim); `overlayListDecision`; `RetrievedMemory`/`MemoryRelation` from client contracts; client `searchMemory(input)` (existing).
- Produces:
  - `type MemorySearchResult = { ok: true; memories: RetrievedMemory[] } | { ok: false }` — declared in `client/BrainBackendClient.ts` beside `RetrievedMemory`; imported by the controller. (Layering rule: `state/*` may import client contracts, never ui modules.)
  - `scorePercent(score: number): number` — `Math.max(0, Math.min(100, Math.round(score)))`; `clampSelection(selected: number, count: number): number` — from memoryOverlayLogic.
  - `SessionController.searchMemories(query: string, limit?: number): Promise<MemorySearchResult>`.
  - `MemoryOverlayView(props: {query: string; state: 'loading'|'offline'|'ready'; rows: readonly RetrievedMemory[]; selectedIndex: number; expandedId: string | null; tokens: BrainTokens})` — PURE.
  - Client `searchMemory` output memories carry `relations` (default `[]`).

- [ ] **Step 1: Write the failing logic + wire tests**

Create `packages/brain-shell/src/test/ui/overlays/memoryOverlayLogic.test.ts`:

```ts
import { describe, expect, test } from 'bun:test';
import { scorePercent, clampSelection } from '../../../ui/overlays/memoryOverlayLogic.js';

describe('memoryOverlayLogic (Inc 21)', () => {
  test('score clamps to 0..100 and rounds', () => {
    expect(scorePercent(99.4)).toBe(99);
    expect(scorePercent(-5)).toBe(0);
    expect(scorePercent(250)).toBe(100);
  });

  test('selection clamps into range, empty-safe', () => {
    expect(clampSelection(7, 3)).toBe(2);
    expect(clampSelection(1, 0)).toBe(0);
    expect(clampSelection(0, 5)).toBe(0);
  });
});
```

Create `packages/brain-shell/src/test/client/memorySearchWire.test.ts` — same scripted-daemon dialect as `toolResultWire.test.ts` (top-level `server.listen`, object-valued response `body`, explicit constructor arg):

```ts
import { describe, expect, test, afterAll } from 'bun:test';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-memory-wire-'));
const sockPath = path.join(dir, 't.sock');

// Scripted daemon: memory/search replies with one memory whose DTO carries
// relations — proving the client mapping preserves them.
const server = net.createServer((socket) => {
  let buffer = '';
  socket.on('data', (data) => {
    buffer += data.toString('utf8');
    let idx: number;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (!line.trim()) continue;
      const req = JSON.parse(line) as { action?: string };
      const reply = (obj: unknown) => socket.write(JSON.stringify(obj) + '\n');
      if (req.action === 'memory/search') {
        reply({
          type: 'Response',
          status: 'success',
          body: {
            memories: [
              {
                node_id: 'n1',
                label: 'Alpha Cortex Node',
                excerpt: 'Cortex excerpt body',
                channel: 'knowledge_graph',
                score: 97,
                timestamp: 1756160000000,
                scope: 'workspace',
                relations: [
                  { target_id: 'b1', relation: 'supports', target_label: 'Beta Concept' },
                ],
              },
            ],
          },
        });
      } else {
        reply({ type: 'Response', status: 'success', body: {} });
      }
    }
  });
});
server.listen(sockPath);
afterAll(() => {
  server.close();
});

describe('searchMemory wire mapping (Inc 21)', () => {
  test('preserves relations from the daemon DTO', async () => {
    const client = new UdsBrainBackendClient(sockPath);
    const res = await client.searchMemory({ query: 'cortex', limit: 10 });
    expect(res.memories).toHaveLength(1);
    expect(res.memories[0]!.label).toBe('Alpha Cortex Node');
    expect(res.memories[0]!.score).toBe(97);
    expect(res.memories[0]!.relations).toEqual([
      { target_id: 'b1', relation: 'supports', target_label: 'Beta Concept' },
    ]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/memoryOverlayLogic.test.ts src/test/client/memorySearchWire.test.ts 2>&1 | tail -8`
Expected: FAIL — memoryOverlayLogic missing; wire test asserts `relations` equals seeded array but mapping drops the field (actual `undefined`).

- [ ] **Step 3: Implement logic helpers + client fix**

(a) In `packages/brain-shell/src/client/BrainBackendClient.ts`, immediately below the `RetrievedMemory` interface's closing brace (line ~141), add:

```ts
/** Liveness-discriminated search result: `ok:false` means the daemon is
 * unreachable — callers render offline copy instead of empty-copy. */
export type MemorySearchResult =
  | { ok: true; memories: RetrievedMemory[] }
  | { ok: false };
```

(b) Create `packages/brain-shell/src/ui/overlays/memoryOverlayLogic.ts`:

```ts
/** /memory overlay shared bits: score display and selection clamping.
 * The liveness result type lives in the client contract (MemorySearchResult). */

export function scorePercent(score: number): number {
  return Math.max(0, Math.min(100, Math.round(score)));
}

export function clampSelection(selected: number, count: number): number {
  if (count === 0) return 0;
  return Math.min(Math.max(0, selected), count - 1);
}
```

(c) In `packages/brain-shell/src/client/UdsBrainBackendClient.ts`, inside `searchMemory`'s map callback (~line 820-830), add one property after the `scope:` line of the returned object:

```ts
        relations: m.relations ?? [],
```

- [ ] **Step 4: Run logic + wire tests to green**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/memoryOverlayLogic.test.ts src/test/client/memorySearchWire.test.ts 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Write the failing view test**

Create `packages/brain-shell/src/test/ui/overlays/memoryOverlayView.test.tsx` (`textOf` walker identical to prior view tests):

```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { MemoryOverlayView } from '../../../ui/overlays/MemoryOverlayView.js';
import type { RetrievedMemory } from '../../../client/BrainBackendClient.js';

function textOf(el: React.ReactElement): string { /* same walker as modalFrame.test.tsx */ 
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    if (el2.props && typeof el2.props === 'object' && 'children' in el2.props) {
      return walk((el2.props as { children?: React.ReactNode }).children);
    }
    return '';
  };
  return walk(el);
}

const tokens = PALETTES.dark;

const row = (label: string, relations?: RetrievedMemory['relations']): RetrievedMemory => ({
  node_id: label.toLowerCase(),
  label,
  excerpt: `${label} excerpt`,
  channel: 'knowledge_graph',
  score: 90,
  timestamp: 0,
  scope: 'workspace',
  ...(relations ? { relations } : {}),
});

describe('MemoryOverlayView (Inc 21)', () => {
  test('ready state lists labels with scores and channels', () => {
    const out = textOf(MemoryOverlayView({
      query: '', state: 'ready',
      rows: [row('Alpha Cortex Node'), row('Beta Ledger')],
      selectedIndex: 0, expandedId: null, tokens,
    }));
    expect(out).toContain('Alpha Cortex Node');
    expect(out).toContain('90%');
    expect(out).toContain('[knowledge_graph]');
  });

  test('query line renders the live filter text', () => {
    const out = textOf(MemoryOverlayView({
      query: 'crtx', state: 'ready',
      rows: [row('Alpha Cortex Node')],
      selectedIndex: 0, expandedId: null, tokens,
    }));
    expect(out).toContain('› crtx');
  });

  test('expanded row shows excerpt and relations', () => {
    const out = textOf(MemoryOverlayView({
      query: '', state: 'ready',
      rows: [row('Alpha Cortex Node', [
        { target_id: 'b1', relation: 'supports', target_label: 'Beta Concept' },
      ])],
      selectedIndex: 0, expandedId: 'alpha cortex node', tokens,
    }));
    expect(out).toContain('Connected Relations:');
    expect(out).toContain('supports');
    expect(out).toContain('Beta Concept');
  });

  test('expanded row without relations shows the none-line', () => {
    const out = textOf(MemoryOverlayView({
      query: '', state: 'ready',
      rows: [row('Solo Node')],
      selectedIndex: 0, expandedId: 'solo node', tokens,
    }));
    expect(out).toContain('(No outgoing relations)');
  });

  test('offline, loading, and empty states', () => {
    expect(textOf(MemoryOverlayView({ query: '', state: 'offline', rows: [], selectedIndex: 0, expandedId: null, tokens })))
      .toContain('Brain daemon is offline or unreachable.');
    expect(textOf(MemoryOverlayView({ query: '', state: 'loading', rows: [], selectedIndex: 0, expandedId: null, tokens })))
      .toContain('Searching knowledge graph');
    expect(textOf(MemoryOverlayView({ query: '', state: 'ready', rows: [], selectedIndex: 0, expandedId: null, tokens })))
      .toContain('No concepts recorded in the Brain knowledge graph yet.');
  });
});
```

- [ ] **Step 6: Run to verify failure**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/memoryOverlayView.test.tsx 2>&1 | tail -5`
Expected: FAIL — module not found.

- [ ] **Step 7: Implement MemoryOverlayView**

Create `packages/brain-shell/src/ui/overlays/MemoryOverlayView.tsx`:

```tsx
/** /memory overlay body: searchable knowledge-graph browser. Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { RetrievedMemory } from '../../client/BrainBackendClient.js';
import { ModalFrame } from './ModalFrame.js';
import { scorePercent } from './memoryOverlayLogic.js';

export function MemoryOverlayView(props: {
  query: string;
  state: 'loading' | 'offline' | 'ready';
  rows: readonly RetrievedMemory[];
  selectedIndex: number;
  expandedId: string | null;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens; // reserved: selection colors arrive with semantic tokens
  return (
    <ModalFrame
      title="Relational Knowledge & Memory"
      subtitle="Inspect concepts, confidence scores, excerpts, and graph relations"
      footerHints="↑↓ navigate · enter expand · type to filter · esc close"
      width={80}
    >
      <Text>› {props.query}▏</Text>
      {props.state === 'offline' ? (
        <Box flexDirection="column">
          <Text color="red">Brain daemon is offline or unreachable.</Text>
          <Text dimColor>Start it with `brain daemon start` or `make dev`</Text>
        </Box>
      ) : props.state === 'loading' ? (
        <Text color="yellow">Searching knowledge graph…</Text>
      ) : props.rows.length === 0 ? (
        <Text dimColor>No concepts recorded in the Brain knowledge graph yet.</Text>
      ) : (
        <Box flexDirection="column">
          {props.rows.slice(0, 6).map((m, idx) => {
            const isSelected = idx === props.selectedIndex;
            const isExpanded = isSelected && props.expandedId === m.node_id;
            return (
              <Box key={m.node_id} flexDirection="column">
                <Box flexDirection="row" justifyContent="space-between">
                  <Text color={isSelected ? 'cyan' : undefined} bold={isSelected}>
                    {isSelected ? '❯ ' : '  '}{m.label}
                  </Text>
                  <Text dimColor><Text color="cyan">{scorePercent(m.score)}%</Text> · [{m.channel}]</Text>
                </Box>
                {isExpanded ? (
                  <Box marginLeft={2} flexDirection="column">
                    {m.excerpt ? <Text dimColor>{m.excerpt}</Text> : null}
                    {m.relations && m.relations.length > 0 ? (
                      <Box flexDirection="column">
                        <Text bold color="cyan">Connected Relations:</Text>
                        {m.relations.map((r, rIdx) => (
                          <Text key={rIdx} dimColor>
                            {'  ⎿ '}{r.relation} → {r.target_label ?? r.target_id}
                          </Text>
                        ))}
                      </Box>
                    ) : (
                      <Text dimColor>(No outgoing relations)</Text>
                    )}
                  </Box>
                ) : null}
              </Box>
            );
          })}
        </Box>
      )}
    </ModalFrame>
  );
}
```

- [ ] **Step 8: Run to verify pass**

Run: `cd packages/brain-shell && bun test src/test/ui/overlays/memoryOverlayView.test.tsx 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 9: Controller wrapper + catalog entry**

(a) In `packages/brain-shell/src/state/sessionController.ts`, add near `listSessions` (inside the class; `client` is the constructor-injected backend), plus one import in the file's existing import block from `'../client/BrainBackendClient.js'`:

```ts
import type { MemorySearchResult } from '../client/BrainBackendClient.js';
```

```ts
  /** /memory data source. Liveness-discriminated so views can render
   * offline copy vs empty copy; every transport failure collapses to ok:false. */
  async searchMemories(query: string, limit = 20): Promise<MemorySearchResult> {
    try {
      const res = await this.client.searchMemory({ query, limit });
      return { ok: true, memories: res.memories };
    } catch {
      return { ok: false };
    }
  }
```

(b) Append to `BUILTINS` in `commands/builtin.ts`:

```ts
  { name: 'memory', description: 'Browse the knowledge graph', run: () => ({ type: 'overlay', overlay: 'memory' }) },
```

(c) Extend `commandRegistry.test.ts`:

```ts
  test('memory opens the knowledge browser overlay', () => {
    expect(getCommand('memory')!.run({ args: [] })).toEqual({ type: 'overlay', overlay: 'memory' });
  });
```

Run: `cd packages/brain-shell && bun test src/test/contracts/commandRegistry.test.ts 2>&1 | tail -4` → PASS.

- [ ] **Step 10: Wire AppShell (state, debounced search, input, render)**

In `packages/brain-shell/src/ui/shell/AppShell.tsx`:

(a) Imports — `applyQueryEdit` is ALREADY imported in AppShell (resume picker) — extend that existing import list with nothing; only ADD these three lines:

```ts
import { MemoryOverlayView } from '../overlays/MemoryOverlayView.js';
import { clampSelection } from '../overlays/memoryOverlayLogic.js';
import type { RetrievedMemory } from '../../client/BrainBackendClient.js';
```

(b) State — replace the bare `memoryOpen` placeholder with the full set:

```ts
  const [memoryOpen, setMemoryOpen] = React.useState(false);
  const [memoryQuery, setMemoryQuery] = React.useState('');
  const [memoryRows, setMemoryRows] = React.useState<RetrievedMemory[]>([]);
  const [memoryState, setMemoryState] = React.useState<'loading' | 'offline' | 'ready'>('loading');
  const [memorySelected, setMemorySelected] = React.useState(0);
  const [memoryExpandedId, setMemoryExpandedId] = React.useState<string | null>(null);
  const memoryToken = React.useRef(0);
```

(c) Debounced search effect — fires on open AND on each query keystroke, 200 ms, monotonic token guard:

```ts
  React.useEffect(() => {
    if (!memoryOpen) return;
    const ticket = ++memoryToken.current;
    setMemoryState('loading');
    const timer = setTimeout(() => {
      void controller.searchMemories(memoryQuery, 20).then((r) => {
        if (memoryToken.current !== ticket) return; // stale response dropped
        if (!r.ok) {
          setMemoryState('offline');
          setMemoryRows([]);
        } else {
          setMemoryState('ready');
          setMemoryRows(r.memories);
        }
        setMemorySelected((i) => clampSelection(i, r.ok ? r.memories.length : 0));
      });
    }, 200);
    return () => clearTimeout(timer);
  }, [memoryOpen, memoryQuery, controller]);
```

(d) Input block (navigation + expand + typing; esc dismisses with the system notice):

```ts
  // /memory overlay: type-to-filter over the knowledge graph.
  useBoundInput({
    contexts: ['overlay'],
    isActive: memoryOpen,
    onAction: (action, input) => {
      const d = overlayListDecision(action, memorySelected, Math.min(memoryRows.length, 6));
      if (d.type === 'move') {
        setMemoryExpandedId(null);
        setMemorySelected(d.index);
      } else if (d.type === 'commit') {
        const chosen = memoryRows[d.index];
        setMemoryExpandedId((cur) =>
          cur === chosen?.node_id ? null : chosen?.node_id ?? null,
        );
      } else if (d.type === 'cancel') {
        setMemoryOpen(false);
        controller.notice('Closed memory exploration view');
      } else if (action === 'overlay:insert') {
        setMemoryQuery((q) => applyQueryEdit(q, action, input));
      } else if (action === 'overlay:backspace') {
        setMemoryQuery((q) => applyQueryEdit(q, action, input));
      }
    },
  });
```

(e) Opening resets the session (in the `case 'overlay':` arm, extend the else branch):

```ts
      case 'overlay':
        if (res.overlay === 'doctor') setDoctorOpen(true);
        else {
          setMemoryQuery('');
          setMemoryRows([]);
          setMemorySelected(0);
          setMemoryExpandedId(null);
          setMemoryOpen(true);
        }
        break;
```

(f) Pause the composer while the overlay is open — extend the same line Task 4 touched:

```tsx
          paused={themeOpen || resumeOpen || permission !== undefined || doctorOpen || memoryOpen}
```

(g) Render — insert directly after the doctor render block from Task 4 (before the composer `<Box marginTop={1}><PromptInput …`):

```tsx
      {memoryOpen ? (
        <Box marginTop={1}>
          <MemoryOverlayView
            query={memoryQuery}
            state={memoryState}
            rows={memoryRows}
            selectedIndex={memorySelected}
            expandedId={memoryExpandedId}
            tokens={tokens}
          />
        </Box>
      ) : null}
```

- [ ] **Step 11: Delete the superseded component + run the neighborhood**

```bash
git rm packages/brain-shell/src/commands/memory/MemoryCommand.tsx
cd packages/brain-shell && bun test src/test/ui src/test/contracts src/test/client 2>&1 | tail -10
```
Expected: no NEW failure identities beyond the documented five.

- [ ] **Step 12: Commit**

```bash
git add packages/brain-shell/src/ui/overlays/memoryOverlayLogic.ts \
  packages/brain-shell/src/ui/overlays/MemoryOverlayView.tsx \
  packages/brain-shell/src/client/BrainBackendClient.ts \
  packages/brain-shell/src/client/UdsBrainBackendClient.ts \
  packages/brain-shell/src/state/sessionController.ts \
  packages/brain-shell/src/commands/builtin.ts \
  packages/brain-shell/src/ui/shell/AppShell.tsx \
  packages/brain-shell/src/test/ui/overlays/memoryOverlayLogic.test.ts \
  packages/brain-shell/src/test/ui/overlays/memoryOverlayView.test.tsx \
  packages/brain-shell/src/test/client/memorySearchWire.test.ts \
  packages/brain-shell/src/test/contracts/commandRegistry.test.ts
git commit -m "feat(shell): wire /memory knowledge-graph browser through the registry

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: PTY smoke + full gates

**Files:**
- Create: `scripts/ptySmokeInc21.py`

**Interfaces:**
- Consumes: completed Tasks 1–5; daemon binary `target/debug/brain-daemon` (already built); `v1/memory/store` RPC `{label, content, scope?, relations?}`; `v1/session/create` for a warm-up turn (not required).
- Produces: exit-0 end-to-end proof; regression evidence for `ptySmokeInc2.py`.

- [ ] **Step 1: Write the smoke**

Model `scripts/ptySmokeInc21.py` on `scripts/ptySmokeInc20.py` (same skeleton: tmp dir env vars, daemon spawn + socket wait, `rpc()`, `pty.fork` + TIOCSWINSZ(30,100), per-keystroke writes with pumps, ANSI-strip, occurrence-count waits, behavioral asserts, teardown). Full content:

```python
#!/usr/bin/env python3
"""Increment 21 PTY smoke: /doctor + /memory wired through the canonical
registry, against a REAL daemon.

Flows:
  A. Type /doctor -> diagnostics modal appears with HEALTHY banner -> enter
     dismisses with the system notice.
  B. Seed one memory via RPC (with a relation), type /memory -> modal opens,
     type "cortex" to filter -> seeded node lists, enter expands details
     naming the relation target, esc closes with the system notice.
"""
import fcntl, json, os, pty, re, select, shutil, signal, socket, struct, subprocess, sys, termios, time, uuid

ROWS, COLS = 30, 100
REPO = "/Users/ritikpathania/Developer/PyCharm/brain"
PKG_DIR = f"{REPO}/packages/brain-shell"
TMP = "/tmp/brain-inc21-smoke"
SOCK = f"{TMP}/brain.sock"
CONFIG_FILE = f"{TMP}/config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

LABEL = "Alpha Cortex Node"
CONTENT = "Cortex excerpt body for the smoke"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

shutil.rmtree(TMP, ignore_errors=True)
os.makedirs(TMP, exist_ok=True)
with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto"}, f)

env = dict(os.environ)
env.update({
    "BRAIN_SOCKET_PATH": SOCK,
    "BRAIN_PID_PATH": f"{TMP}/brain.pid",
    "BRAIN_DB_PATH": f"{TMP}/brain.db",
    "BRAIN_ANALYTICS_DB_PATH": f"{TMP}/analytics.db",
    "BRAIN_CONFIG_DIR": TMP,
    "BRAIN_HEALTH_PORT": "0",
})
daemon = subprocess.Popen(
    ["target/debug/brain-daemon", "daemon", "run"], cwd=REPO, env=env,
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
)
deadline = time.time() + 30
while time.time() < deadline:
    if os.path.exists(SOCK):
        try:
            probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            probe.connect(SOCK)
            probe.close()
            break
        except OSError:
            pass
    time.sleep(0.2)
else:
    sys.exit("FAIL: daemon never bound the socket")

def rpc(action, body, timeout=10.0):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect(SOCK)
    fobj = s.makefile("rw")
    req = {"version": "1.0", "type": "Request", "id": f"smoke-{uuid.uuid4().hex[:8]}",
           "action": action, "body": json.dumps(body)}
    fobj.write(json.dumps(req) + "\n"); fobj.flush()
    resp = json.loads(fobj.readline())
    s.close()
    raw = resp["body"]
    return json.loads(raw) if isinstance(raw, str) else raw

seeded = rpc("v1/memory/store", {
    "label": LABEL,
    "content": CONTENT,
    "scope": "workspace",
    "relations": [{"relation": "supports", "target_id": "beta-1", "target_label": "Beta Concept"}],
})
assert seeded.get("success") is not False, f"memory/store failed: {seeded}"

failures = []
def check(name, cond):
    print(("PASS " if cond else "FAIL ") + name)
    if not cond:
        failures.append(name)

pid, fd = pty.fork()
if pid == 0:
    os.chdir(PKG_DIR)
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["NODE_ENV"] = "production"
    os.execvp("bun", ["bun", "run", "src/main.tsx"])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf = bytearray()
def pump(seconds=0.4):
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                chunk = os.read(fd, 65536)
                if not chunk:
                    return
                buf.extend(chunk)
            except OSError:
                return

def send_key(ch, delay=0.35):
    os.write(fd, ch.encode())
    pump(delay)

def wait_for(needle, timeout=25.0, count=1):
    end = time.time() + timeout
    while time.time() < end:
        if clean(bytes(buf)).count(needle) >= count:
            return True
        pump(0.2)
    return False

check("boot banner", wait_for("memory-first agent workspace", timeout=40))

def run_slash(name):
    send_key("/")
    for ch in name:
        send_key(ch, 0.15)
    send_key("\r")
    pump(0.6)

# ── Flow A: /doctor ───────────────────────────────────────────────────────
run_slash("doctor")
check("A1 doctor modal opens", wait_for("Brain System Doctor"))
check("A2 healthy banner", wait_for("HEALTHY"))
check("A3 subsystem probes listed", wait_for("UDS Daemon Socket"))
send_key("\r")
check("A4 dismissed with notice", wait_for("Completed system diagnostics", count=1))

# ── Flow B: /memory ───────────────────────────────────────────────────────
# The overlay's initial empty-query fetch is deliberately not asserted:
# server-side behavior for query:'' is unspecified. Instead we type a token
# the sole seeded node contains ("cortex") — the private tmp DB guarantees
# it ranks first — and prove listing + expansion behaviorally.
run_slash("memory")
check("B1 memory modal opens", wait_for("Relational Knowledge & Memory"))
for ch in "cortex":
    send_key(ch, 0.3)   # each keystroke re-fires the 200ms debounced search
pump(0.8)
check("B2 filtered listing shows the seeded concept", wait_for(LABEL))
send_key("\r")
check("B3 expand renders the stored relation", wait_for("Beta Concept", timeout=15))
send_key("\x1b")
pump(0.4)
check("B4 dismissed with notice", wait_for("Closed memory exploration view"))

# ── Teardown ──────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
pump(0.5)
try:
    os.kill(pid, signal.SIGTERM)
except ProcessLookupError:
    pass
try:
    daemon.terminate()
    daemon.wait(timeout=10)
except subprocess.TimeoutExpired:
    daemon.kill()

print("FAILURES:", len(failures))
sys.exit(1 if failures else 0)
```

Pre-flight: confirm the daemon binary is current: `ls -la target/debug/brain-daemon` (rebuild with the macOS cargo wrapper ONLY if missing — no Rust surface changed in Inc 21, so an existing binary from the Inc 20 era remains valid).

- [ ] **Step 2: Run the smoke**

Run: `python3 scripts/ptySmokeInc21.py`
Expected: `FAILURES: 0`, exit 0. On failure, dump the buffer to `$CLAUDE_JOB_DIR/tmp/inc21-probe.txt` and diagnose against the documented PTY gotchas (occurrence counts, per-keystroke writes, differential rendering).

- [ ] **Step 3: Palette regression**

Run: `python3 scripts/ptySmokeInc2.py`
Expected: PASS — proves the migrated registry preserves palette narrowing/tab-complete/pass-through behavior.

- [ ] **Step 4: Full gates**

```bash
cd packages/brain-shell && bun test 2>&1 | tail -15
# failure identities must equal exactly the documented five
cd packages/brain-shell && bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK
cd packages/brain-shell && bun x tsc --noEmit 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -c "error TS"
git diff aa9a3554..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'
git status --porcelain | wc -l   # WIP preserved: compare to pre-increment snapshot
```
Expected: identities = documented five; `BUILD_OK`; tsc ≤ 434 + ambient-only deltas; vendor scan `0`; dirty-path total unchanged apart from intended increment files.

- [ ] **Step 5: Commit the smoke**

```bash
git add scripts/ptySmokeInc21.py
git commit -m "test(smoke): Inc 21 wire-level command-surface proof

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Verification Checklist (plan-level definition of done)

- [ ] All eight commands resolvable via `getCommand`, palette narrows over the registry catalog, `/help` lists eight alphabetically.
- [ ] Composer-pause invariant holds: with `/doctor` or `/memory` open, keystrokes reach ONLY the overlay input block (`paused` prop covers both flags).
- [ ] `/doctor` and `/memory` reachable end-to-end against a real daemon (smoke 10-check exit 0); `ptySmokeInc2.py` still green.
- [ ] Zero IPC/schema/Rust diffs: `git diff aa9a3554..HEAD --stat -- crates daemon` is EMPTY.
- [ ] `doctorProbe.ts` byte-identical to `aa9a3554`: `git diff aa9a3554..HEAD -- packages/brain-shell/src/adapter/doctorProbe.ts` is EMPTY.
- [ ] Deleted: only `commands/doctor/DoctorCommand.tsx` and `commands/memory/MemoryCommand.tsx`; `commands/config/*` untouched; no untracked-WIP path created or modified (`git status` shows the same ~3.7k dirty paths minus intended files).
- [ ] Gates: bundle OK; bun identities ⊆ documented five; tsc drift ambient-only; vendor scan 0.
