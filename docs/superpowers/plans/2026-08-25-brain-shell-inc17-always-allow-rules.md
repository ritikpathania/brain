# Brain Shell Inc 17 — Always-Allow Permission Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist always-allow rules (`{tool, inputPrefix}`) so matched tool-permission prompts auto-resolve over the existing `v1/tool/resolve` frame without parking the dialog, with a third "Always allow" dialog option to create rules and `/permissions` to manage them.

**Architecture:** Shell-only. A new `state/permissionRules.ts` owns the rule schema, tolerant config-file persistence (themeStore idiom), pure prefix matching over a shared primary-input extractor, and the `/permissions` output formatter. `SessionController.handleChunk` consults the store before parking the dialog: a match notices `Allowed <tool> (rule <n>)` and fires the same best-effort wire verdict the manual Allow button uses; if that verdict fails to deliver, the dialog is re-parked so failure degrades to manual approval, never silent deny. The daemon and wire format are untouched.

**Tech Stack:** Bun + TypeScript (brain-shell), `bun:test`, React 19/Ink 7 UI untouched except the three-option dialog row and command dispatch; Python 3 PTY harness for the end-to-end smoke.

**Spec:** `docs/superpowers/specs/2026-08-25-brain-shell-inc17-always-allow-rules-design.md`

## Global Constraints

- Preserve Brain's architecture, domain model, IPC contracts, runtime boundaries; shell-only change inside `packages/brain-shell`.
- No Claude/Anthropic models, APIs, authentication, pricing, billing, or LLM-specific product concepts; no vendor-derived code.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit contains ONLY explicitly-added paths (`git add <paths>`, NEVER `git add .`).
- Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- NEVER `git stash` this repo (~1k uncommitted user-WIP files); pushes to origin require explicit user approval each time.
- Sole permitted cargo failure: `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- macOS cargo wrapper for EVERY cargo invocation: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo ...'`.
- bun test path filters need the `./` prefix (`bun test ./src/test/...`); there is no `timeout` command on this macOS — use the Bash tool's timeout parameter.
- PTY fixtures byte-drift from capture nondeterminism; restore drift with `git checkout -- packages/brain-shell/src/test/fixtures/` and never commit it.
- Vendor scan scope is `crates daemon packages scripts` (docs excluded deliberately).
- `themeStore.configPath()` honors `BRAIN_CONFIG_PATH` resolved at CALL time — tests may set `process.env.BRAIN_CONFIG_PATH` per test without import-order tricks.
- Working directory for all bun commands is `/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell` unless stated.

---

### Task 0: Rider commit — remove unused @anthropic-ai/sdk dependency

**Files:**
- Modify: `packages/brain-shell/package.json` (delete line 14, `"@anthropic-ai/sdk": "^0.39.0",`)
- Modify: `packages/brain-lockfile` — none; `packages/brain-shell/bun.lock` regenerates in place.

**Interfaces:**
- Consumes: nothing.
- Produces: a dependency tree without `@anthropic-ai/sdk`; no source file changes (the package has zero imports in `src/` — verified by the vendor scan discipline).

- [ ] **Step 1: Prove zero imports before touching anything**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
grep -rn "@anthropic-ai" src/ | wc -l
```

Expected: `0`. If nonzero, STOP — the removal premise is false and the plan must be revisited.

- [ ] **Step 2: Create the branch and remove the dependency**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git checkout main && git pull --ff-only
git checkout -b feature/brain-shell-inc17-always-allow-rules
```

Edit `packages/brain-shell/package.json`: delete the entire line `"@anthropic-ai/sdk": "^0.39.0",`. Then regenerate the lockfile:

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun install
grep -c "@anthropic-ai" package.json bun.lock || true
```

Expected: `package.json` shows `0`; `bun.lock` either reports `0` or the grep finds no match (exit code tolerated by `|| true`). A stale-cache `bun install` error is not expected; if one occurs, rerun once before investigating.

- [ ] **Step 3: Sanity suite**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/
```

Expected: all state-dir tests pass except zero new failures relative to `main` (this dir currently contributes no documented failures).

- [ ] **Step 4: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/package.json packages/brain-shell/bun.lock
git commit -m "chore(shell): drop unused @anthropic-ai/sdk dependency

The shell never imported the SDK (zero references under src/); its
transitive presence in bun.lock was dead weight from an early scaffold.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 1: `permissionRules.ts` — matcher, store, formatters, command output

**Files:**
- Create: `packages/brain-shell/src/state/permissionRules.ts`
- Test: `packages/brain-shell/src/test/state/permissionRules.test.ts`
- Modify: `packages/brain-shell/src/ui/transcript/MessageRow.tsx` (`summarizeToolInput` refactor onto the shared extractor)

**Interfaces:**
- Consumes: `configPath()` from `./themeStore.js` (existing export).
- Produces, all exported from `state/permissionRules.ts`, used verbatim by Tasks 2–4:
  - `interface AllowRule { tool: string; inputPrefix: string }`
  - `primaryInputString(input: Record<string, unknown>): string`
  - `matchingRuleIndex(toolName: string, input: Record<string, unknown>, rules: readonly AllowRule[]): number`
  - `readAllowRules(): AllowRule[]` · `addAllowRule(rule: AllowRule): void` · `removeAllowRule(index: number): boolean`
  - `describeRule(rule: AllowRule): string` · `describeRules(rules: readonly AllowRule[]): string[]`
  - `runPermissionsCommand(args: readonly string[]): string`

- [ ] **Step 1: Write the failing tests**

Create `packages/brain-shell/src/test/state/permissionRules.test.ts`:

