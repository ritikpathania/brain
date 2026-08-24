# Brain Shell Increment 3 — Session Frame Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the shell its session frame — a welcome screen, a `/resume` session picker, a status line footer, a `/theme` picker over the four palettes plus auto (with persistence), and a permission dialog driven by `tool_permission_requested` stream frames.

**Architecture:** Every feature decomposes into a pure core (decision tables, replay mapper, store) plus a thin interactive shell mounted in `AppShell` behind an `isActive`-gated `useBoundInput` registration per overlay. Overlays share one `'overlay'` (lists) / `'dialog'` (allow-deny) keybinding context added to `DEFAULT_BINDINGS`. The composer pauses while any overlay is open so esc/arrows/enter never leak into editing or turn control. Theme changes ride the existing `ThemeProvider.setSetting` (live preview = just call it); persistence is a new original `themeStore` over `~/.brain/config.json`.

**Tech Stack:** Bun 1.4 + React 19 + stock Ink 7 via `src/compat/index.js`; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-08-23-brain-shell-contracts-first-design.md` §5 row 3 ("Session frame: welcome/logo, resume picker, status line, themes (+daltonized), permission dialogs"), §7 gap table rows (lines 77–81), §8 testing strategy.

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

Constraint 4 ruling used throughout this plan (from spec line 101 "adapter/ unchanged"): **no file under `src/adapter/` is modified in this increment.** Permission frames are normalized in the shell-owned client layer (`src/client/`) and intercepted live in the controller; the turn-event vocabulary in `BrainTurnEvents.ts` already names these events and stays exactly as-is.

## Reference behavior contract (archaeology notes, not code to copy)

Extracted from the read-only reference tree:

- Welcome banner: wordmark + one-line identity + a short hint block; disappears once the conversation owns the screen.
- Theme picker: arrow-navigable list including a system/auto entry; navigating previews immediately; esc restores the previous theme without restart; enter commits and persists.
- Resume picker: reverse-chronological sessions, relative timestamps ("2h ago"), pinned surfaced first; selecting resumes without losing the prompt.
- Permission dialog: tool name + summarized input, Allow/Deny options, keyboard-first (y/n shortcuts, arrows + enter, esc denies).
- Status line: single dim footer line of context + keybind hints.

## Recon findings (verified on `main` @ `31db121`, 2026-08-24)

- All four palettes exist in `src/state/palettes.ts`; `src/state/themeContext.tsx` provides `ThemeProvider({setting, children})`, `useTheme()` (tokens + unresolved `setting` + `themeName`), `useThemeSetting()`, and imperative `setSetting` — designed for this increment, but **nothing mounts the provider yet** (`main.tsx` renders `AppShell` bare).
- `src/contracts/theme.ts`: `ThemeSetting = ThemeName | 'auto'`; `resolveThemeSetting('auto')` → `getSystemThemeName()` which reads preload's `__BRAIN_SYSTEM_THEME` (COLORFGBG heuristic), then `BRAIN_THEME` env, then `'dark'`.
- Client seam: `listSessions(): Promise<BrainSessionSummary[]>` (UDS impl proxies `session/list` RPC, tolerant field parsing) and `loadSession(id): Promise<{session: BrainSession}>` (`v1/session/load`). `BrainSession.messages: BrainMessage[]` with `{id, role: 'user'|'assistant'|'system', content}` — enough to replay transcripts.
- Permission vocabulary exists but is dead code end-to-end: `BrainTurnEvents.ts` declares `tool_permission_requested`/`tool_permission_resolved`; `BrainStreamChunk`'s union has no permission member; the UDS stream parser drops unknown raw frame types; nothing sets `status: 'permission_required'` on live tool cards. The daemon never emits these frames yet — the shell adds *tolerant reception* (additive, IPC-preserving); the resolution round-trip lands with daemon support later.
- Untracked salvage file `src/adapter/BrainConfigStore.ts` (never committed): read for reference, **left untracked** — this plan ships an original, much smaller `themeStore` instead.
- `ToolCardData` carries `callId`, so a denial can be matched to its tool card precisely.
- `src/ui/shell/BrainMark.tsx` carries the comment "Replaced by the full welcome frame in Inc 3." Its pure views stay (tested by `src/test/contracts/shell.test.tsx`); only the AppShell mount changes.
- Dirty-baseline hazard (gotcha #5/#6 follow-ups): `src/client/BrainBackendClient.ts` (+518 lines) and `src/client/UdsBrainBackendClient.ts` (+616/-20) hold uncommitted salvage-era expansion sitting in the worktree — tests/build run against it. Both diffs were vendor-scanned: **0 hits** for `claude|anthropic|vendor` on added lines. Task 0 baselines exactly these two files in their own commit so every later feature diff stays reviewable and no later commit silently smuggles a thousand unreviewed lines.

## Scope decisions (deferrals)

- **No permission round-trip on the wire.** Granting/denying resolves local UX state only (notice + tool-card status). The daemon cannot receive resolutions today, and inventing a frame would violate constraint 4. Documented in the dialog task.
- **Status line is one dim line**, not a segmented powerline bar.
- **Resume replays text rows only** (`user`/`assistant`/`system` messages); tool cards are not part of `BrainMessage` and are not synthesized.
- **`editorMode`/vim mode** in old configs is ignored by the new store (preserved byte-for-byte in the file on write, just never read here).
- PTY fixtures under `src/test/fixtures/pty/inc1` and `inc2` show as modified after every smoke re-run (smoke scripts rewrite snapshots). They are **left dirty**; only `inc3` fixtures get committed.

## Increment 2 baseline (current `main` @ `31db121`)

- `bun test`: **179 pass / 5 fail** — the 5 documented baseline failures (`visualCellParity` ×2, `sessionSemanticIntegration`, `brainMemoryIntegration`, `brainTurnTransformer`). Zero NEW failures may be introduced; record final numbers.
- Bundle gate: `bun build src/main.tsx --outdir dist --target bun` → BUILD_OK.
- Smoke gate: `python3 scripts/ptySmokeInc3.py` → all PASS, exit 0 (new this increment; `ptySmokeInc2.py` remains runnable but its launch fixture expectations about the always-visible mark are superseded by the welcome frame).

## Toolchain gotchas (from memory + Inc 1/2 experience)

- cwd drifts between Bash calls: always prefix `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && …` (or repo root for git).
- macOS APFS case-folds imports: never keep two files in one dir whose names differ only by case.
- Ink parses ONE stdin chunk as ONE keypress: PTY harnesses write text and Enter separately, pump ≥0.3 s between distinct keystrokes. Multi-char text as ONE chunk inserts as paste (fine).
- PTY needs `fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))` before exec; strip ANSI before matching rendered text.
- Repo test pattern: invoke PURE view functions directly with explicit `tokens: BrainTokens` (never mount reconcilers/hooked wrappers); live rendering verified only via PTY smoke.
- Vendor grep must be diff-scoped (gotcha #6): `git diff <base>..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'`.
- `git ls-files <path>` before creating any file (gotcha #5) — done during recon; all Create paths below returned 0 tracked files.
- `error: daemon terminated` noise from git/bash is harmless.

---

### Task 0: Baseline the dirty client layer

The two client files carry large uncommitted salvage-era expansions that Inc 3 must build on and modify. Committing their current worktree state as a discrete chore commit keeps every subsequent diff reviewable and keeps later commits honest (staging the whole file stages only what was baselined here). Worktree bytes do not change, so all gates behave identically before and after.

**Files:**
- Commit-only: `packages/brain-shell/src/client/BrainBackendClient.ts`, `packages/brain-shell/src/client/UdsBrainBackendClient.ts`

**Interfaces:**
- Produces: clean tracked state for `BrainStreamChunk` (Task 7 extends its union) and the UDS stream parser (Task 7 adds a normalization branch).

- [ ] **Step 1: Confirm the dirty state is exactly as recon described**

Run (repo root):
```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git status --porcelain -- packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/client/UdsBrainBackendClient.ts
```
Expected: both lines start with ` M`.

- [ ] **Step 2: Re-run the vendor scan on the added lines**

```bash
git diff -- packages/brain-shell/src/client/ | grep '^+' | grep -icE 'claude|anthropic|vendor'
```
Expected: `0`. If nonzero, STOP and surface the hits before committing.

- [ ] **Step 3: Suite sanity (unchanged tree)**

Run: `cd packages/brain-shell && bun test 2>&1 | tail -5`
Expected: 179 pass / 5 fail (the documented five).

- [ ] **Step 4: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/client/UdsBrainBackendClient.ts
git commit -m "chore(brain-shell): baseline pre-existing working-tree state of client layer

These two files carried large uncommitted salvage-era expansions (mock
client surface, UDS session RPC implementations) that tests and builds
already run against. Baselining them before Increment 3 keeps the
increment's diffs reviewable and its commits free of unrelated content.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 1: Welcome frame

**Files:**
- Create: `packages/brain-shell/src/ui/shell/WelcomeFrame.tsx`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (import block; replace `<BrainMark />` mount)
- Test: `packages/brain-shell/src/test/ui/welcomeFrame.test.tsx`

**Interfaces:**
- Consumes: `useTheme()` from `../../state/themeContext.js` (hooked wrapper only), `BrainTokens` from `../../state/palettes.js`, compat `Box`/`Text`.
- Produces: `WelcomeFrameView(props: {tokens: BrainTokens; workspace: string}): React.ReactElement` (pure, unit-testable) and `WelcomeFrame(): React.ReactElement` (hooked wrapper computing workspace from cwd).

- [ ] **Step 1: Write the failing test**

Create `packages/brain-shell/src/test/ui/welcomeFrame.test.tsx`:
```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../state/palettes.js';
import { WelcomeFrameView } from '../../ui/shell/WelcomeFrame.js';

function textOf(el: React.ReactElement): string {
  // Flatten ink Text/Box trees into plain text for assertion.
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

describe('WelcomeFrameView', () => {
  test('carries the wordmark, identity line, workspace, and hints', () => {
    const text = textOf(WelcomeFrameView({ tokens: PALETTES.dark, workspace: 'brain' }));
    expect(text).toContain('◆ BRAIN');
    expect(text).toContain('memory-first agent workspace');
    expect(text).toContain('workspace brain');
    expect(text).toContain('/help commands');
    expect(text).toContain('/resume sessions');
    expect(text).toContain('/theme appearance');
  });

  test('renders nothing proprietary', () => {
    const text = textOf(WelcomeFrameView({ tokens: PALETTES.dark, workspace: 'x' }));
    expect(text.toLowerCase()).not.toContain('claude');
    expect(text.toLowerCase()).not.toContain('anthropic');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/welcomeFrame.test.tsx`
Expected: FAIL — cannot resolve `../../ui/shell/WelcomeFrame.js`.

- [ ] **Step 3: Write the implementation**

Create `packages/brain-shell/src/ui/shell/WelcomeFrame.tsx`:
```tsx
/**
 * Launch-screen frame: wordmark, identity, workspace, and hint block.
 * Shown only while the transcript is empty; the conversation owns the
 * screen afterwards (replaces the Inc 0 BrainMark mount, per its comment).
 */
import * as React from 'react';
import * as path from 'path';
import { Box, Text, useTheme } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';

export function WelcomeFrameView(props: {
  tokens: BrainTokens;
  workspace: string;
}): React.ReactElement {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text>
        <Text bold color={props.tokens.brand}>◆ BRAIN</Text>
        <Text dimColor> memory-first agent workspace</Text>
      </Text>
      <Box marginTop={1} flexDirection="column">
        <Text dimColor>  workspace {props.workspace}</Text>
        <Text dimColor>  /help commands · ! bash · /resume sessions · /theme appearance</Text>
      </Box>
    </Box>
  );
}

/** Hooked wrapper: theme tokens + cwd basename as the workspace label. */
export function WelcomeFrame(): React.ReactElement {
  const { tokens } = useTheme();
  const workspace = path.basename(process.cwd()).slice(0, 24);
  return <WelcomeFrameView tokens={tokens} workspace={workspace} />;
}
```

Modify `src/ui/shell/AppShell.tsx` — replace the import (line 3):
```tsx
import { WelcomeFrame } from './WelcomeFrame.js';
```
and replace the mount (line 76):
```tsx
      {snapshot.rows.length === 0 && !snapshot.busy ? <WelcomeFrame /> : null}
```
Leave `BrainMark.tsx` itself untouched (still exercised by `src/test/contracts/shell.test.tsx`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/ui/welcomeFrame.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Full-suite regression check**

Run: `bun test 2>&1 | tail -5`
Expected: ≥181 pass / same 5 baseline failures. In particular `src/test/contracts/shell.test.tsx` must stay green (it tests `BrainMark` views, which are untouched).

- [ ] **Step 6: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/shell/WelcomeFrame.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/ui/welcomeFrame.test.tsx
git commit -m "feat(brain-shell): welcome frame replaces bare launch mark

Wordmark + identity + workspace + hint block, shown only while the
transcript is empty. BrainMark pure views remain for their contract
tests; only the AppShell mount changes.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Theme store (persistence)

Original module — deliberately NOT the untracked salvage `BrainConfigStore.ts` (which stays untracked). Reads/writes only the `theme` key of `~/.brain/config.json` (or `$BRAIN_CONFIG_PATH`), preserving all other keys, tolerating legacy values.

**Files:**
- Create: `packages/brain-shell/src/state/themeStore.ts`
- Test: `packages/brain-shell/src/test/state/themeStore.test.ts`

**Interfaces:**
- Produces: `readThemeSetting(): ThemeSetting` and `writeThemeSetting(setting: ThemeSetting): void`, plus `configPath(): string`. Task 3 consumes `readThemeSetting`; Task 4 consumes `writeThemeSetting`.

- [ ] **Step 1: Write the failing test**

Create `packages/brain-shell/src/test/state/themeStore.test.ts`:
```ts
import { afterEach, describe, expect, test } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { configPath, readThemeSetting, writeThemeSetting } from '../../state/themeStore.js';

let tmpDir: string;

function useTmpConfig(): string {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-theme-store-'));
  const p = path.join(tmpDir, 'config.json');
  process.env.BRAIN_CONFIG_PATH = p;
  return p;
}

afterEach(() => {
  delete process.env.BRAIN_CONFIG_PATH;
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('themeStore', () => {
  test('missing file resolves to auto', () => {
    useTmpConfig();
    expect(readThemeSetting()).toBe('auto');
  });

  test('reads a valid setting and preserves foreign keys on write', () => {
    const p = useTmpConfig();
    fs.writeFileSync(p, JSON.stringify({ theme: 'light', editorMode: 'vim', nested: { a: 1 } }));
    expect(readThemeSetting()).toBe('light');
    writeThemeSetting('light-daltonized');
    const doc = JSON.parse(fs.readFileSync(p, 'utf8'));
    expect(doc.theme).toBe('light-daltonized');
    expect(doc.editorMode).toBe('vim'); // preserved
    expect(doc.nested).toEqual({ a: 1 }); // preserved
  });

  test('legacy dark-ansi/light-ansi aliases map onto modern themes', () => {
    const p = useTmpConfig();
    fs.writeFileSync(p, JSON.stringify({ theme: 'dark-ansi' }));
    expect(readThemeSetting()).toBe('dark');
    fs.writeFileSync(p, JSON.stringify({ theme: 'light-ansi' }));
    expect(readThemeSetting()).toBe('light');
  });

  test('invalid JSON and unknown values fall back to auto without throwing', () => {
    const p = useTmpConfig();
    fs.writeFileSync(p, '{not json');
    expect(readThemeSetting()).toBe('auto');
    fs.writeFileSync(p, JSON.stringify({ theme: 'neon-purple' }));
    expect(readThemeSetting()).toBe('auto');
  });

  test('write creates parent directories and round-trips', () => {
    const p = useTmpConfig();
    fs.rmSync(path.dirname(p), { recursive: true, force: true });
    writeThemeSetting('dark');
    expect(readThemeSetting()).toBe('dark');
    expect(configPath()).toBe(p);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/state/themeStore.test.ts`
Expected: FAIL — cannot resolve `../../state/themeStore.js`.

- [ ] **Step 3: Write the implementation**

Create `packages/brain-shell/src/state/themeStore.ts`:
```ts
/**
 * Theme persistence: the `theme` key of the user's brain config file.
 * Original, minimal surface — read/merge-write only, tolerant of missing
 * files, bad JSON, and legacy values. Other keys pass through untouched.
 */
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import type { ThemeSetting } from '../contracts/theme.js';

const VALID: readonly string[] = [
  'auto',
  'dark',
  'light',
  'dark-daltonized',
  'light-daltonized',
];

/** Legacy values kept readable so old configs still resolve. */
const LEGACY_ALIASES: Record<string, ThemeSetting> = {
  'dark-ansi': 'dark',
  'light-ansi': 'light',
};

export function configPath(): string {
  if (process.env.BRAIN_CONFIG_PATH) return path.resolve(process.env.BRAIN_CONFIG_PATH);
  return path.join(os.homedir(), '.brain', 'config.json');
}

export function readThemeSetting(): ThemeSetting {
  try {
    const parsed = JSON.parse(fs.readFileSync(configPath(), 'utf8')) as { theme?: unknown };
    const t = parsed && typeof parsed === 'object' ? parsed.theme : undefined;
    if (typeof t === 'string' && VALID.includes(t)) return t as ThemeSetting;
    if (typeof t === 'string' && LEGACY_ALIASES[t] !== undefined) return LEGACY_ALIASES[t]!;
  } catch {
    // missing file / bad JSON → default below
  }
  return 'auto';
}

export function writeThemeSetting(setting: ThemeSetting): void {
  let doc: Record<string, unknown> = {};
  try {
    const parsed = JSON.parse(fs.readFileSync(configPath(), 'utf8')) as unknown;
    if (parsed && typeof parsed === 'object') doc = parsed as Record<string, unknown>;
  } catch {
    // start a fresh document
  }
  doc.theme = setting;
  fs.mkdirSync(path.dirname(configPath()), { recursive: true });
  fs.writeFileSync(configPath(), JSON.stringify(doc, null, 2) + '\n');
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/test/state/themeStore.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/state/themeStore.ts packages/brain-shell/src/test/state/themeStore.test.ts
git commit -m "feat(brain-shell): theme settings store over the brain config file

Read/merge-write of the theme key with legacy alias support and
pass-through preservation of all other config keys.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Mount ThemeProvider + overlay/dialog keybinding contexts

**Files:**
- Modify: `packages/brain-shell/src/main.tsx` (wrap AppShell in ThemeProvider)
- Modify: `packages/brain-shell/src/keybindings/resolve.ts` (extend `DEFAULT_BINDINGS`)
- Test: `packages/brain-shell/src/test/keybindings/resolve.test.ts` (append)

**Interfaces:**
- Consumes: `readThemeSetting()` (Task 2), `ThemeProvider` from `state/themeContext.js`, existing `BindingRule`/`DEFAULT_BINDINGS`.
- Produces: binding actions consumed by later tasks — `overlay:up` / `overlay:down` / `overlay:commit` / `overlay:cancel` (context `'overlay'`) and `dialog:left` / `dialog:right` / `dialog:allow` / `dialog:deny` / `dialog:commit` / `dialog:cancel` (context `'dialog'`). Context names `'overlay'` and `'dialog'` join `KeybindingContextName`.

- [ ] **Step 1: Write the failing tests**

Append to `src/test/keybindings/resolve.test.ts` (inside the outermost `describe`, after existing tests — reuse that file's existing imports; it already imports `resolveAction`, `strokeToKey`, `DEFAULT_BINDINGS`, `decide`-style helpers if present; otherwise assert with `resolveAction(DEFAULT_BINDINGS, ['overlay'], strokeToKey('…', info))` directly):
```ts
describe('overlay + dialog contexts (Inc 3)', () => {
  test('overlay context binds arrows/enter/esc', () => {
    expect(resolveAction(DEFAULT_BINDINGS, ['overlay'], strokeToKey('', { upArrow: true }))).toBe('overlay:up');
    expect(resolveAction(DEFAULT_BINDINGS, ['overlay'], strokeToKey('', { downArrow: true }))).toBe('overlay:down');
    expect(resolveAction(DEFAULT_BINDINGS, ['overlay'], strokeToKey('', { return: true }))).toBe('overlay:commit');
    expect(resolveAction(DEFAULT_BINDINGS, ['overlay'], strokeToKey('', { escape: true }))).toBe('overlay:cancel');
  });

  test('dialog context binds left/right/y/n/enter/esc', () => {
    expect(resolveAction(DEFAULT_BINDINGS, ['dialog'], strokeToKey('', { leftArrow: true }))).toBe('dialog:left');
    expect(resolveAction(DEFAULT_BINDINGS, ['dialog'], strokeToKey('', { rightArrow: true }))).toBe('dialog:right');
    expect(resolveAction(DEFAULT_BINDINGS, ['dialog'], strokeToKey('y', {}))).toBe('dialog:allow');
    expect(resolveAction(DEFAULT_BINDINGS, ['dialog'], strokeToKey('n', {}))).toBe('dialog:deny');
    expect(resolveAction(DEFAULT_BINDINGS, ['dialog'], strokeToKey('', { return: true }))).toBe('dialog:commit');
    expect(resolveAction(DEFAULT_BINDINGS, ['dialog'], strokeToKey('', { escape: true }))).toBe('dialog:cancel');
  });

  test('global fallback still resolves under overlay context', () => {
    expect(resolveAction(DEFAULT_BINDINGS, ['overlay'], strokeToKey('c', { ctrl: true }))).toBe('shell:exit');
  });
});
```
Note for the implementer: if the existing test file wraps assertions in a local helper (e.g. `decide(...)`), prefer that helper's style; the three behaviors above are the contract, the spelling may follow the file's idiom. If `KeybindingContextName` is a closed union, extend it in Step 3 — TypeScript will flag the mismatch at compile time if forgotten.

- [ ] **Step 2: Run tests to verify they fail**

Run: `bun test src/test/keybindings/resolve.test.ts`
Expected: FAIL — the three new tests come back undefined/null actions (or compile error on `'overlay'` not assignable to `KeybindingContextName`).

- [ ] **Step 3: Implement**

In `src/keybindings/resolve.ts`:
1. Extend the context union:
```ts
export type KeybindingContextName = 'global' | 'composer' | 'palette' | 'overlay' | 'dialog';
```
2. Append to `DEFAULT_BINDINGS`:
```ts
  // Overlay lists (theme picker, resume picker): arrow-navigate, enter picks, esc closes.
  { action: 'overlay:up', context: 'overlay', key: 'up' },
  { action: 'overlay:down', context: 'overlay', key: 'down' },
  { action: 'overlay:commit', context: 'overlay', key: 'return' },
  { action: 'overlay:cancel', context: 'overlay', key: 'escape' },
  // Permission dialog: left/right choose, y allow, n deny, enter confirms, esc denies.
  { action: 'dialog:left', context: 'dialog', key: 'left' },
  { action: 'dialog:right', context: 'dialog', key: 'right' },
  { action: 'dialog:allow', context: 'dialog', key: 'y' },
  { action: 'dialog:deny', context: 'dialog', key: 'n' },
  { action: 'dialog:commit', context: 'dialog', key: 'return' },
  { action: 'dialog:cancel', context: 'dialog', key: 'escape' },
```
(`strokeToKey` already normalizes literal chars, so `'y'`/`'n'` strokes need no translator change.)

In `src/main.tsx` — wrap the app (imports added at top, below the existing ones):
```tsx
import { ThemeProvider } from './state/themeContext.js';
import { readThemeSetting } from './state/themeStore.js';
```
and replace the render line:
```tsx
  const app = render(
    React.createElement(
      ThemeProvider,
      { setting: readThemeSetting() },
      React.createElement(AppShell),
    ),
    { patchConsole: false },
  );
```
Auto detection rides the existing chain: `resolveThemeSetting('auto')` → `getSystemThemeName()` → preload's `__BRAIN_SYSTEM_THEME` / `BRAIN_THEME` / `'dark'`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun test src/test/keybindings/resolve.test.ts`
Expected: PASS including the three new tests.

- [ ] **Step 5: Build gate**

Run: `bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK`
Expected: `BUILD_OK` (main.tsx changed, so prove it still bundles).

- [ ] **Step 6: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/main.tsx packages/brain-shell/src/keybindings/resolve.ts packages/brain-shell/src/test/keybindings/resolve.test.ts
git commit -m "feat(brain-shell): mount ThemeProvider at launch; overlay/dialog binding contexts

The provider was built in Inc 0 but never mounted; seeding it from the
theme store makes palette selection live without restarts. The new
contexts give overlays and the permission dialog first-class bindings.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Shared overlay decision core + theme picker

**Files:**
- Create: `packages/brain-shell/src/ui/overlays/overlayLogic.ts`
- Create: `packages/brain-shell/src/ui/overlays/ThemePicker.tsx`
- Modify: `packages/brain-shell/src/commands/matcher.ts` (add `theme` command)
- Modify: `packages/brain-shell/src/ui/composer/PromptInput.tsx` (add `paused` prop)
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (picker state, `/theme` handling, overlay hook, render)
- Test: `packages/brain-shell/src/test/ui/overlays/overlayLogic.test.ts`
- Test: `packages/brain-shell/src/test/ui/overlays/themePickerView.test.tsx`

**Interfaces:**
- Consumes: `overlay:*` actions (Task 3), `useBoundInput`, `THEME_NAMES`/types from `contracts/theme.js`, `useTheme`/`setSetting` via `state/themeContext.js`, `writeThemeSetting` (Task 2), `COMMANDS` (matcher).
- Produces:
  - `overlayListDecision(action: string | null, selected: number, count: number): OverlayListDecision` where `type OverlayListDecision = { type: 'move'; index: number } | { type: 'commit'; index: number } | { type: 'cancel' } | { type: 'passthrough' }` — shared by theme AND resume pickers.
  - `THEME_CHOICES: readonly { setting: ThemeSetting; label: string }[]` (auto first, then the four themes).
  - `ThemePickerView(props: { choices: readonly {setting; label}[]; selectedIndex: number; current: ThemeSetting; tokens: BrainTokens })` (pure) and `ThemePicker` hooked wrapper.
  - `PromptInput` accepts `paused?: boolean` — while true, its internal `useInput` is inactive.

- [ ] **Step 1: Write the failing tests**

Create `src/test/ui/overlays/overlayLogic.test.ts`:
```ts
import { describe, expect, test } from 'bun:test';
import { overlayListDecision } from '../../../ui/overlays/overlayLogic.js';

describe('overlayListDecision', () => {
  test('closed or empty lists pass everything through', () => {
    expect(overlayListDecision('overlay:down', 0, 0)).toEqual({ type: 'passthrough' });
    expect(overlayListDecision(null, 0, 5)).toEqual({ type: 'passthrough' });
  });

  test('arrows clamp within bounds', () => {
    expect(overlayListDecision('overlay:up', 0, 3)).toEqual({ type: 'move', index: 0 });
    expect(overlayListDecision('overlay:up', 2, 3)).toEqual({ type: 'move', index: 1 });
    expect(overlayListDecision('overlay:down', 2, 3)).toEqual({ type: 'move', index: 2 });
    expect(overlayListDecision('overlay:down', 0, 3)).toEqual({ type: 'move', index: 1 });
  });

  test('commit carries the selected index and cancel cancels', () => {
    expect(overlayListDecision('overlay:commit', 1, 3)).toEqual({ type: 'commit', index: 1 });
    expect(overlayListDecision('overlay:cancel', 1, 3)).toEqual({ type: 'cancel' });
  });

  test('unrelated actions pass through', () => {
    expect(overlayListDecision('dialog:allow', 0, 2)).toEqual({ type: 'passthrough' });
  });
});
```

Create `src/test/ui/overlays/themePickerView.test.tsx`:
```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { THEME_CHOICES, ThemePickerView } from '../../../ui/overlays/ThemePicker.js';

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

describe('ThemePickerView', () => {
  test('lists all five settings with the selection marker and current check', () => {
    const text = textOf(
      ThemePickerView({ choices: THEME_CHOICES, selectedIndex: 2, current: 'light', tokens: PALETTES.dark }),
    );
    expect(text).toContain('Theme');
    expect(text).toContain('❯ Light');
    expect(text).not.toContain('❯ Auto');
    expect(text).toContain('✓ light');
    for (const label of ['Auto (detect terminal)', 'Dark', 'Light', 'Dark (daltonized)', 'Light (daltonized)']) {
      expect(text).toContain(label);
    }
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bun test src/test/ui/overlays/`
Expected: FAIL — modules don't exist.

- [ ] **Step 3: Implement the pure cores and view**

Create `src/ui/overlays/overlayLogic.ts`:
```ts
/**
 * Shared decision table for arrow-navigable overlay lists (theme picker,
 * resume picker). Actions arrive as namespaced ids from the keybinding
 * framework ('overlay:*'); indexes clamp, never wrap.
 */
export type OverlayListDecision =
  | { type: 'move'; index: number }
  | { type: 'commit'; index: number }
  | { type: 'cancel' }
  | { type: 'passthrough' };

export function overlayListDecision(
  action: string | null,
  selected: number,
  count: number,
): OverlayListDecision {
  if (action === null || count === 0) return { type: 'passthrough' };
  switch (action) {
    case 'overlay:up':
      return { type: 'move', index: Math.max(0, selected - 1) };
    case 'overlay:down':
      return { type: 'move', index: Math.min(count - 1, selected + 1) };
    case 'overlay:commit':
      return { type: 'commit', index: Math.min(selected, count - 1) };
    case 'overlay:cancel':
      return { type: 'cancel' };
    default:
      return { type: 'passthrough' };
  }
}
```

Create `src/ui/overlays/ThemePicker.tsx`:
```tsx
/**
 * /theme overlay: five settings (auto + four palettes). Navigation calls
 * setSetting live — that IS the preview; esc rolls back, enter persists
 * via the theme store. Rounded border per TUI rules.
 */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ThemeSetting } from '../../contracts/theme.js';

export interface ThemeChoice {
  setting: ThemeSetting;
  label: string;
}

export const THEME_CHOICES: readonly ThemeChoice[] = [
  { setting: 'auto', label: 'Auto (detect terminal)' },
  { setting: 'dark', label: 'Dark' },
  { setting: 'light', label: 'Light' },
  { setting: 'dark-daltonized', label: 'Dark (daltonized)' },
  { setting: 'light-daltonized', label: 'Light (daltonized)' },
];

export function ThemePickerView(props: {
  choices: readonly ThemeChoice[];
  selectedIndex: number;
  current: ThemeSetting;
  tokens: BrainTokens;
}): React.ReactElement {
  const sel = Math.min(props.selectedIndex, Math.max(0, props.choices.length - 1));
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.promptBorder} paddingX={1}>
      <Text bold>Theme</Text>
      {props.choices.map((c, i) => (
        <Text key={c.setting} inverse={i === sel}>
          {i === sel ? '❯ ' : '  '}
          {c.label}
          {c.setting === props.current ? '  ✓' : ''}
        </Text>
      ))}
      <Text dimColor>↑↓ navigate (live preview) · enter apply · esc cancel</Text>
    </Box>
  );
}
```
(The hooked `ThemePicker` wrapper is intentionally omitted — AppShell owns state and passes everything to the pure view; adding a second wrapper would duplicate that state.)

Modify `src/commands/matcher.ts` — extend `COMMANDS` (keep alphabetical-ish grouping; description wording matters for the fuzzy scorer tests' uniqueness assumptions — none collide):
```ts
  { name: 'theme', description: 'Change the color theme' },
```

Modify `src/ui/composer/PromptInput.tsx`:
1. Add to props interface: `paused?: boolean;`
2. Locate its `useInput(handler…)` call and gate it:
```ts
  useInput(handler, { isActive: !(props.paused ?? false) });
```
(Keep whatever handler/options shape exists; the only change is the `isActive` option.)

- [ ] **Step 4: Wire AppShell**

In `src/ui/shell/AppShell.tsx`:

Imports to add:
```tsx
import { useTheme } from '../../compat/index.js';
import { overlayListDecision } from '../overlays/overlayLogic.js';
import { THEME_CHOICES, ThemePickerView } from '../overlays/ThemePicker.js';
import { writeThemeSetting } from '../../state/themeStore.js';
import type { ThemeSetting } from '../../contracts/theme.js';
```

Inside the component (after `expandTools` state). `useTheme()` returns `{ setting, themeName, tokens }` with `setSetting` attached to the same object (see `src/state/themeContext.tsx` — it `Object.assign`s the setter onto the context value), so destructure all four:
```tsx
  const { setting: themeSetting, tokens, setSetting } = useTheme();
  const [themeOpen, setThemeOpen] = React.useState(false);
  const [themeSelected, setThemeSelected] = React.useState(0);
  const [themeOriginal, setThemeOriginal] = React.useState<ThemeSetting>('auto');
```

A second `useBoundInput` registration (below the existing global one):
```tsx
  useBoundInput({
    contexts: ['overlay'],
    isActive: themeOpen,
    onAction: (action) => {
      const d = overlayListDecision(action, themeSelected, THEME_CHOICES.length);
      if (d.type === 'move') {
        setThemeSelected(d.index);
        setSetting(THEME_CHOICES[d.index]!.setting); // live preview
      } else if (d.type === 'commit') {
        setThemeOpen(false);
        try {
          writeThemeSetting(THEME_CHOICES[d.index]!.setting);
        } catch {
          controller.notice('Could not save the theme setting.');
        }
      } else if (d.type === 'cancel') {
        setSetting(themeOriginal); // rollback preview
        setThemeOpen(false);
      }
    },
  });
```
(No bridge needed — the destructured `setSetting` IS the provider's imperative setter. Do not modify `themeContext.tsx`.)

`runCommand` gains a case beside the others:
```tsx
    else if (chosen.name === 'theme') {
      setThemeOriginal(themeSetting);
      setThemeSelected(Math.max(0, THEME_CHOICES.findIndex((c) => c.setting === themeSetting)));
      setThemeOpen(true);
    }
```

Render: pass `paused` to the composer and draw the picker between transcript and spinner:
```tsx
        <PromptInput
          disabled={false}
          busy={snapshot.busy}
          paused={themeOpen}
          onSubmit={handleSubmit}
          onAbort={() => controller.abort()}
        />
```
and directly before the `<Box marginTop={1}>` composer wrapper:
```tsx
      {themeOpen ? (
        <Box marginTop={1}>
          <ThemePickerView
            choices={THEME_CHOICES}
            selectedIndex={themeSelected}
            current={themeSetting}
            tokens={tokens}
          />
        </Box>
      ) : null}
```
Note: `AppShell` previously had no `useTheme` call; the memo-controller, snapshot hook, and hook ORDER must stay stable — add the new hooks in the same place every render (they are unconditional).

- [ ] **Step 5: Run tests to verify they pass**

Run: `bun test src/test/ui/overlays/ src/test/commands/matcher.test.ts`
Expected: overlay + theme-picker suites PASS; matcher suite still PASS (new command extends the registry; the executor-lookup pin counts commands dynamically or pins specific ones — if a test hard-codes the command count, update that constant to include `theme` and note it in the commit body).

- [ ] **Step 6: Full suite + build**

Run: `bun test 2>&1 | tail -5` then `bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK`
Expected: no NEW failures vs baseline; BUILD_OK.

- [ ] **Step 7: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/overlays/overlayLogic.ts packages/brain-shell/src/ui/overlays/ThemePicker.tsx packages/brain-shell/src/test/ui/overlays/overlayLogic.test.ts packages/brain-shell/src/test/ui/overlays/themePickerView.test.tsx packages/brain-shell/src/commands/matcher.ts packages/brain-shell/src/ui/composer/PromptInput.tsx packages/brain-shell/src/ui/shell/AppShell.tsx
git commit -m "feat(brain-shell): /theme picker with live preview over four palettes + auto

Shared overlay decision core serves theme and resume pickers; navigating
previews instantly via ThemeProvider.setSetting, esc rolls back, enter
persists through the theme store. Composer pauses while overlays are up
so esc/enter never leak into the editor or turn control.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Status line

**Files:**
- Create: `packages/brain-shell/src/ui/shell/StatusBar.tsx`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (replace footer `<Text>` block)
- Test: `packages/brain-shell/src/test/ui/statusBar.test.tsx`

**Interfaces:**
- Consumes: `useMainLoopModel()` (existing), `useTheme()` (Task 4 wiring), `BrainTokens`.
- Produces: `StatusBarView(props: { model: string; workspace: string; theme: string; expandTools: boolean; tokens: BrainTokens }): React.ReactElement` (pure; no hooked wrapper — AppShell already holds all inputs).

- [ ] **Step 1: Write the failing test**

Create `src/test/ui/statusBar.test.tsx`:
```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../state/palettes.js';
import { StatusBarView } from '../../ui/shell/StatusBar.js';

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

describe('StatusBarView', () => {
  test('one dim line: workspace, model, theme, hints', () => {
    const text = textOf(
      StatusBarView({
        model: 'brain-default',
        workspace: 'brain',
        theme: 'auto',
        expandTools: false,
        tokens: PALETTES.dark,
      }),
    );
    expect(text).toContain('brain · model brain-default · theme auto');
    expect(text).toContain('! bash');
    expect(text).toContain('/ commands');
    expect(text).toContain('ctrl+c exit');
    expect(text).toContain('ctrl+o expand tools');
  });

  test('reflects the tools toggle state', () => {
    const text = textOf(
      StatusBarView({
        model: 'm',
        workspace: 'w',
        theme: 'dark',
        expandTools: true,
        tokens: PALETTES.dark,
      }),
    );
    expect(text).toContain('ctrl+o collapse tools');
  });

  test('nothing proprietary', () => {
    const text = textOf(
      StatusBarView({
        model: 'brain-default',
        workspace: 'w',
        theme: 'dark',
        expandTools: false,
        tokens: PALETTES.light,
      }),
    );
    expect(text.toLowerCase()).not.toContain('claude');
    expect(text.toLowerCase()).not.toContain('anthropic');
    expect(text.toLowerCase()).not.toContain('plan');
    expect(text.toLowerCase()).not.toContain('billing');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/ui/statusBar.test.tsx`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement**

Create `src/ui/shell/StatusBar.tsx`:
```tsx
/** Footer status line: workspace/model/theme context + keybind hints. */
import * as React from 'react';
import { Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';

export function StatusBarView(props: {
  model: string;
  workspace: string;
  theme: string;
  expandTools: boolean;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens; // reserved: segments gain token colors in later increments
  return (
    <Text dimColor>
      {props.workspace} · model {props.model} · theme {props.theme} · ! bash · / commands · ↑↓
      history · esc stop · ctrl+o {props.expandTools ? 'collapse' : 'expand'} tools · ctrl+c exit
    </Text>
  );
}
```

In `AppShell.tsx`, replace the closing footer `<Text dimColor>…</Text>` block with:
```tsx
      <StatusBarView
        model={model}
        workspace={workspaceLabel}
        theme={themeSetting}
        expandTools={expandTools}
        tokens={tokens}
      />
```
and hoist the workspace label next to the other hook outputs:
```tsx
  const workspaceLabel = path.basename(process.cwd()).slice(0, 24);
```
with `import * as path from 'path';` added to AppShell imports.

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/test/ui/statusBar.test.tsx`
Expected: PASS (3 tests).

- [ ] **Step 5: Full suite + commit**

Run: `bun test 2>&1 | tail -5` — no NEW failures.

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/shell/StatusBar.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/ui/statusBar.test.tsx
git commit -m "feat(brain-shell): status line footer with workspace, model, and theme

Replaces the bare hint footer; Brain content only — no vendor plan or
billing surfaces.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Resume picker + session replay

**Files:**
- Create: `packages/brain-shell/src/ui/overlays/resumePickerLogic.ts`
- Create: `packages/brain-shell/src/ui/overlays/ResumePicker.tsx`
- Create: `packages/brain-shell/src/state/sessionReplay.ts`
- Modify: `packages/brain-shell/src/state/sessionController.ts` (`listSessions`, `resumeSession`)
- Modify: `packages/brain-shell/src/commands/matcher.ts` (add `resume`)
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (picker state, `/resume`, overlay hook, render)
- Test: `packages/brain-shell/src/test/ui/overlays/resumePickerLogic.test.ts`
- Test: `packages/brain-shell/src/test/state/sessionReplay.test.ts`
- Test: `packages/brain-shell/src/test/state/sessionControllerResume.test.ts`

**Interfaces:**
- Consumes: `overlayListDecision` (Task 4), `overlay:*` bindings (Task 3), client `listSessions()`/`loadSession(id)` (existing seam), `BrainSessionSummary`/`BrainSession`/`BrainMessage` types from `client/BrainBackendClient.js`, `TranscriptRow`.
- Produces:
  - `formatAge(nowMs: number, updatedAtMs: number): string` — 'just now' (<60s), `Nm ago`, `Nh ago`, `Nd ago` (<7d), else `Mon DD`.
  - `resumeChoices(summaries: BrainSessionSummary[], nowMs: number): ResumeVM[]` where `interface ResumeVM { id: string; title: string; age: string; pinned: boolean }` — archived filtered, pinned first then newest first, capped at 8.
  - `sessionToRows(session: BrainSession): TranscriptRow[]` — text replay of prior turns.
  - Controller: `listSessions(): Promise<BrainSessionSummary[]>`, `resumeSession(sessionId: string): Promise<void>`.

- [ ] **Step 1: Write the failing tests**

Create `src/test/ui/overlays/resumePickerLogic.test.ts`:
```ts
import { describe, expect, test } from 'bun:test';
import { formatAge, resumeChoices } from '../../../ui/overlays/resumePickerLogic.js';
import type { BrainSessionSummary } from '../../../client/BrainBackendClient.js';

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const NOW = 1_800_000_000_000;

const s = (over: Partial<BrainSessionSummary>): BrainSessionSummary => ({
  id: 's',
  title: 'T',
  updatedAtMs: NOW - HOUR,
  pinned: false,
  archived: false,
  ...over,
});

describe('formatAge', () => {
  test('buckets', () => {
    expect(formatAge(NOW, NOW - 30_000)).toBe('just now');
    expect(formatAge(NOW, NOW - 5 * MIN)).toBe('5m ago');
    expect(formatAge(NOW, NOW - 3 * HOUR)).toBe('3h ago');
    expect(formatAge(NOW, NOW - 2 * DAY)).toBe('2d ago');
    expect(formatAge(NOW, NOW - 30 * DAY)).toMatch(/^[A-Z][a-z]{2} \d{1,2}$/);
  });
});

describe('resumeChoices', () => {
  test('filters archived, pins first, newest first, caps at 8', () => {
    const list = [
      s({ id: 'a', updatedAtMs: NOW - DAY }),
      s({ id: 'b', archived: true }),
      s({ id: 'c', pinned: true, updatedAtMs: NOW - 5 * DAY }),
      s({ id: 'd', updatedAtMs: NOW - MIN }),
    ];
    const out = resumeChoices(list, NOW);
    expect(out.map((v) => v.id)).toEqual(['c', 'd', 'a']);
    expect(out[0]).toEqual({ id: 'c', title: 'T', age: '5d ago', pinned: true });
  });

  test('caps at eight entries', () => {
    const many = Array.from({ length: 12 }, (_, i) => s({ id: `x${i}`, updatedAtMs: NOW - i * MIN }));
    expect(resumeChoices(many, NOW)).toHaveLength(8);
  });
});
```

Create `src/test/state/sessionReplay.test.ts`:
```ts
import { describe, expect, test } from 'bun:test';
import { sessionToRows } from '../../state/sessionReplay.js';
import type { BrainSession } from '../../client/BrainBackendClient.js';

const session = (messages: BrainSession['messages']): BrainSession => ({
  id: 'sess-1',
  title: 'T',
  createdAtMs: 0,
  updatedAtMs: 0,
  pinned: false,
  archived: false,
  messages,
});

describe('sessionToRows', () => {
  test('maps roles to transcript kinds in order', () => {
    const rows = sessionToRows(
      session([
        { id: 'm1', role: 'user', content: 'hello' },
        { id: 'm2', role: 'assistant', content: '**hi**' },
        { id: 'm3', role: 'system', content: 'note' },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'user', id: 'm1', text: 'hello' },
      { kind: 'assistant', id: 'm2', markdown: '**hi**' },
      { kind: 'system', id: 'm3', text: 'note' },
    ]);
  });

  test('skips empty content and synthesizes ids when missing', () => {
    const rows = sessionToRows(
      session([
        { id: '', role: 'user', content: '  ' },
        { id: '', role: 'assistant', content: 'answer' },
      ]),
    );
    expect(rows).toHaveLength(1);
    expect(rows[0]).toEqual({ kind: 'assistant', id: 'hist:1', markdown: 'answer' });
  });
});
```

Create `src/test/state/sessionControllerResume.test.ts`:
```ts
import { describe, expect, test } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainSession,
  BrainSessionSummary,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

function resumeFake(session: BrainSession | Error, summaries: BrainSessionSummary[] = []) {
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(_r: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      yield { type: 'finished', status: 'completed' };
    },
    async listSessions(): Promise<BrainSessionSummary[]> {
      return summaries;
    },
    async loadSession(id: string): Promise<{ session: BrainSession }> {
      if (session instanceof Error) throw session;
      return { session: { ...session, id } };
    },
  } as unknown as BrainBackendClient;
  return client;
}

const SESSION: BrainSession = {
  id: 'old-1',
  title: 'Refactor graph indexer',
  createdAtMs: 0,
  updatedAtMs: 0,
  pinned: false,
  archived: false,
  messages: [
    { id: 'm1', role: 'user', content: 'hello' },
    { id: 'm2', role: 'assistant', content: 'world' },
  ],
};

describe('SessionController resume', () => {
  test('resume adopts the session id and replays messages as rows', async () => {
    const ctl = new SessionController(resumeFake(SESSION));
    await ctl.init?.(); // no-op if the controller has no init method
    await ctl.resumeSession('old-1');
    const snap = ctl.getSnapshot();
    expect(snap.busy).toBe(false);
    expect(snap.rows.map((r) => r.kind)).toEqual(['user', 'assistant', 'system']);
    expect(snap.rows[0]).toMatchObject({ kind: 'user', text: 'hello' });
    expect(JSON.stringify(snap.rows.at(-1))).toContain('Resumed');
  });

  test('failed loads surface a system notice, not a crash', async () => {
    const ctl = new SessionController(resumeFake(new Error('socket gone')));
    await ctl.resumeSession('old-1');
    expect(JSON.stringify(ctl.getSnapshot().rows)).toContain('Could not resume');
  });

  test('listSessions passes through to the client', async () => {
    const summaries: BrainSessionSummary[] = [
      { id: 'x', title: 'X', updatedAtMs: 1, pinned: false, archived: false },
    ];
    const ctl = new SessionController(resumeFake(SESSION, summaries));
    expect(await ctl.listSessions()).toEqual(summaries);
  });
});
```
Implementer note: if `SessionController` has no `init()` method, drop that line — the fake satisfies `createSession` lazily the same way the existing `fakeClient` does. Match whatever lazy-init pattern the existing `sessionController.test.ts` uses to get a usable instance.

- [ ] **Step 2: Run tests to verify they fail**

Run: `bun test src/test/ui/overlays/resumePickerLogic.test.ts src/test/state/sessionReplay.test.ts src/test/state/sessionControllerResume.test.ts`
Expected: FAIL — modules/methods missing.

- [ ] **Step 3: Implement**

Create `src/ui/overlays/resumePickerLogic.ts`:
```ts
/** Pure core for the /resume picker: ordering, age labels, decisions. */
import type { BrainSessionSummary } from '../../client/BrainBackendClient.js';
import { overlayListDecision, type OverlayListDecision } from './overlayLogic.js';

export interface ResumeVM {
  id: string;
  title: string;
  age: string;
  pinned: boolean;
}

export const RESUME_MAX_ITEMS = 8;

export function formatAge(nowMs: number, updatedAtMs: number): string {
  const dt = Math.max(0, nowMs - updatedAtMs);
  if (dt < 60_000) return 'just now';
  if (dt < 3_600_000) return `${Math.floor(dt / 60_000)}m ago`;
  if (dt < 86_400_000) return `${Math.floor(dt / 3_600_000)}h ago`;
  if (dt < 7 * 86_400_000) return `${Math.floor(dt / 86_400_000)}d ago`;
  const d = new Date(updatedAtMs);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function resumeChoices(summaries: BrainSessionSummary[], nowMs: number): ResumeVM[] {
  return summaries
    .filter((s) => !s.archived)
    .sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || b.updatedAtMs - a.updatedAtMs)
    .slice(0, RESUME_MAX_ITEMS)
    .map((s) => ({ id: s.id, title: s.title, age: formatAge(nowMs, s.updatedAtMs), pinned: s.pinned }));
}

export function resumeListDecision(action: string | null, selected: number, count: number): OverlayListDecision {
  return overlayListDecision(action, selected, count);
}
```

Create `src/state/sessionReplay.ts`:
```ts
/** Replay a stored session's messages as frozen transcript rows. */
import type { BrainSession } from '../client/BrainBackendClient.js';
import type { TranscriptRow } from '../contracts/messages.js';

export function sessionToRows(session: BrainSession): TranscriptRow[] {
  return session.messages.flatMap((m, i) => {
    const text = (m.content ?? '').trim();
    if (text.length === 0) return [];
    const id = m.id && m.id.length > 0 ? m.id : `hist:${i}`;
    if (m.role === 'user') return [{ kind: 'user' as const, id, text }];
    if (m.role === 'assistant') return [{ kind: 'assistant' as const, id, markdown: text }];
    return [{ kind: 'system' as const, id, text }];
  });
}
```

In `src/state/sessionController.ts`:
1. Imports to add:
```ts
import { sessionToRows } from './sessionReplay.js';
import type { BrainSessionSummary } from '../client/BrainBackendClient.js';
```
(merge the type into the existing client-type import block if preferred)
2. Methods after `clear()`/`notice()`:
```ts
  async listSessions(): Promise<BrainSessionSummary[]> {
    return this.client.listSessions();
  }

  async resumeSession(sessionId: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    try {
      const { session } = await this.client.loadSession(sessionId);
      this.sessionId = session.id;
      this.rows = [...sessionToRows(session)];
      this.sysSeq += 1;
      this.rows = [
        ...this.rows,
        { kind: 'system', id: `sys:${this.sysSeq}`, text: `Resumed “${session.title}”` },
      ];
      this.emit();
    } catch (e) {
      this.notice(`Could not resume session: ${e instanceof Error ? e.message : String(e)}`);
    }
  }
```
(If the class already tracks `sysSeq` differently, reuse its private counter — the observable contract is the appended system row.)

Modify `src/commands/matcher.ts` — extend `COMMANDS`:
```ts
  { name: 'resume', description: 'Resume a previous session' },
```

- [ ] **Step 4: Wire AppShell**

In `src/ui/shell/AppShell.tsx`:

Imports:
```tsx
import { resumeChoices, resumeListDecision, type ResumeVM } from '../overlays/resumePickerLogic.js';
import { ResumePickerView } from '../overlays/ResumePicker.js';
```

State (beside the theme picker state):
```tsx
  const [resumeOpen, setResumeOpen] = React.useState(false);
  const [resumeItems, setResumeItems] = React.useState<ResumeVM[]>([]);
  const [resumeSelected, setResumeSelected] = React.useState(0);
```

Third `useBoundInput` registration:
```tsx
  useBoundInput({
    contexts: ['overlay'],
    isActive: resumeOpen,
    onAction: (action) => {
      const d = resumeListDecision(action, resumeSelected, resumeItems.length);
      if (d.type === 'move') {
        setResumeSelected(d.index);
      } else if (d.type === 'commit') {
        setResumeOpen(false);
        const chosen = resumeItems[d.index];
        if (chosen) void controller.resumeSession(chosen.id);
      } else if (d.type === 'cancel') {
        setResumeOpen(false);
      }
    },
  });
```

`runCommand` case:
```tsx
    else if (chosen.name === 'resume') {
      if (snapshot.busy) {
        controller.notice('Busy — wait for the current turn to finish.');
        return;
      }
      void controller.listSessions().then((all) => {
        const items = resumeChoices(all, Date.now());
        if (items.length === 0) {
          controller.notice('No previous sessions found.');
          return;
        }
        setResumeItems(items);
        setResumeSelected(0);
        setResumeOpen(true);
      });
    }
```

Render (before the composer wrapper; alongside the theme picker conditional):
```tsx
      {resumeOpen ? (
        <Box marginTop={1}>
          <ResumePickerView items={resumeItems} selectedIndex={resumeSelected} tokens={tokens} />
        </Box>
      ) : null}
```
and update the composer's `paused` prop to cover both overlays:
```tsx
          paused={themeOpen || resumeOpen}
```

Create `src/ui/overlays/ResumePicker.tsx`:
```tsx
/** /resume overlay: prior sessions, pinned first, relative ages. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ResumeVM } from './resumePickerLogic.js';

export function ResumePickerView(props: {
  items: readonly ResumeVM[];
  selectedIndex: number;
  tokens: BrainTokens;
}): React.ReactElement {
  const sel = Math.min(props.selectedIndex, Math.max(0, props.items.length - 1));
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.promptBorder} paddingX={1}>
      <Text bold>Resume session</Text>
      {props.items.map((it, i) => (
        <Text key={it.id} inverse={i === sel}>
          {(i === sel ? '❯ ' : '  ') + (it.pinned ? '★ ' : '')}
          {`${it.title.slice(0, 46)} — ${it.age}`}
        </Text>
      ))}
      <Text dimColor>↑↓ navigate · enter resume · esc cancel</Text>
    </Box>
  );
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `bun test src/test/ui/overlays/ src/test/state/sessionReplay.test.ts src/test/state/sessionControllerResume.test.ts src/test/commands/matcher.test.ts`
Expected: all PASS.

- [ ] **Step 6: Full suite + commit**

Run: `bun test 2>&1 | tail -5` — no NEW failures.

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/overlays/resumePickerLogic.ts packages/brain-shell/src/ui/overlays/ResumePicker.tsx packages/brain-shell/src/state/sessionReplay.ts packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/commands/matcher.ts packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/ui/overlays/resumePickerLogic.test.ts packages/brain-shell/src/test/state/sessionReplay.test.ts packages/brain-shell/src/test/state/sessionControllerResume.test.ts
git commit -m "feat(brain-shell): /resume picker with transcript replay

Lists prior sessions (pinned first, relative ages) over the existing
listSessions/loadSession seams; resuming adopts the session id and
replays stored messages as frozen transcript rows.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Permission chunk plumbing (client tolerance + controller state)

Additive reception for `tool_permission_requested` wire frames. The daemon does not send them yet; the shell becomes ready to. **No file under `src/adapter/` is touched** (constraint 4; the turn-event vocabulary already names these events and the transformer stays out of scope).

**Files:**
- Modify: `packages/brain-shell/src/client/BrainBackendClient.ts` (`BrainStreamChunk` union + fields)
- Modify: `packages/brain-shell/src/client/UdsBrainBackendClient.ts` (stream-parser branch)
- Modify: `packages/brain-shell/src/state/sessionController.ts` (`ShellSnapshot.permission`, live intercept, `resolvePermission`)
- Test: `packages/brain-shell/src/test/state/sessionControllerPermission.test.ts`

**Interfaces:**
- Produces:
  - `BrainStreamChunk` gains union member `'permission_request'` with optional fields `callId?: string; toolName?: string; input?: Record<string, unknown>; reason?: string;`.
  - `PendingPermissionView { callId: string; toolName: string; input: Record<string, unknown>; reason?: string }` exported from `sessionController.ts`; `ShellSnapshot.permission?: PendingPermissionView`.
  - Controller `resolvePermission(callId: string, granted: boolean): void` — clears the pending view, appends an `ℹ Allowed <tool>` / `ℹ Denied <tool>` system notice, and on deny flips the matching tool card (matched by `callId`) to `denied`.

- [ ] **Step 1: Write the failing test**

Create `src/test/state/sessionControllerPermission.test.ts`:
```ts
import { describe, expect, test } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

function scriptFake(chunks: BrainStreamChunk[]) {
  const client = {
    async createSession() {
      return { sessionId: 'stub-session-1', title: 'stub', createdAtMs: 0 };
    },
    async *streamText(_r: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      for (const c of chunks) yield c;
    },
  } as unknown as BrainBackendClient;
  return client;
}

const PERM_SCRIPT: BrainStreamChunk[] = [
  {
    type: 'permission_request',
    callId: 'call_9',
    toolName: 'bash',
    input: { command: 'rm -rf build' },
    reason: 'destructive',
  },
  { type: 'finished', status: 'completed' },
];

describe('SessionController permission requests', () => {
  test('a permission_request chunk parks a pending dialog in the snapshot', async () => {
    const ctl = new SessionController(scriptFake(PERM_SCRIPT));
    await ctl.submit('clean this up');
    const snap = ctl.getSnapshot();
    expect(snap.permission).toEqual({
      callId: 'call_9',
      toolName: 'bash',
      input: { command: 'rm -rf build' },
      reason: 'destructive',
    });
  });

  test('grant clears the pending dialog and posts an Allowed notice', async () => {
    const ctl = new SessionController(scriptFake(PERM_SCRIPT));
    await ctl.submit('clean this up');
    ctl.resolvePermission('call_9', true);
    const snap = ctl.getSnapshot();
    expect(snap.permission).toBeUndefined();
    expect(JSON.stringify(snap.rows)).toContain('Allowed bash');
    expect(JSON.stringify(snap.rows)).not.toContain('denied');
  });

  test('deny marks the matching tool card denied by callId', async () => {
    const ctl = new SessionController(
      scriptFake([
        { type: 'permission_request', callId: 'call_9', toolName: 'bash', input: {} },
        { type: 'finished', status: 'completed' },
      ]),
    );
    await ctl.submit('go');
    ctl.resolvePermission('call_9', false);
    const snap = ctl.getSnapshot();
    expect(snap.permission).toBeUndefined();
    expect(JSON.stringify(snap.rows)).toContain('Denied bash');
  });

  test('resolving an unknown callId is a no-op', () => {
    const ctl = new SessionController(scriptFake([]));
    ctl.resolvePermission('ghost', true);
    expect(ctl.getSnapshot().permission).toBeUndefined();
  });
});
```
Note: whether a `tool` row materializes for `call_9` depends on the existing chunk→row pipeline; the deny assertion pins the *notice*, and the tool-card flip is additionally pinned in the next test if a tool_use chunk precedes the request:
```ts
  test('deny flips a preceding tool card to denied', async () => {
    const ctl = new SessionController(
      scriptFake([
        { type: 'tool_use', toolUse: { id: 'call_9', name: 'bash', input: { command: 'ls' } } },
        { type: 'permission_request', callId: 'call_9', toolName: 'bash', input: { command: 'ls' } },
        { type: 'finished', status: 'completed' },
      ]),
    );
    await ctl.submit('go');
    ctl.resolvePermission('call_9', false);
    const toolRow = ctl.getSnapshot().rows.find((r) => r.kind === 'tool');
    expect(toolRow && toolRow.kind === 'tool' && toolRow.tool.status).toBe('denied');
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bun test src/test/state/sessionControllerPermission.test.ts`
Expected: FAIL — `'permission_request'` not assignable / `permission` missing on snapshot.

- [ ] **Step 3: Implement**

In `src/client/BrainBackendClient.ts` — extend the union and add fields to `BrainStreamChunk`:
```ts
export interface BrainStreamChunk {
  type:
    | 'token'
    | 'thinking'
    | 'redacted_thinking'
    | 'tool_use'
    | 'error'
    | 'finished'
    | 'permission_request';
  …
  /** Present when type === 'permission_request'. */
  callId?: string;
  toolName?: string;
  input?: Record<string, unknown>;
  reason?: string;
```
(additive; leave every existing member untouched)

In `src/client/UdsBrainBackendClient.ts` — inside the stream loop's raw-frame dispatch (alongside the `token`/`thinking`/`tool_use` branches), add:
```ts
        } else if (raw.type === 'tool_permission_requested') {
          yield {
            type: 'permission_request',
            callId: raw.callId ?? raw.call_id,
            toolName: raw.toolName ?? raw.tool_name,
            input: (raw.input ?? {}) as Record<string, unknown>,
            reason: raw.reason,
          };
```

In `src/state/sessionController.ts`:
1. Export near `ShellSnapshot`:
```ts
export interface PendingPermissionView {
  callId: string;
  toolName: string;
  input: Record<string, unknown>;
  reason?: string;
}
```
2. `ShellSnapshot` gains `permission?: PendingPermissionView;` (update the initial `snapshot` field literal type implicitly via the interface; initialize `permission: undefined` in the constructor snapshot if the object is built exhaustively).
3. Private field: `private pendingPermission: PendingPermissionView | undefined;` — and make sure `emit()` composes `snapshot` to include `permission: this.pendingPermission` (follow however `emit()` currently rebuilds `this.snapshot`).
4. In `handleChunk`, BEFORE the existing `chunkToTurnEvent` mapping (so the transformer/event log never sees it):
```ts
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
5. Public method (beside `abort()`):
```ts
  resolvePermission(callId: string, granted: boolean): void {
    if (this.pendingPermission?.callId !== callId) return;
    const toolName = this.pendingPermission.toolName;
    this.pendingPermission = undefined;
    this.rows = this.rows.map((r) =>
      r.kind === 'tool' && r.tool.callId === callId && !granted
        ? { ...r, tool: { ...r.tool, status: 'denied' as const } }
        : r,
    );
    this.notice(`${granted ? 'Allowed' : 'Denied'} ${toolName}`);
  }
```
(`notice()` already appends + emits.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun test src/test/state/sessionControllerPermission.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Full suite + commit**

Run: `bun test 2>&1 | tail -5` — no NEW failures (existing controller suites must stay green).

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/client/UdsBrainBackendClient.ts packages/brain-shell/src/state/sessionController.ts packages/brain-shell/src/test/state/sessionControllerPermission.test.ts
git commit -m "feat(brain-shell): tolerate tool_permission_requested stream frames

Additive chunk member + UDS normalization; the controller parks a live
pending-permission view on the snapshot and resolves it locally (notice
+ tool-card status). Wire round-trip lands with daemon-side support;
adapter files stay untouched per constraint 4.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: Permission dialog UI

**Files:**
- Create: `packages/brain-shell/src/ui/overlays/permissionDialogLogic.ts`
- Create: `packages/brain-shell/src/ui/overlays/PermissionDialog.tsx`
- Modify: `packages/brain-shell/src/ui/shell/AppShell.tsx` (dialog hook + render)
- Test: `packages/brain-shell/src/test/ui/overlays/permissionDialogLogic.test.ts`
- Test: `packages/brain-shell/src/test/ui/overlays/permissionDialogView.test.tsx`

**Interfaces:**
- Consumes: `dialog:*` actions (Task 3), `snapshot.permission` + `controller.resolvePermission` (Task 7), `summarizeToolInput` from `../transcript/MessageRow.js` (already exported), `PendingPermissionView`.
- Produces: `dialogDecision(action: string | null, selected: number): DialogDecision` with `type DialogDecision = { type: 'allow' } | { type: 'deny' } | { type: 'move'; index: 0 | 1 } | { type: 'passthrough' }`; `PermissionDialogView(props: { req: PendingPermissionView; selected: number; tokens: BrainTokens })` (pure).

- [ ] **Step 1: Write the failing tests**

Create `src/test/ui/overlays/permissionDialogLogic.test.ts`:
```ts
import { describe, expect, test } from 'bun:test';
import { dialogDecision } from '../../../ui/overlays/permissionDialogLogic.js';

describe('dialogDecision', () => {
  test('direct keys decide', () => {
    expect(dialogDecision('dialog:allow', 1)).toEqual({ type: 'allow' });
    expect(dialogDecision('dialog:deny', 0)).toEqual({ type: 'deny' });
    expect(dialogDecision('dialog:cancel', 0)).toEqual({ type: 'deny' }); // esc denies
  });

  test('arrows move within [Allow, Deny]; enter confirms selection', () => {
    expect(dialogDecision('dialog:left', 1)).toEqual({ type: 'move', index: 0 });
    expect(dialogDecision('dialog:left', 0)).toEqual({ type: 'move', index: 0 });
    expect(dialogDecision('dialog:right', 0)).toEqual({ type: 'move', index: 1 });
    expect(dialogDecision('dialog:right', 1)).toEqual({ type: 'move', index: 1 });
    expect(dialogDecision('dialog:commit', 0)).toEqual({ type: 'allow' });
    expect(dialogDecision('dialog:commit', 1)).toEqual({ type: 'deny' });
  });

  test('null and unrelated actions pass through', () => {
    expect(dialogDecision(null, 0)).toEqual({ type: 'passthrough' });
    expect(dialogDecision('overlay:up', 0)).toEqual({ type: 'passthrough' });
  });
});
```

Create `src/test/ui/overlays/permissionDialogView.test.tsx`:
```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { PermissionDialogView } from '../../../ui/overlays/PermissionDialog.js';

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

describe('PermissionDialogView', () => {
  test('shows tool, summarized input, and both options with selection', () => {
    const text = textOf(
      PermissionDialogView({
        req: { callId: 'c1', toolName: 'bash', input: { command: 'rm -rf build' }, reason: 'destructive' },
        selected: 1,
        tokens: PALETTES.dark,
      }),
    );
    expect(text).toContain('Permission required');
    expect(text).toContain('bash');
    expect(text).toContain('rm -rf build');
    expect(text).toContain('[ Deny ]');
    expect(text).toContain('[ Allow ]');
    expect(text).toContain('esc denies');
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `bun test src/test/ui/overlays/permissionDialog`
Expected: FAIL — modules missing.

- [ ] **Step 3: Implement**

Create `src/ui/overlays/permissionDialogLogic.ts`:
```ts
/**
 * Decision table for the permission dialog. Options are fixed:
 * index 0 = Allow, index 1 = Deny. esc always denies — a permission the
 * user dismisses is a permission not granted.
 */
export type DialogDecision =
  | { type: 'allow' }
  | { type: 'deny' }
  | { type: 'move'; index: 0 | 1 }
  | { type: 'passthrough' };

export function dialogDecision(action: string | null, selected: number): DialogDecision {
  if (action === null) return { type: 'passthrough' };
  switch (action) {
    case 'dialog:allow':
      return { type: 'allow' };
    case 'dialog:deny':
    case 'dialog:cancel':
      return { type: 'deny' };
    case 'dialog:left':
      return { type: 'move', index: 0 };
    case 'dialog:right':
      return { type: 'move', index: 1 };
    case 'dialog:commit':
      return selected === 0 ? { type: 'allow' } : { type: 'deny' };
    default:
      return { type: 'passthrough' };
  }
}
```

Create `src/ui/overlays/PermissionDialog.tsx`:
```tsx
/** Modal permission dialog: tool + summarized input + Allow/Deny. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { PendingPermissionView } from '../../state/sessionController.js';
import { summarizeToolInput } from '../transcript/MessageRow.js';

export function PermissionDialogView(props: {
  req: PendingPermissionView;
  selected: number;
  tokens: BrainTokens;
}): React.ReactElement {
  const summary = summarizeToolInput(props.req.input);
  const opt = (label: string, i: number): string =>
    `${i === props.selected ? '❯ ' : '  '}[ ${label} ]`;
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.warning} paddingX={1}>
      <Text bold color={props.tokens.warning}>Permission required</Text>
      <Text>
        {props.req.toolName}
        {summary.length > 0 ? ` — ${summary}` : ''}
      </Text>
      {props.req.reason ? <Text dimColor>{props.req.reason}</Text> : null}
      <Text>{opt('Allow', 0)}   {opt('Deny', 1)}</Text>
      <Text dimColor>←→ choose · enter confirm · y allow · n deny · esc denies</Text>
    </Box>
  );
}
```

Wire `AppShell.tsx`:

Imports:
```tsx
import { dialogDecision } from '../overlays/permissionDialogLogic.js';
import { PermissionDialogView } from '../overlays/PermissionDialog.js';
```

State + effect (beside the other overlay state):
```tsx
  const [permSelected, setPermSelected] = React.useState(0);
  const permission = snapshot.permission;
  React.useEffect(() => {
    setPermSelected(0);
  }, [permission?.callId]);
```

Fourth `useBoundInput` registration:
```tsx
  useBoundInput({
    contexts: ['dialog'],
    isActive: permission !== undefined,
    onAction: (action) => {
      if (!permission) return;
      const d = dialogDecision(action, permSelected);
      if (d.type === 'move') setPermSelected(d.index);
      else if (d.type === 'allow') controller.resolvePermission(permission.callId, true);
      else if (d.type === 'deny') controller.resolvePermission(permission.callId, false);
    },
  });
```

Composer pause widens:
```tsx
          paused={themeOpen || resumeOpen || permission !== undefined}
```

Render above the composer (with the other overlay conditionals):
```tsx
      {permission ? (
        <Box marginTop={1}>
          <PermissionDialogView req={permission} selected={permSelected} tokens={tokens} />
        </Box>
      ) : null}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `bun test src/test/ui/overlays/`
Expected: all overlay suites PASS.

- [ ] **Step 5: Full suite + commit**

Run: `bun test 2>&1 | tail -5` — no NEW failures.

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add packages/brain-shell/src/ui/overlays/permissionDialogLogic.ts packages/brain-shell/src/ui/overlays/PermissionDialog.tsx packages/brain-shell/src/ui/shell/AppShell.tsx packages/brain-shell/src/test/ui/overlays/permissionDialogLogic.test.ts packages/brain-shell/src/test/ui/overlays/permissionDialogView.test.tsx
git commit -m "feat(brain-shell): permission dialog over pending tool approvals

Rounded-border modal with Allow/Deny, keyboard-first (y/n, arrows +
enter); esc and n both deny — dismissal never grants. Pauses the
composer while visible.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 9: PTY smoke — Increment 3 end-to-end + final gates

**Files:**
- Create: `scripts/ptySmokeInc3.py`
- Create (generated by the run, then committed): `packages/brain-shell/src/test/fixtures/pty/inc3/{welcome,theme,resume,permission}.txt`

**Interfaces:**
- Consumes: everything shipped in Tasks 1–8; the Inc 2 harness discipline (stub UDS daemon, winsize ioctl, discrete keystrokes, ANSI-stripped matching).
- Produces: the increment's smoke gate. Stub daemon handles: `v1/session/create`, `session/list`, `v1/session/load`, `v1/generation/stream` (emitting tool_use → tool_permission_requested → finished for the permission flow).

- [ ] **Step 1: Write the smoke script**

Model `scripts/ptySmokeInc3.py` on `scripts/ptySmokeInc2.py` (same pump/snapshot/expect helpers, same teardown). Full script:

```python
#!/usr/bin/env python3
"""Increment 3 PTY smoke: welcome frame, /theme picker, /resume picker with
replay, and the permission dialog driven by tool_permission_requested.

Discipline (carried from Inc 1/2): stub UDS daemon, winsize ioctl before
exec, discrete keystroke writes with >=0.3 s pumps between distinct keys
(ink parses one stdin chunk as one keypress), ANSI-stripped matching.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc3-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc3-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc3"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
NOW_MS = int(time.time() * 1000)

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

PERMISSION_STATE = {"asked": False}

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
                    act = req.get("action")
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-session-3"}})
                    elif act == "session/list":
                        reply({"id": rid, "status": "success", "body": {
                            "sessions": [{
                                "session_id": "sess-old-9",
                                "title": "Refactor graph indexer",
                                "message_count": 2,
                                "created_at": NOW_MS // 1000 - 7200,
                                "updated_at": NOW_MS // 1000 - 300,
                            }],
                            "total": 1}})
                    elif act == "v1/session/load":
                        reply({"id": rid, "status": "success", "body": {"session": {
                            "id": "sess-old-9",
                            "title": "Refactor graph indexer",
                            "archived": False, "pinned": False,
                            "updated_at_ms": NOW_MS - 300_000,
                            "messages": [
                                {"id": "m1", "role": "user", "content": "index the graph"},
                                {"id": "m2", "role": "assistant", "content": "Indexed 42 nodes."},
                            ]}}})
                    elif act == "v1/generation/stream":
                        # Turn: tool call → permission request → (resolution is
                        # local) → completion. Emit the permission frame once.
                        reply({"type": "tool_use", "toolUse": {"id": "call_9",
                               "name": "bash", "input": {"command": "ls build"}},
                               "sequence": 0})
                        time.sleep(0.2)
                        PERMISSION_STATE["asked"] = True
                        reply({"type": "tool_permission_requested", "call_id": "call_9",
                               "tool_name": "bash", "input": {"command": "ls build"},
                               "reason": "shell access", "sequence": 1})
                        time.sleep(1.5)   # give the smoke time to answer before closing
                        reply({"type": "finished", "status": "completed", "sequence": 2})
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

threading.Thread(target=serve, daemon=True).start()
if os.path.exists(FRAMES_FILE):
    os.remove(FRAMES_FILE)

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.environ["BRAIN_CONFIG_PATH"] = "/tmp/brain-inc3-smoke-config.json"
    if os.path.exists("/tmp/brain-inc3-smoke-config.json"):
        os.remove("/tmp/brain-inc3-smoke-config.json")
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

# ── Flow A: welcome frame ──────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("welcome-identity", "memory-first agent workspace")
ok &= expect("welcome-hints", "/resume sessions")
ok &= expect("launch-prompt", "❯")
snapshot("welcome")

# ── Flow B: /theme picker, live preview, commit ────────────────────────────
os.write(fd, b"/theme")                  # one chunk inserts as text
pump(0.3)
os.write(fd, b"\r")                      # enter submits
ok &= expect("theme-title", "Theme")
ok &= expect("theme-auto-entry", "Auto (detect terminal)")
ok &= expect("theme-current-check", "✓")
os.write(fd, b"\x1b[B")                  # ↓ moves selection (preview switches)
pump(0.3)
ok &= expect("theme-selection-moved", "❯ Dark")
snapshot("theme")
os.write(fd, b"\r")                      # commit → persists to BRAIN_CONFIG_PATH
deadline = time.time() + 5
committed = False
while time.time() < deadline:
    pump(0.1)
    try:
        with open("/tmp/brain-inc3-smoke-config.json") as f:
            if json.load(f).get("theme") == "dark":
                committed = True
                break
    except Exception:
        pass
print(("PASS" if committed else "FAIL") + " theme-persisted")
ok &= committed
pump(0.5)

# ── Flow C: /resume picker + transcript replay ─────────────────────────────
os.write(fd, b"/resume")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("resume-title", "Resume session")
ok &= expect("resume-entry", "Refactor graph indexer")
snapshot("resume")
os.write(fd, b"\r")                      # pick it
ok &= expect("resume-replayed-user", "index the graph")
ok &= expect("resume-replayed-assistant", "Indexed 42 nodes.")
ok &= expect("resume-notice", "Resumed")

# ── Flow D: permission dialog over a streamed tool call ────────────────────
os.write(fd, b"list the build folder")   # plain prompt (multi-char paste is fine)
pump(0.3)
os.write(fd, b"\r")
ok &= expect("tool-card", "bash")
ok &= expect("dialog-header", "Permission required")
ok &= expect("dialog-tool", "[ Allow ]")
snapshot("permission")
os.write(fd, b"y")                       # allow
ok &= expect("allowed-notice", "Allowed bash")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
```

- [ ] **Step 2: Run the smoke to a green exit**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc3.py; echo "exit=$?"`
Expected: every flow PASS, `exit=0`. Iterate ONLY on harness bugs (keystroke timing, stub frame shapes) — if a product behavior is wrong, fix the product with a unit test first, then rerun.

Known timing hazards to watch: the ↓ keystroke after the theme dialog opens must land after the dialog renders (0.3 s pump is the floor); Flow D's `y` must land while the dialog is up (the stub delays `finished` by 1.5 s to make that window generous).

- [ ] **Step 3: Final gates on the branch**

Run, from repo root unless noted:
```bash
# 1. Unit suite — zero NEW failures vs 179 pass / 5 documented fails
cd packages/brain-shell && bun test 2>&1 | tail -5
# 2. Bundle
bun build src/main.tsx --outdir dist --target bun >/dev/null 2>&1 && echo BUILD_OK
# 3. Vendor audit — diff-scoped (gotcha #6); base = branch point 31db121
cd /Users/ritikpathania/Developer/PyCharm/brain && git diff 31db121..HEAD -- packages/brain-shell/src/ | grep '^+' | grep -icE 'claude|anthropic|vendor'
# 4. Explicit-path audit — every commit's files match its message
git log --stat 31db121..HEAD | less   # eyeball; each commit 2–10 named paths
```
Expected: `≥210 pass / 5 fail`, `BUILD_OK`, vendor count `0`, path audit clean.

- [ ] **Step 4: Commit the smoke + fixtures**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain && git add scripts/ptySmokeInc3.py packages/brain-shell/src/test/fixtures/pty/inc3/welcome.txt packages/brain-shell/src/test/fixtures/pty/inc3/theme.txt packages/brain-shell/src/test/fixtures/pty/inc3/resume.txt packages/brain-shell/src/test/fixtures/pty/inc3/permission.txt
git commit -m "test(brain-shell): Inc 3 PTY smoke — welcome, theme, resume, permission

Stub daemon grows session/list, session/load, and a permission-bearing
generation stream; four flows exercise the new session frame end-to-end
with the Inc 1/2 keystroke discipline.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-review record

- **Spec coverage:** §5 row 3 lists five deliverables → welcome/logo (Task 1), resume picker (Task 6), status line (Task 5), themes + daltonized with auto (Tasks 2–4), permission dialogs (Tasks 7–8); gate "Unit tests + PTY smoke" → per-task unit tests + Task 9 smoke. §7 gap-table rows 77–81 all land in a task. Constraint 4 honored: zero `src/adapter/` modifications (grep-audited in Task 9 step 3 implicitly via vendor/path audit; the plan's Files sections name none).
- **Placeholder scan:** no TBD/TODO; every code step carries complete code. The remaining "implementer note" (Task 6 controller-test lazy init) resolves a genuine idiom question by pointing at the authoritative existing test file, not deferring design. The `setSetting` accessor was verified against `themeContext.tsx` during plan review and needs no note.
- **Type consistency:** `OverlayListDecision` produced Task 4, consumed Task 6 (`resumeListDecision` delegates); `dialog:*`/`overlay:*` action ids identical across Tasks 3/4/6/8; `PendingPermissionView` defined Task 7, imported by Task 8's view; `ResumeVM` defined/consumed within Task 6; `ThemeSetting` spelled identically everywhere; `paused?: boolean` introduced and consumed in the same task, widened in Task 8.
- **Risk watch:** Task 4's AppShell growth is the largest edit — executor must keep hook order unconditional and stable. Task 7's `emit()` composition must thread `pendingPermission` into the snapshot; the failing test pins it. Task 9's Flow B asserts `✓` which also appears nowhere earlier in Flow B's window — acceptable; tighten to `✓ Auto` if ambiguous at run time.