```ts
import { describe, it, expect, beforeEach } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
  addAllowRule,
  describeRule,
  describeRules,
  matchingRuleIndex,
  primaryInputString,
  readAllowRules,
  removeAllowRule,
  runPermissionsCommand,
} from '../../state/permissionRules.js';

let cfgPath: string;

beforeEach(() => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-inc17-rules-'));
  cfgPath = path.join(dir, 'config.json');
  process.env.BRAIN_CONFIG_PATH = cfgPath;
});

function seed(doc: unknown): void {
  fs.writeFileSync(cfgPath, JSON.stringify(doc));
}

const GIT_RULE = { tool: 'bash', inputPrefix: 'git ' };

describe('primaryInputString', () => {
  it('prefers canonical keys in declaration order', () => {
    expect(primaryInputString({ query: 'q', command: 'c' })).toBe('c');
    expect(primaryInputString({ file_path: '/a/b', path: '/z' })).toBe('/a/b');
  });

  it('falls back to the first non-empty string value', () => {
    expect(primaryInputString({ other: '  x  ', n: 3 })).toBe('x');
  });

  it('returns empty string when no string values exist', () => {
    expect(primaryInputString({})).toBe('');
    expect(primaryInputString({ depth: 2 })).toBe('');
  });
});

describe('matchingRuleIndex', () => {
  const rules = [GIT_RULE, { tool: 'read_file', inputPrefix: '' }];

  it('matches tool plus byte-exact case-sensitive prefix', () => {
    expect(matchingRuleIndex('bash', { command: 'git status' }, rules)).toBe(0);
    expect(matchingRuleIndex('bash', { command: 'Git status' }, rules)).toBe(-1);
    expect(matchingRuleIndex('bash', { command: 'rm -rf /' }, rules)).toBe(-1);
    expect(matchingRuleIndex('write_file', { command: 'git status' }, rules)).toBe(-1);
  });

  it('treats an empty prefix as any invocation of the tool', () => {
    expect(matchingRuleIndex('read_file', { path: '/etc/hosts' }, rules)).toBe(1);
    expect(matchingRuleIndex('read_file', {}, rules)).toBe(1);
  });
});

describe('store round-trips', () => {
  it('reads [] when the file or key is missing', () => {
    expect(readAllowRules()).toEqual([]);
    seed({ theme: 'dark' });
    expect(readAllowRules()).toEqual([]);
  });

  it('filters malformed entries but keeps valid ones', () => {
    seed({
      permissions: {
        allow: [
          GIT_RULE,
          { tool: '', inputPrefix: 'x' },
          { tool: 7, inputPrefix: 'y' },
          { tool: 'ok' },
          'junk',
        ],
      },
    });
    expect(readAllowRules()).toEqual([GIT_RULE]);
  });

  it('merge-writes a rule while preserving sibling keys', () => {
    seed({ theme: 'dark', other: { nested: true } });
    addAllowRule(GIT_RULE);
    const doc = JSON.parse(fs.readFileSync(cfgPath, 'utf8'));
    expect(doc.theme).toBe('dark');
    expect(doc.other).toEqual({ nested: true });
    expect(doc.permissions.allow).toEqual([GIT_RULE]);
  });

  it('dedupes identical rules instead of appending', () => {
    addAllowRule(GIT_RULE);
    addAllowRule(GIT_RULE);
    expect(readAllowRules()).toEqual([GIT_RULE]);
  });

  it('removes by index and reports out-of-range as false', () => {
    addAllowRule(GIT_RULE);
    addAllowRule({ tool: 'read_file', inputPrefix: '' });
    expect(removeAllowRule(0)).toBe(true);
    expect(readAllowRules()).toEqual([{ tool: 'read_file', inputPrefix: '' }]);
    expect(removeAllowRule(5)).toBe(false);
    expect(removeAllowRule(-1)).toBe(false);
  });
});

describe('formatters and command output', () => {
  it('describes prefixed and tool-wide rules through one formatter', () => {
    expect(describeRule(GIT_RULE)).toBe('bash — commands starting with "git "');
    expect(describeRule({ tool: 'read_file', inputPrefix: '' })).toBe(
      'read_file — any invocation',
    );
    expect(describeRules([GIT_RULE])).toEqual([' 1. bash — commands starting with "git "']);
  });

  it('runPermissionsCommand lists, removes, and rejects bad usage', () => {
    seed({ permissions: { allow: [GIT_RULE] } });
    const out = runPermissionsCommand([]);
    expect(out).toContain(`Always-allow rules (${cfgPath}):`);
    expect(out).toContain(' 1. bash — commands starting with "git "');
    expect(out).toContain('Remove with: /permissions remove <n>');

    expect(runPermissionsCommand(['remove', '1'])).toBe(
      'Removed rule 1 (bash — commands starting with "git ").',
    );
    expect(runPermissionsCommand([])).toBe('No always-allow rules saved.');
    expect(runPermissionsCommand(['remove', '9'])).toBe('No rule 9.');
    expect(runPermissionsCommand(['remove', 'x'])).toBe(
      'Usage: /permissions remove <rule number>',
    );
    expect(runPermissionsCommand(['frobnicate'])).toBe(
      'Usage: /permissions [remove <rule number>]',
    );
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/permissionRules.test.ts
```

Expected: FAIL — `module not found "…/state/permissionRules.js"` (or equivalent resolution error) before any assertion runs.

- [ ] **Step 3: Implement the module**

Create `packages/brain-shell/src/state/permissionRules.ts`:

```ts
/**
 * Always-allow rule store for tool permissions (Inc 17). Rules persist as
 * the `permissions.allow` array of the user's brain config file — the same
 * document themeStore owns a key of — and every check reads fresh from disk
 * so edits (via /permissions or by hand) apply immediately.
 */
import * as fs from 'fs';
import * as path from 'path';
import { configPath } from './themeStore.js';

export interface AllowRule {
  /** Tool name exactly as the daemon reports it (e.g. 'bash'). */
  tool: string;
  /** Byte prefix matched against the tool's primary input string;
   * '' matches every invocation of the tool. */
  inputPrefix: string;
}

/** Keys searched first, mirroring the dialog summarizer's preference order. */
const PRIMARY_KEYS: readonly string[] = [
  'command',
  'file_path',
  'path',
  'query',
  'pattern',
  'url',
  'prompt',
];

/**
 * The input's primary string: first non-empty trimmed value among the
 * canonical keys, else the first non-empty trimmed string value, else ''.
 * Shared single source of truth for rule matching and dialog display.
 */
export function primaryInputString(input: Record<string, unknown>): string {
  for (const key of PRIMARY_KEYS) {
    const v = input[key];
    if (typeof v === 'string' && v.trim().length > 0) return v.trim();
  }
  for (const v of Object.values(input)) {
    if (typeof v === 'string' && v.trim().length > 0) return v.trim();
  }
  return '';
}

/** First matching rule index, or -1. Byte-exact, case-sensitive prefix. */
export function matchingRuleIndex(
  toolName: string,
  input: Record<string, unknown>,
  rules: readonly AllowRule[],
): number {
  const primary = primaryInputString(input);
  return rules.findIndex((r) => r.tool === toolName && primary.startsWith(r.inputPrefix));
}

function readDoc(): Record<string, unknown> {
  try {
    const parsed = JSON.parse(fs.readFileSync(configPath(), 'utf8')) as unknown;
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : {};
  } catch {
    // missing file / bad JSON / unreadable path -> fresh document
    return {};
  }
}

function writeDoc(doc: Record<string, unknown>): void {
  fs.mkdirSync(path.dirname(configPath()), { recursive: true });
  fs.writeFileSync(configPath(), JSON.stringify(doc, null, 2) + '\n');
}

/** Tolerant parse; anything without a non-empty string `tool` and a string
 * `inputPrefix` is dropped rather than trusted. */
function parseRules(value: unknown): AllowRule[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (r): r is AllowRule =>
      r !== null &&
      typeof r === 'object' &&
      typeof (r as AllowRule).tool === 'string' &&
      (r as AllowRule).tool.length > 0 &&
      typeof (r as AllowRule).inputPrefix === 'string',
  );
}

function currentRules(doc: Record<string, unknown>): {
  perms: Record<string, unknown>;
  rules: AllowRule[];
} {
  const raw = doc.permissions;
  const perms =
    raw !== null && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  return { perms, rules: parseRules(perms.allow) };
}

/** Tolerant read; missing file/key or malformed entries yield fewer/no rules. */
export function readAllowRules(): AllowRule[] {
  return currentRules(readDoc()).rules;
}

/** Merge-write one rule; an identical existing rule is left as-is. */
export function addAllowRule(rule: AllowRule): void {
  const doc = readDoc();
  const { perms, rules } = currentRules(doc);
  if (!rules.some((r) => r.tool === rule.tool && r.inputPrefix === rule.inputPrefix)) {
    rules.push(rule);
  }
  perms.allow = rules;
  doc.permissions = perms;
  writeDoc(doc);
}

/** Remove the nth rule of the current read order; false when out of range. */
export function removeAllowRule(index: number): boolean {
  const doc = readDoc();
  const { perms, rules } = currentRules(doc);
  if (!Number.isInteger(index) || index < 0 || index >= rules.length) return false;
  rules.splice(index, 1);
  perms.allow = rules;
  doc.permissions = perms;
  writeDoc(doc);
  return true;
}

/** Human description shared by the /permissions listing and removal notes. */
export function describeRule(rule: AllowRule): string {
  return rule.inputPrefix.length > 0
    ? `${rule.tool} — commands starting with "${rule.inputPrefix}"`
    : `${rule.tool} — any invocation`;
}

export function describeRules(rules: readonly AllowRule[]): string[] {
  return rules.map((r, i) => ` ${i + 1}. ${describeRule(r)}`);
}

/**
 * Full output of `/permissions [remove <n>]` as one notice block. Performs
 * its own store reads/writes so the AppShell dispatch stays a single call.
 */
export function runPermissionsCommand(args: readonly string[]): string {
  if (args.length === 0) {
    const rules = readAllowRules();
    if (rules.length === 0) return 'No always-allow rules saved.';
    return [
      `Always-allow rules (${configPath()}):`,
      ...describeRules(rules),
      'Remove with: /permissions remove <n>',
    ].join('\n');
  }
  if (args[0] === 'remove') {
    const raw = args[1] ?? '';
    if (!/^\d+$/.test(raw)) return 'Usage: /permissions remove <rule number>';
    const n = Number.parseInt(raw, 10);
    const target = readAllowRules()[n - 1];
    if (target === undefined) return `No rule ${n}.`;
    removeAllowRule(n - 1);
    return `Removed rule ${n} (${describeRule(target)}).`;
  }
  return 'Usage: /permissions [remove <rule number>]';
}
```

Then refactor the summarizer in `packages/brain-shell/src/ui/transcript/MessageRow.tsx`. Replace the whole current body:

```ts
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
```

with:

```ts
export function summarizeToolInput(input: Record<string, unknown>): string {
  return primaryInputString(input).slice(0, 60);
}
```

and add to the imports at the top of `MessageRow.tsx`:

```ts
import { primaryInputString } from '../../state/permissionRules.js';
```

Behavior is byte-identical (trim-then-slice order preserved; the canonical key list moves into `PRIMARY_KEYS`).

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/permissionRules.test.ts
bun test ./src/test/ui/messageRowView.test.tsx
```

Expected: permissionRules **12 pass / 0 fail**; messageRowView passes unchanged (the refactor is behavior-identical).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/state/permissionRules.ts packages/brain-shell/src/test/state/permissionRules.test.ts packages/brain-shell/src/ui/transcript/MessageRow.tsx
git commit -m "feat(shell): always-allow rule store with prefix matching

Inc 17 B2: AllowRule {tool, inputPrefix} persisted as permissions.allow
in the brain config document (themeStore merge-write idiom, theme key
preserved), read fresh on every check. The primary-input extractor is
lifted out of summarizeToolInput so dialog display and rule matching
share one canonical notion of the tool's main string. Includes the
pure /permissions output formatter.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Controller auto-allow decision point + Always-allow resolution

**Files:**
- Modify: `packages/brain-shell/src/state/sessionController.ts` (permission branch ~`:324`, `resolvePermission` region ~`:106-123`)
- Test: `packages/brain-shell/src/test/state/sessionControllerPermission.test.ts`

**Interfaces:**
- Consumes from Task 1: `addAllowRule`, `matchingRuleIndex`, `primaryInputString`, `readAllowRules`.
- Produces: `SessionController.resolvePermissionAlways(callId: string): void` (public, called by AppShell in Task 3); private `autoAllow(view, ruleNumber)`; changed behavior of the `permission_request` branch (auto-resolve on rule match). Existing public surface unchanged.

- [ ] **Step 1: Write the failing tests**

Create `packages/brain-shell/src/test/state/sessionControllerPermission.test.ts`:

```ts
import { describe, it, expect, beforeEach } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

let cfgPath: string;

beforeEach(() => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-inc17-ctl-'));
  cfgPath = path.join(dir, 'config.json');
  process.env.BRAIN_CONFIG_PATH = cfgPath;
});

function seedRule(): void {
  fs.writeFileSync(
    cfgPath,
    JSON.stringify({
      theme: 'dark',
      permissions: { allow: [{ tool: 'bash', inputPrefix: 'git ' }] },
    }),
  );
}

interface Resolution {
  callId: string;
  granted: boolean;
}

function recordingClient(
  chunks: BrainStreamChunk[],
  opts: { rejectResolve?: boolean } = {},
): { client: BrainBackendClient; resolutions: Resolution[] } {
  const resolutions: Resolution[] = [];
  const client = {
    async createSession() {
      return { sessionId: 'perm-probe', title: 't', createdAtMs: 0 };
    },
    async *streamText(_req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      yield* chunks;
    },
    async resolveToolPermission(callId: string, granted: boolean): Promise<void> {
      if (opts.rejectResolve) {
        throw new Error('Brain daemon socket error on v1/tool/resolve: boom');
      }
      resolutions.push({ callId, granted });
    },
  } as unknown as BrainBackendClient;
  return { client, resolutions };
}

function permChunk(command: string, callId = 'c1'): BrainStreamChunk {
  return {
    type: 'permission_request',
    callId,
    toolName: 'bash',
    input: { command },
  } as unknown as BrainStreamChunk;
}

function systemText(ctl: SessionController): string {
  return ctl
    .getSnapshot()
    .rows.filter((r) => r.kind === 'system')
    .map((r) => r.text)
    .join('\n');
}

describe('Inc 17: controller auto-allow from saved rules', () => {
  it('auto-allows a matching rule without parking the dialog', async () => {
    seedRule();
    const { client, resolutions } = recordingClient([
      permChunk('git status'),
      { type: 'token', token: 'clean tree' },
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('check the repo');
    expect(ctl.getSnapshot().permission).toBeUndefined();
    expect(resolutions).toEqual([{ callId: 'c1', granted: true }]);
    expect(systemText(ctl)).toContain('Allowed bash (rule 1)');
    ctl.dispose();
  });

  it('parks unmatched requests and resolves them manually as before', async () => {
    const { client, resolutions } = recordingClient([
      permChunk('rm -rf build'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('clean up');
    expect(ctl.getSnapshot().permission?.callId).toBe('c1');
    expect(resolutions).toEqual([]);
    ctl.resolvePermission('c1', true);
    expect(resolutions).toEqual([{ callId: 'c1', granted: true }]);
    expect(ctl.getSnapshot().permission).toBeUndefined();
    ctl.dispose();
  });

  it('re-parks the dialog when the wire verdict fails to deliver', async () => {
    seedRule();
    const { client } = recordingClient(
      [
        permChunk('git push'),
        { type: 'token', token: 'partial' },
        { type: 'finished', status: 'completed' },
      ],
      { rejectResolve: true },
    );
    const ctl = new SessionController(client);
    await ctl.submit('push it');
    expect(ctl.getSnapshot().permission).toBeUndefined(); // not parked synchronously
    await sleep(5); // let the rejected promise route through the fallback
    expect(ctl.getSnapshot().permission?.callId).toBe('c1');
    ctl.dispose();
  });

  it('resolvePermissionAlways persists the derived rule and grants', async () => {
    const { client, resolutions } = recordingClient([
      permChunk('git fetch', 'c9'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('fetch refs');
    ctl.resolvePermissionAlways('c9');
    expect(resolutions).toEqual([{ callId: 'c9', granted: true }]);
    const saved = JSON.parse(fs.readFileSync(cfgPath, 'utf8')) as {
      permissions?: { allow?: Array<{ tool: string; inputPrefix: string }> };
    };
    expect(saved.permissions?.allow).toEqual([{ tool: 'bash', inputPrefix: 'git ' }]);

    // The saved rule takes effect on the very next request.
    await ctl.submit('fetch again');
    expect(ctl.getSnapshot().permission).toBeUndefined();
    expect(resolutions).toEqual([
      { callId: 'c9', granted: true },
      { callId: 'c9', granted: true },
    ]);
    expect(systemText(ctl)).toContain('Allowed bash (rule 1)');
    ctl.dispose();
  });

  it('still grants this call when saving the rule fails', async () => {
    process.env.BRAIN_CONFIG_PATH = path.join(os.tmpdir(), 'brain-inc17-dir-not-file');
    fs.rmSync(process.env.BRAIN_CONFIG_PATH!, { recursive: true, force: true });
    fs.mkdirSync(process.env.BRAIN_CONFIG_PATH!); // configPath() now names a directory
    const { client, resolutions } = recordingClient([
      permChunk('git log', 'cf'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('show log');
    ctl.resolvePermissionAlways('cf');
    expect(systemText(ctl)).toContain('Could not save the always-allow rule.');
    expect(resolutions).toEqual([{ callId: 'cf', granted: true }]);
    expect(ctl.getSnapshot().permission).toBeUndefined();
    ctl.dispose();
  });

  it('derives a tool-wide rule from inputs without a string field', async () => {
    const { client, resolutions } = recordingClient([
      permChunk('', 'ce'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('go');
    ctl.resolvePermissionAlways('ce'); // input {command:''} has no primary string
    const saved = JSON.parse(fs.readFileSync(cfgPath, 'utf8')) as {
      permissions?: { allow?: Array<{ tool: string; inputPrefix: string }> };
    };
    expect(saved.permissions?.allow).toEqual([{ tool: 'bash', inputPrefix: '' }]);
    ctl.dispose();
  });
});
```

Note: chunk literals cast through `unknown` because the `BrainStreamChunk` union marks these fields optional; the runtime shape matches what `UdsBrainBackendClient` produces (`callId`, `toolName`, `input`).

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/sessionControllerPermission.test.ts
```

Expected: FAIL — `resolvePermissionAlways is not a function` on the always-tests, and the auto-allow/fallback tests fail on `permission` being parked (object, not undefined) and/or missing `Allowed bash (rule 1)` notice. The manual-park test (test 2) PASSES already — today's behavior is its baseline.

- [ ] **Step 3: Implement the controller changes**

In `packages/brain-shell/src/state/sessionController.ts`:

**(a)** Add this import alongside the other state imports (after the `probeDaemonSocket` import):

```ts
import {
  addAllowRule,
  matchingRuleIndex,
  primaryInputString,
  readAllowRules,
} from './permissionRules.js';
```

**(b)** Replace the entire `permission_request` branch inside `handleChunk` (currently `sessionController.ts:321-333`, comment included):

```ts
    // Permission requests are handled LIVE here, not routed into turn
    // events — they park a dialog on the snapshot and resolve locally
    // (the wire has no resolution frame yet).
    if (chunk.type === 'permission_request' && typeof chunk.callId === 'string') {
      this.pendingPermission = {
        callId: chunk.callId,
        toolName: chunk.toolName ?? 'tool',
        input: chunk.input ?? {},
        reason: chunk.reason,
      };
      this.emit();
      return;
    }
```

with:

```ts
    // Permission requests either auto-allow from a saved rule (Inc 17) or
    // park a dialog on the snapshot; both paths end in a wire verdict via
    // v1/tool/resolve, which the daemon parks the stream awaiting.
    if (chunk.type === 'permission_request' && typeof chunk.callId === 'string') {
      const view: PendingPermissionView = {
        callId: chunk.callId,
        toolName: chunk.toolName ?? 'tool',
        input: chunk.input ?? {},
        reason: chunk.reason,
      };
      const ruleNumber =
        matchingRuleIndex(view.toolName, view.input, readAllowRules()) + 1;
      if (ruleNumber > 0) this.autoAllow(view, ruleNumber);
      else {
        this.pendingPermission = view;
        this.emit();
      }
      return;
    }
```

**(c)** Replace the `resolvePermission` block (`sessionController.ts:106-123`, including the stale docblock — its claim "The daemon cannot receive resolutions yet" contradicts the wire call in its own body) with three methods:

```ts
  /** Resolve the parked dialog manually. Local UX settles first; the wire
   * verdict rides v1/tool/resolve best-effort — legacy fakes and offline
   * daemons simply omit or reject the call, while live daemons gate
   * execution on it. */
  resolvePermission(callId: string, granted: boolean): void {
    if (this.pendingPermission?.callId !== callId) return;
    const toolName = this.pendingPermission.toolName;
    this.pendingPermission = undefined;
    if (!granted) {
      this.rows = this.rows.map((r) =>
        r.kind === 'tool' && r.tool.callId === callId
          ? { ...r, tool: { ...r.tool, status: 'denied' as const } }
          : r,
      );
    }
    // Best-effort wire round-trip; local UX above is already settled.
    void this.client.resolveToolPermission?.(callId, granted)?.catch(() => {});
    this.notice(`${granted ? 'Allowed' : 'Denied'} ${toolName}`);
  }

  /** Inc 17: grant AND persist an always-allow rule derived from the pending
   * view. A save failure only skips persistence — this call is still granted. */
  resolvePermissionAlways(callId: string): void {
    const view = this.pendingPermission;
    if (view?.callId !== callId) return;
    try {
      addAllowRule({
        tool: view.toolName,
        inputPrefix: primaryInputString(view.input),
      });
    } catch {
      this.notice('Could not save the always-allow rule.');
    }
    this.resolvePermission(callId, true);
  }

  /** Saved-rule hit: never park the dialog. If the wire verdict cannot be
   * delivered, re-park so the user decides manually — a silent drop would
   * leave the daemon's waiter parked until its deny-by-default timeout. */
  private autoAllow(view: PendingPermissionView, ruleNumber: number): void {
    this.notice(`Allowed ${view.toolName} (rule ${ruleNumber})`);
    const park = (): void => {
      this.pendingPermission = view;
      this.emit();
    };
    try {
      const p = this.client.resolveToolPermission?.(view.callId, true);
      if (p) p.catch(park);
    } catch {
      park();
    }
  }
```

Nothing else in the file changes.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/state/sessionControllerPermission.test.ts
bun test ./src/test/state/sessionControllerReconnect.test.ts
bun test ./src/test/state/sessionControllerFreeze.test.ts
```

Expected: permission **6 pass / 0 fail**; reconnect **3 pass / 0 fail**; freeze **6 pass / 0 fail** (unchanged — their stub clients have no `resolveToolPermission` and never send `permission_request`).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionControllerPermission.test.ts
git commit -m "feat(shell): auto-allow matched permission requests from saved rules

handleChunk now consults the rule store before parking the dialog; a
match notices 'Allowed <tool> (rule <n>)' and delivers the same
v1/tool/resolve verdict the manual Allow button uses. If that verdict
fails to deliver, the dialog re-parks so the outcome is a manual
decision, never the daemon's deny-by-default timeout. New
resolvePermissionAlways grants the pending call and persists a rule
derived from its primary input string. Rewrites the stale docblock
that claimed resolutions cannot reach the daemon.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Third dialog option "Always allow"

**Files:**
- Modify: `packages/brain-shell/src/keybindings/resolve.ts` (binding table, after `dialog:deny`)
- Modify: `packages/brain-shell/src/ui/overlays/permissionDialogLogic.ts` (whole file)
- Modify: `packages/brain-shell/src/ui/overlays/PermissionDialog.tsx` (options row + help line)
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (dialog handler)
- Test: `packages/brain-shell/src/test/ui/overlays/permissionDialogLogic.test.ts` (extend)
- Test: `packages/brain-shell/src/test/ui/overlays/permissionDialogView.test.tsx` (extend)

**Interfaces:**
- Consumes from Task 2: `SessionController.resolvePermissionAlways(callId)`.
- Produces: `dialogDecision` gains decision `{ type: 'always' }` and widens `move.index` to `0 | 1 | 2` with clamped relative arrows; keymap action `'dialog:always'` bound to `a`.

- [ ] **Step 1: Write the failing tests**

In `packages/brain-shell/src/test/ui/overlays/permissionDialogLogic.test.ts`, replace the two middle assertions blocks so the whole file reads:

```ts
import { describe, expect, test } from 'bun:test';
import { dialogDecision } from '../../../ui/overlays/permissionDialogLogic.js';

describe('dialogDecision', () => {
  test('direct keys decide', () => {
    expect(dialogDecision('dialog:allow', 2)).toEqual({ type: 'allow' });
    expect(dialogDecision('dialog:always', 0)).toEqual({ type: 'always' }); // the a key
    expect(dialogDecision('dialog:deny', 0)).toEqual({ type: 'deny' });
    expect(dialogDecision('dialog:cancel', 0)).toEqual({ type: 'deny' }); // esc denies
  });

  test('arrows move relatively within [Allow, Deny, Always]; enter confirms', () => {
    expect(dialogDecision('dialog:left', 1)).toEqual({ type: 'move', index: 0 });
    expect(dialogDecision('dialog:left', 0)).toEqual({ type: 'move', index: 0 });
    expect(dialogDecision('dialog:right', 0)).toEqual({ type: 'move', index: 1 });
    expect(dialogDecision('dialog:right', 1)).toEqual({ type: 'move', index: 2 });
    expect(dialogDecision('dialog:right', 2)).toEqual({ type: 'move', index: 2 });
    expect(dialogDecision('dialog:left', 2)).toEqual({ type: 'move', index: 1 });
    expect(dialogDecision('dialog:commit', 0)).toEqual({ type: 'allow' });
    expect(dialogDecision('dialog:commit', 1)).toEqual({ type: 'deny' });
    expect(dialogDecision('dialog:commit', 2)).toEqual({ type: 'always' });
  });

  test('null and unrelated actions pass through', () => {
    expect(dialogDecision(null, 0)).toEqual({ type: 'passthrough' });
    expect(dialogDecision('overlay:up', 0)).toEqual({ type: 'passthrough' });
  });
});
```

(The one intentional behavior change vs the old expectations: arrows moved absolutely — right jumped straight to Deny — which has no meaning across three options; they now move relatively and clamp. Every old assertion except `right(1) → index 1` is preserved verbatim.)

Extend the existing test in `packages/brain-shell/src/test/ui/overlays/permissionDialogView.test.tsx` — replace its single test body with:

```ts
  test('shows tool, summarized input, and all three options with selection', () => {
    const text = textOf(
      PermissionDialogView({
        req: {
          callId: 'c1',
          toolName: 'bash',
          input: { command: 'rm -rf build' },
          reason: 'destructive',
        },
        selected: 1,
        tokens: PALETTES.dark,
      }),
    );
    expect(text).toContain('Permission required');
    expect(text).toContain('bash');
    expect(text).toContain('rm -rf build');
    expect(text).toContain('[ Deny ]');
    expect(text).toContain('[ Allow ]');
    expect(text).toContain('[ Always allow ]');
    expect(text).toContain('esc denies');
    expect(text).toContain('a always');

    const alwaysSelected = textOf(
      PermissionDialogView({
        req: { callId: 'c1', toolName: 'bash', input: { command: 'ls' } },
        selected: 2,
        tokens: PALETTES.dark,
      }),
    );
    expect(alwaysSelected).toContain('❯ [ Always allow ]');
    expect(alwaysSelected).not.toContain('❯ [ Deny ]');
  });
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/ui/overlays/permissionDialogLogic.test.ts ./src/test/ui/overlays/permissionDialogView.test.tsx
```

Expected: logic FAIL — `dialog:always` returns passthrough, `right(1)` returns index 1 not 2, `commit(2)` returns deny; view FAIL — `[ Always allow ]` absent.

- [ ] **Step 3: Implement**

**(a)** `keybindings/resolve.ts` — update the section comment and insert one binding so the block reads:

```ts
  // Permission dialog: left/right choose, y allow, a always, n deny, enter
  // confirms, esc denies.
  { action: 'dialog:left', context: 'dialog', key: 'left' },
  { action: 'dialog:right', context: 'dialog', key: 'right' },
  { action: 'dialog:allow', context: 'dialog', key: 'y' },
  { action: 'dialog:always', context: 'dialog', key: 'a' },
  { action: 'dialog:deny', context: 'dialog', key: 'n' },
  { action: 'dialog:commit', context: 'dialog', key: 'return' },
  { action: 'dialog:cancel', context: 'dialog', key: 'escape' },
```

**(b)** `ui/overlays/permissionDialogLogic.ts` — replace the whole file:

```ts
/**
 * Decision table for the permission dialog. Options are fixed:
 * index 0 = Allow, index 1 = Deny, index 2 = Always allow (grant and
 * persist a rule, Inc 17). Arrows move relatively and clamp; esc always
 * denies — a permission the user dismisses is a permission not granted.
 */
export type DialogDecision =
  | { type: 'allow' }
  | { type: 'deny' }
  | { type: 'always' }
  | { type: 'move'; index: 0 | 1 | 2 }
  | { type: 'passthrough' };

export function dialogDecision(action: string | null, selected: number): DialogDecision {
  if (action === null) return { type: 'passthrough' };
  switch (action) {
    case 'dialog:allow':
      return { type: 'allow' };
    case 'dialog:always':
      return { type: 'always' };
    case 'dialog:deny':
    case 'dialog:cancel':
      return { type: 'deny' };
    case 'dialog:left':
      return { type: 'move', index: Math.max(0, selected - 1) as 0 | 1 | 2 };
    case 'dialog:right':
      return { type: 'move', index: Math.min(2, selected + 1) as 0 | 1 | 2 };
    case 'dialog:commit':
      return selected === 0 ? { type: 'allow' } : selected === 2 ? { type: 'always' } : { type: 'deny' };
    default:
      return { type: 'passthrough' };
  }
}
```

**(c)** `ui/overlays/PermissionDialog.tsx` — replace the options row and help line:

```tsx
      <Text>
        {opt('Allow', 0)}   {opt('Deny', 1)}   {opt('Always allow', 2)}
      </Text>
      <Text dimColor>←→ choose · enter confirm · y allow · a always · n deny · esc denies</Text>
```

**(d)** `ui/shell/AppShell.tsx` — inside the permission `useBoundInput` handler, insert the `always` arm between `allow` and `deny`:

```ts
      if (d.type === 'move') {
        setPermSelected(d.index);
      } else if (d.type === 'allow') {
        controller.resolvePermission(permission.callId, true);
      } else if (d.type === 'always') {
        controller.resolvePermissionAlways(permission.callId);
      } else if (d.type === 'deny') {
        controller.resolvePermission(permission.callId, false);
      }
```

No other AppShell edits in this task (`permSelected` is untyped `useState(0)`; `0 | 1 | 2` assigns fine).

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/ui/overlays/
bun test ./src/test/keybindings/
```

Expected: overlays directory all pass (logic 4 tests, view extended, resume/theme/picker suites untouched); keybindings suite passes unchanged (one binding added, dispatcher generic).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/keybindings/resolve.ts packages/brain-shell/src/ui/overlays/permissionDialogLogic.ts packages/brain-shell/src/ui/overlays/PermissionDialog.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/ui/overlays/permissionDialogLogic.test.ts packages/brain-shell/src/test/ui/overlays/permissionDialogView.test.tsx
git commit -m "feat(shell): third permission option Always allow persists a rule

Arrows become relative with clamping across the three options; the a
key and enter-on-index-2 route to resolvePermissionAlways, which
grants and persists. y/n/esc semantics untouched — esc still denies.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: `/permissions` slash command

**Files:**
- Modify: `packages/brain-shell/src/commands/matcher.ts` (COMMANDS entry between `theme` and `quit`)
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (`runCommand` arg splitting + dispatch)
- Test: `packages/brain-shell/src/test/commands/matcher.test.ts:51` (exact-list update)

**Interfaces:**
- Consumes from Task 1: `runPermissionsCommand(args: readonly string[]): string`.
- Produces: slash command `permissions` reachable bare or with args; `runCommand` now exposes subcommand args to handlers via a `words` split.

- [ ] **Step 1: Update the failing test first**

In `packages/brain-shell/src/test/commands/matcher.test.ts`, line 51 becomes:

```ts
    expect(COMMANDS.map((c) => c.name).sort()).toEqual([
      'clear',
      'help',
      'permissions',
      'quit',
      'resume',
      'theme',
    ]);
```

- [ ] **Step 2: Run to verify the failure mode is only the registry mismatch**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/commands/matcher.test.ts
```

Expected: FAIL on exactly that expectation (actual list lacks `'permissions'`).

- [ ] **Step 3: Implement**

**(a)** `commands/matcher.ts` — insert between the `theme` and `quit` entries:

```ts
  { name: 'theme', description: 'Change the color theme' },
  { name: 'permissions', description: 'List or remove always-allow rules' },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
```

**(b)** `ui/shell/AppShell.tsx` — change the head of `runCommand` from:

```ts
  const runCommand = (rawValue: string): void => {
    const token = rawValue.trim().slice(1).toLowerCase(); // strip '/', tolerate trailing space
```

to:

```ts
  const runCommand = (rawValue: string): void => {
    const words = rawValue.trim().slice(1).split(/\s+/); // strip '/', split args
    const token = (words[0] ?? '').toLowerCase();
    const args = words.slice(1);
```

(the rest of the matcher logic keeps using `token`; `/clear now` continues to match `clear` by exact name just as it did by prefix before — behavior for all existing commands is unchanged because they ignore extra words)

and add the dispatch arm before `quit`:

```ts
    } else if (chosen.name === 'theme') {
      ...existing...
    } else if (chosen.name === 'permissions') {
      controller.notice(runPermissionsCommand(args));
    } else if (chosen.name === 'quit') process.exit(0);
```

with the import added alongside the other state imports:

```ts
import { runPermissionsCommand } from '../../state/permissionRules.js';
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test ./src/test/commands/matcher.test.ts
```

Expected: PASS. (`runPermissionsCommand`'s own behavior is fully covered by Task 1's tests; the dispatch arm is a one-line delegation.)

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/src/commands/matcher.ts packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/commands/matcher.test.ts
git commit -m "feat(shell): /permissions lists and removes always-allow rules

Bare form prints the numbered rule listing against the real config
path; 'remove <n>' deletes by 1-based index. Dispatch splits the
composer input into command word + args so subcommands reach handlers.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: End-to-end PTY smoke + full gates + finishing

**Files:**
- Create: `scripts/ptySmokeInc17.py`

**Interfaces:**
- Consumes: everything built in Tasks 1–4, exercised through the real UDS client against a stub daemon speaking the verified wire shapes (`tool_permission_requested` snake_case accepted by `UdsBrainBackendClient.ts:267-280`; RPC frames `{id, action, payload, body}` per `callRpc` at `UdsBrainBackendClient.ts:508-514`).
- Produces: gate evidence for finishing.

- [ ] **Step 1: Full bun suite**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bun test
```

Expected: **299 tests / 294 pass / 5 fail**, the five failures exactly the documented pre-existing set (visualCellParity ×2, sessionSemanticIntegration, brainMemoryIntegration, brainTurnTransformer Scenario 8). Baseline 280 + 19 new `it()`s (12 rules + 6 controller + 1 dialog-logic). If any ADDITIONAL test fails — e.g. an exact `/help` listing or golden-screen fixture that enumerates slash commands — STOP and surface it; do not silently edit parity fixtures.

- [ ] **Step 2: Write the PTY smoke**

Create `scripts/ptySmokeInc17.py`:

```python
#!/usr/bin/env python3
"""Increment 17 PTY smoke: saved-rule auto-allow end to end.

Config pre-seeds an always-allow rule for bash commands starting with
"git ". The stub daemon emits a tool_use packet plus a
tool_permission_requested frame mid-stream and PARKS until it sees
v1/tool/resolve on a second connection — exactly like the real daemon's
waiter. The shell must auto-allow (notice 'Allowed bash (rule 1)'),
deliver granted=true over the wire (script-level assertion against the
stub's recorded resolutions), never render the permission dialog
(cumulative-buffer absence check is sound: once emitted, always in buf),
and freeze the post-grant answer.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc17-smoke.sock"
CONFIG_FILE = "/tmp/brain-inc17-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
PKG_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"

RESOLUTIONS = []
RESOLVED = threading.Event()

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto",
               "permissions": {"allow": [{"tool": "bash", "inputPrefix": "git "}]}}, f)

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
                    rid = req.get("id")
                    act = req.get("action")
                    payload = req.get("payload") or {}
                    if not payload and isinstance(req.get("body"), str):
                        try:
                            payload = json.loads(req["body"])
                        except Exception:
                            payload = {}
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-s17"}})
                    elif act == "v1/generation/stream":
                        reply({"type": "stream_start", "session_id": "stub-s17",
                               "sequence": 0})
                        time.sleep(0.3)
                        reply({"type": "tool_use", "session_id": "stub-s17",
                               "toolUse": {"id": "call-17", "name": "bash",
                                           "input": {"command": "git status"}},
                               "sequence": 1})
                        reply({"type": "tool_permission_requested",
                               "session_id": "stub-s17",
                               "call_id": "call-17", "tool_name": "bash",
                               "input": {"command": "git status"},
                               "reason": "tool execution requires approval",
                               "sequence": 2})
                        # Park like the real waiter until a verdict arrives
                        RESOLVED.wait(timeout=15)
                        reply({"type": "tool_result", "session_id": "stub-s17",
                               "call_id": "call-17", "output": "On branch main",
                               "is_error": False, "exit_code": 0,
                               "sequence": 3})
                        time.sleep(0.2)
                        reply({"type": "token", "session_id": "stub-s17",
                               "token": "Done.", "sequence": 4})
                        reply({"type": "finished", "session_id": "stub-s17",
                               "status": "completed", "sequence": 5})
                    elif act == "v1/tool/resolve":
                        RESOLUTIONS.append({"call_id": payload.get("call_id"),
                                            "granted": payload.get("granted")})
                        RESOLVED.set()
                        reply({"id": rid, "status": "success", "body": {}})
                    else:
                        reply({"id": rid, "status": "success", "body": {}})
            except Exception:
                pass
            finally:
                try:
                    conn.close()
                except Exception:
                    pass
        threading.Thread(target=handle, daemon=True).start()

srv_thread = threading.Thread(target=serve, daemon=True)
srv_thread.start()
time.sleep(0.3)  # let bind() land before the child connects

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.environ["BRAIN_CONFIG_PATH"] = CONFIG_FILE
    os.chdir(PKG_DIR)
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

def expect(label, needle, timeout=10.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print("PASS " + label)
            return True
    print("FAIL %s: %r not seen" % (label, needle))
    return False

ok = True

# ── Flow A: boot ───────────────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# ── Flow B: prompt triggers a ruled tool call -> auto-allow, no dialog ────
os.write(fd, b"check repo status")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("auto-allow-notice", "Allowed bash (rule 1)")

# ── Flow C: verdict reached the stub daemon; the turn resumed and froze ───
deadline = time.time() + 10
while time.time() < deadline and len(RESOLUTIONS) == 0:
    pump(0.1)
wire_ok = RESOLUTIONS == [{"call_id": "call-17", "granted": True}]
print(("PASS" if wire_ok else "FAIL") + " wire-resolution " + json.dumps(RESOLUTIONS))
ok &= wire_ok
ok &= expect("post-grant-answer", "Done.")

# Cumulative buffer: if the dialog EVER rendered, its text would remain
# in buf even after Ink overwrote the screen.
never_shown = "Permission required" not in clean(buf)
print(("PASS" if never_shown else "FAIL") + " dialog-never-shown")
ok &= never_shown

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
try:
    os.remove(CONFIG_FILE)
except OSError:
    pass

sys.exit(0 if ok else 1)
```

Run it:

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
rm -f /tmp/brain-inc17-smoke.sock
python3 scripts/ptySmokeInc17.py
echo "exit:$?"
```

Expected: all six checks PASS, `exit:0`.

- [ ] **Step 3: Regression smokes**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
python3 scripts/ptySmokeInc15.py; echo "inc15:$?"
python3 scripts/ptySmokeInc6.py; echo "inc6:$?"
```

Expected: inc15 `exit:0` (all-PASS; its replay assertions unaffected — no permission frames in that script), inc6 `exit:0` (all 16 assertions). Then restore capture drift:

```bash
git checkout -- packages/brain-shell/src/test/fixtures/
git status --porcelain packages/brain-shell/src/test/fixtures/
```

Expected: empty output from the final status call.

- [ ] **Step 4: tsc touched-file parity vs pristine origin/main**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
bunx tsc --noEmit > "$CLAUDE_JOB_DIR/tmp/inc17-tsc.log" 2>&1; echo "exit:$?"
for f in permissionRules sessionController; do
  echo "== $f =="
  sed $'s/\x1b\[[0-9;]*m//g' "$CLAUDE_JOB_DIR/tmp/inc17-tsc.log" \
    | grep -E "^src/(state/$f)\.tsx?" \
    | grep -oE "error TS[0-9]+" | sort | uniq -c
done
```

Then prove parity with a throwaway detached worktree (symlinked node_modules, cleaned afterwards):

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git worktree add --detach "$CLAUDE_JOB_DIR/tmp/inc17-probe" origin/main
ln -s /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/node_modules \
  "$CLAUDE_JOB_DIR/tmp/inc17-probe/packages/brain-shell/node_modules"
cd "$CLAUDE_JOB_DIR/tmp/inc17-probe/packages/brain-shell"
bunx tsc --noEmit > "$CLAUDE_JOB_DIR/tmp/inc17-tsc-main.log" 2>&1
for f in permissionRules sessionController; do
  echo "== $f =="
  sed $'s/\x1b\[[0-9;]*m//g' "$CLAUDE_JOB_DIR/tmp/inc17-tsc-main.log" \
    | grep -E "^src/(state/$f)\.tsx?" \
    | grep -oE "error TS[0-9]+" | sort | uniq -c
done
cd /Users/ritikpathania/Developer/PyCharm/brain
rm "$CLAUDE_JOB_DIR/tmp/inc17-probe/packages/brain-shell/node_modules"
git worktree remove --force "$CLAUDE_JOB_DIR/tmp/inc17-probe"
git worktree prune
```

Expected: `permissionRules.ts` appears in NEITHER log (new clean file); `sessionController.ts` shows identical counts both sides (pristine main documents none for it beyond ambient classes seen on every state file).

- [ ] **Step 5: Vendor scan on the increment diff**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
BASE=$(git merge-base HEAD origin/main)
git diff "$BASE"..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"
```

Expected: `0`. Nonzero → inspect line-by-line; docs paths are excluded by design.

- [ ] **Step 6: Cargo workspace (Rust untouched, prove it anyway)**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test --workspace --no-fail-fast' 2>&1 | grep -E "test result: FAILED|failures:" -A2 | grep -vE "^--" | sort | uniq -c
```

Expected: exactly ONE failed suite — the sole permitted `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.

- [ ] **Step 7: Finishing**

Announce finishing-a-development-branch, verify tests (Steps 1–6 ARE the verification), detect environment (normal repo — standard menu), base branch `main`, and present exactly:

```
Implementation complete. What would you like to do?

1. Merge back to main locally
2. Push and create a Pull Request
3. Keep the branch as-is (I'll handle it later)

Which option?
```

On Option 1: `git checkout main && git pull --ff-only && git merge feature/brain-shell-inc17-always-allow-rules`, confirm `git rev-parse main` equals the branch tip hash, rerun the bun suite once as post-merge sanity, delete the branch with `git branch -d feature/brain-shell-inc17-always-allow-rules`, report `[ahead N]` state. Pushes require explicit user approval.

---

## Self-Review (completed during planning)

1. **Spec coverage:** §2 rule model/store/formatters → Task 1 (schema, `primaryInputString`, matcher, tolerant store ops, fresh-read semantics, `configPath()` reuse, summarizer refactor); §3 controller decision point + wire-failure fallback + `resolvePermissionAlways` + stale-docblock rewrite → Task 2; §3 dialog (keymap `a`, relative clamped arrows, commit dispatch, view row/help line, AppShell wiring) → Task 3; §4 command (registry entry, arg-splitting dispatch, listing/removal/usage copy via `runPermissionsCommand`) → Task 4; §5 testing strategy (unit truth tables, store round-trips incl. theme-key survival, controller auto/manual/fallback/always cases, dialog extensions, six-check wire-level smoke) → Tasks 1–5; §5 standard gates + §7 rider commit → Task 0 and Task 5 Steps 1–6. No gaps.
2. **Placeholder scan:** every code step carries complete code; every run step carries the exact command and expected output. The one conditional instruction (Task 5 Step 1) is a deliberate stop-and-surface gate, not an unpinned edit. Task 4 Step 3(b)'s `...existing...` marker denotes literally untouched adjacent lines shown for anchoring — the inserted arm is given in full.
3. **Type consistency:** `AllowRule` field names identical across store/matcher/tests/smoke config; `dialogDecision` return union (`'always'`, `move.index: 0|1|2`) matches the AppShell handler arms and the widened `permSelected`; `runPermissionsCommand(args: readonly string[])` matches both the Task 1 tests and the Task 4 dispatch; controller method names (`resolvePermissionAlways`, private `autoAllow`) consistent between Task 2 implementation and Task 3 consumption; smoke constants (`call-17`, `stub-s17`) used consistently between frames and the wire-resolution assertion.
4. **Known intentional deltas, surfaced honestly:** (a) dialog arrows change from absolute-jump to relative-clamped — exactly one legacy assertion (`right(1) → index 1`) is replaced, documented in Task 3 Step 1; (b) removal-notice wording uses the shared `describeRule` formatter (`Removed rule 1 (bash — commands starting with "git ").`) where the spec's sketch abbreviated to `(bash "git ")` — the spec itself mandates the shared formatter, so this follows the spec over its illustrative example; (c) expected suite totals are derived arithmetically (280 baseline + 19 new `it()`s = 299 / 294 pass / 5 documented fails) and Step 1 treats any additional failure as a stop condition.
