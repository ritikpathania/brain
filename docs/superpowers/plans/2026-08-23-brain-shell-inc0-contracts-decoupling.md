# Brain Shell Increment 0 — Contracts Layer & Vendor Decoupling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `packages/brain-shell` compile, test, and boot with **zero** imports from `vendor/claude`, by introducing Brain-owned `contracts/` (types + factories) and `compat/` (runtime shims over stock Ink), deleting Anthropic-specific dead code, swapping the entrypoint to a Brain skeleton shell, then removing the 168 MB vendored tree.

**Architecture:** Contracts define Brain's own UI vocabulary (messages, tools, query seam, session, model, theme, commands). Compat provides small original implementations of generic terminal utilities consumed from vendor today. All daemon communication stays in the untouched `client/` + `adapter/` seams.

**Tech Stack:** Bun ≥ 1.2, TypeScript ~7 (strict), React 19.2, Ink 7.1 (stock npm), yoga-layout, Zod 4. Python3 stdlib for PTY smoke.

**Spec:** `docs/superpowers/specs/2026-08-23-brain-shell-contracts-first-design.md`

## Global Constraints

- Working directory for every task: `/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell` unless stated otherwise.
- **Never import anything from `vendor/claude` after Task 9 completes; never re-add such imports.**
- No Claude/Anthropic product concepts in UI copy or code names: no models, APIs, auth, pricing, billing, plan tiers. The string "Claude Code" must not appear in shipped UI text.
- Do not modify files under `crates/`, `apps/`, `protocol/`, `schemas/` (Rust side is out of scope).
- Commit messages: conventional commits, end with `Co-Authored-By: Claude <noreply@anthropic.com>`.
- The repo has ~3,700 unrelated uncommitted changes: ALWAYS `git add` explicit paths, never `-A`.
- Test baseline recorded 2026-08-23: `bun test` → 277 pass / 33–34 fail across 42 files (~121 s). Gate for Task 12: pass count ≥ 277 minus tests deleted by Task 10, and no failure that wasn't already failing at baseline.
- Known-broken at baseline (do not fix here): Phase 1/2/4/6.5 component suites (BrainModal, WorkspaceDashboard, Resume picker, cell-grid parity) — failing before any of our changes.

---

### Task 1: Fix tsconfig so typecheck runs on TypeScript 7

**Files:**
- Modify: `packages/brain-shell/tsconfig.json` (line ~11)

**Interfaces:**
- Consumes: nothing
- Produces: a working `bunx tsc --noEmit`; later tasks rely on it as a gate

- [ ] **Step 1: Remove the removed option**

In `packages/brain-shell/tsconfig.json`, delete the `"baseUrl": "."` line. If `"paths"` entries are relative-valued, ensure they remain as-is (TS7 resolves them relative to the tsconfig location).

- [ ] **Step 2: Run tsc and record the error baseline**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bunx tsc --noEmit 2>&1 | tail -5`
Expected: TS5102 about `baseUrl` is GONE. Remaining output is pre-existing type errors — record their count:
`bunx tsc --noEmit 2>&1 | grep -c "error TS" > /tmp/tsc-baseline.txt`

- [ ] **Step 3: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git add packages/brain-shell/tsconfig.json
git commit -m "chore(brain-shell): drop TS7-removed baseUrl option

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: contracts/messages.ts — message taxonomy & factories

The adapter consumes CC-style envelopes: `{type:'user'|'assistant', message:{content: string|Block[]}}`. This contract makes that shape Brain-owned. Ground truth consumer: `src/adapter/brainCallModel.ts:41-108`.

**Files:**
- Create: `src/contracts/messages.ts`
- Test: `src/test/contracts/messages.test.ts`

**Interfaces:**
- Produces: types `Message`, `UserMessage`, `AssistantMessage`, `StreamEvent`, `ContentBlock`, `ThinkingBlock`, `RedactedThinkingBlock`, `ToolUseBlock`, `ToolResultBlock`, `TextBlock`; functions `createUserMessage(content: string): UserMessage`, `createAssistantMessage(content: string): AssistantMessage`, `createAssistantAPIErrorMessage(error: string): AssistantMessage`, `extractTag(text: string, tag: string): string | null`, `getMessagesAfterCompactBoundary(messages: Message[]): Message[]`, `handleMessageFromStream(event: StreamEvent, messages: Message[]): Message[]`

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/messages.test.ts
import { describe, expect, test } from 'bun:test';
import {
  createAssistantAPIErrorMessage,
  createAssistantMessage,
  createUserMessage,
  extractTag,
  getMessagesAfterCompactBoundary,
} from '../../contracts/messages.js';

describe('contracts/messages', () => {
  test('createUserMessage wraps content in the envelope shape', () => {
    const m = createUserMessage('hello');
    expect(m.type).toBe('user');
    expect(m.message.content).toBe('hello');
    expect(typeof m.uuid).toBe('string');
  });

  test('createAssistantMessage produces assistant envelope', () => {
    const m = createAssistantMessage('hi there');
    expect(m.type).toBe('assistant');
    expect(m.message.content[0]).toEqual({ type: 'text', text: 'hi there' });
  });

  test('createAssistantAPIErrorMessage marks isError', () => {
    const m = createAssistantAPIErrorMessage('daemon unreachable');
    expect(m.isError).toBe(true);
    expect(JSON.stringify(m.message.content)).toContain('daemon unreachable');
  });

  test('extractTag finds tagged content', () => {
    expect(extractTag('<think>abc</think>', 'think')).toBe('abc');
    expect(extractTag('no tags', 'think')).toBeNull();
  });

  test('getMessagesAfterCompactBoundary drops messages up to boundary', () => {
    const msgs = [
      createUserMessage('a'),
      { type: 'system', subtype: 'compact_boundary', uuid: 'b1' } as never,
      createUserMessage('after'),
    ];
    expect(getMessagesAfterCompactBoundary(msgs)).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/contracts/messages.test.ts`
Expected: FAIL — cannot resolve `../../contracts/messages.js`

- [ ] **Step 3: Implement contracts/messages.ts**

```ts
// src/contracts/messages.ts
/**
 * Brain-owned message vocabulary for the shell UI and adapter seam.
 * Wire-compatible with the shapes the Rust daemon streams (see AGENTS.md,
 * UDS Streaming Protocol) but owned here, not by any external codebase.
 */

export interface TextBlock { type: 'text'; text: string }
export interface ThinkingBlock { type: 'thinking'; thinking: string; signature?: string }
export interface RedactedThinkingBlock { type: 'redacted_thinking'; data: string }
export interface ToolUseBlock { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> }
export interface ToolResultBlock {
  type: 'tool_result';
  tool_use_id: string;
  content: string;
  is_error?: boolean;
}
export type ContentBlock = TextBlock | ThinkingBlock | RedactedThinkingBlock | ToolUseBlock | ToolResultBlock;

interface Envelope<B extends ContentBlock[]> { content: string | B }

export interface UserMessage { type: 'user'; uuid: string; timestamp: string; message: Envelope<[TextBlock, ToolResultBlock]> }
export interface AssistantMessage {
  type: 'assistant';
  uuid: string;
  timestamp: string;
  isError?: boolean;
  message: Envelope<[TextBlock, ThinkingBlock, RedactedThinkingBlock, ToolUseBlock]>;
}
export interface SystemMessage { type: 'system'; subtype: string; uuid: string; timestamp: string; data?: unknown }
export type Message = UserMessage | AssistantMessage | SystemMessage;

/** View-level stream events emitted by the typewriter pipeline (Inc 1 consumes these). */
export type StreamEvent =
  | { type: 'stream_start'; turnId: string }
  | { type: 'stream_progress'; turnId: string; seq: number }
  | { type: 'stream_chunk'; turnId: string; seq: number; delta: string }
  | { type: 'stream_end'; turnId: string }
  | { type: 'stream_cancelled'; turnId: string };

function uid(): string {
  return globalThis.crypto?.randomUUID?.() ?? `m_${Date.now()}_${Math.random().toString(36).slice(2)}`;
}

function now(): string {
  return new Date().toISOString();
}

export function createUserMessage(content: string): UserMessage {
  return { type: 'user', uuid: uid(), timestamp: now(), message: { content } };
}

export function createAssistantMessage(content: string): AssistantMessage {
  return {
    type: 'assistant', uuid: uid(), timestamp: now(),
    message: { content: [{ type: 'text', text: content }] },
  };
}

export function createAssistantAPIErrorMessage(error: string): AssistantMessage {
  return {
    type: 'assistant', uuid: uid(), timestamp: now(), isError: true,
    message: { content: [{ type: 'text', text: `Error: ${error}` }] },
  };
}

export function extractTag(text: string, tag: string): string | null {
  const m = new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`).exec(text);
  return m ? m[1] : null;
}

export function getMessagesAfterCompactBoundary(messages: readonly Message[]): Message[] {
  const idx = messages.findLastIndex((m) => m.type === 'system' && (m as SystemMessage).subtype === 'compact_boundary');
  return idx === -1 ? [...messages] : messages.slice(idx + 1);
}

/**
 * Fold a stream event into the transcript: text chunks append to the trailing
 * assistant message (creating one if needed); start/end/cancel are metadata-only.
 */
export function handleMessageFromStream(event: StreamEvent, messages: Message[]): Message[] {
  if (event.type !== 'stream_chunk') return messages;
  const last = messages.at(-1);
  if (last?.type === 'assistant' && !last.isError) {
    const blocks = last.message.content as ContentBlock[];
    const tail = blocks.at(-1);
    if (tail?.type === 'text') tail.text += event.delta;
    else blocks.push({ type: 'text', text: event.delta });
    return [...messages.slice(0, -1), { ...last, message: { content: blocks } }];
  }
  return [...messages, createAssistantMessage(event.delta)];
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/test/contracts/messages.test.ts`
Expected: PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
git add src/contracts/messages.ts src/test/contracts/messages.test.ts
git commit -m "feat(brain-shell): contracts/messages — Brain-owned message taxonomy

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: contracts/tools.ts + contracts/query.ts — tool & query-call seams

**Files:**
- Create: `src/contracts/tools.ts`, `src/contracts/query.ts`
- Test: `src/test/contracts/querySeam.test.ts`

**Interfaces:**
- Consumes: `createBrainCallModel` from `src/adapter/brainCallModel.ts` (exists)
- Produces: `Tool`, `ToolPermissionContext`, `ToolUseContext` (types); `QueryDeps` (type); `productionDeps` (value wired to adapter)

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/querySeam.test.ts
import { describe, expect, test } from 'bun:test';
import { productionDeps } from '../../contracts/query.js';

describe('contracts/query', () => {
  test('productionDeps exposes a callModel callable backed by the Brain adapter', () => {
    expect(typeof productionDeps.callModel).toBe('function');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/contracts/querySeam.test.ts`
Expected: FAIL — cannot resolve module

- [ ] **Step 3: Implement both contract files**

```ts
// src/contracts/tools.ts
/** Brain-owned tool-descriptor vocabulary (UI presentation + permission mapping input). */
export interface ToolPermissionContext {
  mode: 'default' | 'acceptEdits' | 'plan' | 'bypassPermissions';
  alwaysAllowRules: string[];
  alwaysDenyRules: string[];
}

export interface ToolUseContext {
  sessionId: string;
  workingDirectory: string;
  abortController?: AbortController;
}

export interface Tool<TInput = Record<string, unknown>> {
  name: string;
  description: string;
  inputSchema: TInput;
  isReadOnly(input: TInput): boolean;
  isConcurrencySafe(input: TInput): boolean;
}
```

```ts
// src/contracts/query.ts
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { BrainBackendClient } from '../client/BrainBackendClient.js';

/**
 * The single seam between the UI layer and Brain intelligence.
 * Inc 1 REPL loop calls deps.callModel; the daemon stays the only backend.
 */
export interface QueryDeps {
  callModel(input: { prompt: string; signal?: AbortSignal }): Promise<{ text: string }>;
}

let cached: QueryDeps | undefined;

export function getProductionDeps(client?: BrainBackendClient): QueryDeps {
  cached ??= {
    callModel: async ({ prompt }) => {
      const brain = createBrainCallModel(client ?? new BrainBackendClient());
      const result = await brain(prompt);
      return { text: result };
    },
  };
  return cached;
}

export const productionDeps: QueryDeps = new Proxy({} as QueryDeps, {
  get(_t, prop) {
    return getProductionDeps()[prop as keyof QueryDeps];
  },
});
```

> NOTE for executor: check `createBrainCallModel`'s real signature in `src/adapter/brainCallModel.ts:135` first (it takes `(client, sessionStore?, contextProvider?, toolFeedbackEmitter?)`) and whether it returns a callable or an object with `.call()`; adapt the wrapper to the real shape rather than changing the adapter.

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/test/contracts/querySeam.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/contracts/tools.ts src/contracts/query.ts src/test/contracts/querySeam.test.ts
git commit -m "feat(brain-shell): contracts/tools + query seam over Brain adapter

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: contracts/session.ts — session identity & cwd

Replaces `vendor bootstrap/state.js` + `utils/cwd.js` + `types/ids.js` consumers (`getSessionId`, `getOriginalCwd`, `getCwd`, `switchSession`, `asSessionId`, `getKairosActive`, `getUserMsgOptIn`).

Brain decisions: session identity lives in `BrainSessionStore`; kairos/user-msg opt-ins are Brain config flags defaulting to false (drop the exotic names — keep functions for source-compat during swap, marked deprecated).

**Files:**
- Create: `src/contracts/session.ts`
- Test: `src/test/contracts/session.test.ts`

**Interfaces:**
- Produces: `SessionId` (branded string), `asSessionId(v: string): SessionId`, `getSessionId(): SessionId`, `setSessionId(id: SessionId): void`, `getOriginalCwd(): string`, `getCwd(): string`, `switchSession(id: SessionId): void`, `getKairosActive(): boolean`, `getUserMsgOptIn(): boolean`

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/session.test.ts
import { describe, expect, test } from 'bun:test';
import { asSessionId, getCwd, getSessionId, getOriginalCwd, setSessionId, switchSession } from '../../contracts/session.js';

describe('contracts/session', () => {
  test('session id is stable and switchable', () => {
    const first = getSessionId();
    expect(first).toBe(asSessionId(first));
    switchSession(asSessionId('session-beta'));
    expect(getSessionId()).toBe('session-beta');
    setSessionId(asSessionId(first));
  });
  test('cwd getters return absolute paths', () => {
    expect(getCwd().startsWith('/')).toBe(true);
    expect(getOriginalCwd().startsWith('/')).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/contracts/session.test.ts` → Expected: FAIL (module missing)

- [ ] **Step 3: Implement**

```ts
// src/contracts/session.ts
const BRAND: unique symbol = Symbol('SessionId');
export type SessionId = string & { readonly [BRAND]: true };

export function asSessionId(value: string): SessionId {
  if (!value) throw new Error('SessionId must be non-empty');
  return value as SessionId;
}

let current = asSessionId(
  process.env.BRAIN_SESSION_ID ?? `ses_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`,
);
const originalCwd = process.cwd();

export function getSessionId(): SessionId { return current; }
export function setSessionId(id: SessionId): void { current = id; }
export function switchSession(id: SessionId): void { current = id; }
export function getOriginalCwd(): string { return originalCwd; }
export function getCwd(): string { return process.cwd(); }

/** Deprecated compat flags — Brain config owns real feature flags. */
export function getKairosActive(): boolean { return false; }
export function getUserMsgOptIn(): boolean { return false; }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/test/contracts/session.test.ts` → Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/contracts/session.ts src/test/contracts/session.test.ts
git commit -m "feat(brain-shell): contracts/session — session identity without vendor state

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: contracts/theme.ts + contracts/model.ts — appearance & model label

**Files:**
- Create: `src/contracts/theme.ts`, `src/contracts/model.ts`
- Test: `src/test/contracts/themeModel.test.ts`

**Interfaces:**
- Consumes: `BrainThemeTokens` from `src/adapter/BrainTheme.ts` (exists)
- Produces: `ThemeName`, `ThemeSetting`, `THEME_NAMES`, `SystemTheme`, `getSystemThemeName()`, `resolveThemeSetting(setting)`; `renderModelSetting(model: string): string`, `useMainLoopModel(): string`

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/themeModel.test.ts
import { renderHook } from 'bun-react-testing-library'; // if unavailable in repo, inline React render per existing suite pattern in src/test/theme_integration_brain.test.tsx
import { describe, expect, test } from 'bun:test';
import { THEME_NAMES, getSystemThemeName, renderModelSetting, resolveThemeSetting } from '../../contracts/theme.js';
import { useMainLoopModel } from '../../contracts/model.js';

describe('contracts/theme+model', () => {
  test('theme names include dark/light bases', () => {
    expect(THEME_NAMES).toContain('dark');
    expect(THEME_NAMES).toContain('light');
  });
  test('resolveThemeSetting resolves auto via system theme', () => {
    expect(resolveThemeSetting('auto')).toBe(getSystemThemeName());
    expect(resolveThemeSetting('dark')).toBe('dark');
  });
  test('model label renders without vendor branding', () => {
    expect(renderModelSetting('brain-default')).toContain('brain-default');
  });
});
```

If `bun-react-testing-library` is not present, replace the unused import line — this test file does not actually need hooks rendering (useMainLoopModel gets a plain unit test via React's `renderToString` from `react-dom/server` inside Ink-free scope, or defer hook test to Task 8's AppShell smoke). Keep the three pure-function tests above as the gate.

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/test/contracts/themeModel.test.ts` → Expected: FAIL (module missing)

- [ ] **Step 3: Implement**

```ts
// src/contracts/theme.ts
import * as React from 'react';

export type ThemeName = 'dark' | 'light' | 'dark-daltonized' | 'light-daltonized';
export type ThemeSetting = ThemeName | 'auto';
export const THEME_NAMES: ThemeName[] = ['dark', 'light', 'dark-daltonized', 'light-daltonized'];
export type SystemTheme = 'dark' | 'light';

export function getSystemThemeName(): SystemTheme {
  const g = globalThis as Record<string, unknown>;
  if (typeof g.__BRAIN_SYSTEM_THEME === 'string') return g.__BRAIN_SYSTEM_THEME as SystemTheme; // injected by preload AUTO_THEME
  const scheme = process.env.COLORFGBG ? Number(String(process.env.COLORFGBG).split(';').pop()) : undefined;
  return scheme !== undefined && scheme < 8 ? 'dark' : 'dark';
}

export function resolveThemeSetting(setting: ThemeSetting): ThemeName {
  if (setting !== 'auto') return setting;
  return getSystemThemeName();
}

/** Model-setting label for status display. Brain-neutral: no vendor names. */
export function renderModelSetting(model: string): string {
  return model;
}

export function useMainLoopModel(): string {
  const [model] = React.useState(() => process.env.BRAIN_MODEL ?? 'brain-default');
  return model;
}
```

(`src/contracts/model.ts` is intentionally folded into `theme.ts` exports? NO — keep separate file re-exporting for swap-path parity:)

```ts
// src/contracts/model.ts
export { useMainLoopModel, renderModelSetting } from './theme.js';
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/test/contracts/themeModel.test.ts` → Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/contracts/theme.ts src/contracts/model.ts src/test/contracts/themeModel.test.ts
git commit -m "feat(brain-shell): contracts/theme + model labels, Brain-neutral

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: contracts/commands.ts — command registry

**Files:**
- Create: `src/commands/registry.ts` (implementation lives beside existing `commands/`)
- Test: `src/test/contracts/commandRegistry.test.ts`

**Interfaces:**
- Consumes: nothing yet (existing 4 command files migrate here in Task 10 swaps)
- Produces: `Command` interface `{name, description, aliases?, argumentHint?, hidden?, handler(ctx)}`, `CommandContext`, `CommandResult` (`{type:'text',value}|{type:'none'}`), `registerCommand(cmd)`, `getCommands(): Command[]`, `getCommand(name): Command | undefined`

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/commandRegistry.test.ts
import { describe, expect, test } from 'bun:test';
import { getCommand, getCommands, registerCommand } from '../../commands/registry.js';

describe('contracts/commandRegistry', () => {
  test('registers and resolves by name and alias', () => {
    registerCommand({
      name: 'ping', description: 'responds pong',
      aliases: ['p'],
      handler: async () => ({ type: 'text', value: 'pong' }),
    });
    expect(getCommand('ping')?.description).toBe('responds pong');
    expect(getCommand('p')?.name).toBe('ping');
    expect(getCommands().map((c) => c.name)).toContain('ping');
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — `bun test src/test/contracts/commandRegistry.test.ts` → FAIL

- [ ] **Step 3: Implement**

```ts
// src/commands/registry.ts
export interface CommandResult {
  type: 'text' | 'none';
  value?: string;
}

export interface CommandContext {
  args: string[];
  sessionId: string;
}

export interface Command {
  name: string;
  description: string;
  aliases?: string[];
  argumentHint?: string;
  hidden?: boolean;
  supportsNonInteractive?: boolean;
  handler(ctx: CommandContext): Promise<CommandResult>;
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

- [ ] **Step 4: Run test to verify it passes** — `bun test src/test/contracts/commandRegistry.test.ts` → PASS

- [ ] **Step 5: Commit**

```bash
git add src/commands/registry.ts src/test/contracts/commandRegistry.test.ts
git commit -m "feat(brain-shell): command registry contract for slash surface

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: compat layer — ink re-export, useTerminalSize, text utils

**Files:**
- Create: `src/compat/ink.ts`, `src/compat/hooks.ts`, `src/compat/text.ts`
- Test: `src/test/contracts/compat.test.ts`

**Interfaces:**
- Consumes: npm `ink` package (already in dependencies)
- Produces: everything `from '<...>/vendor/claude/ink.js'` consumers use: `Box, Text, Key, Ansi(color), useInput, useTheme, usePreviewTheme, useTerminalFocus, useThemeSetting, createRoot, stringWidth`; plus `useTerminalSize(): {columns:number, rows:number}`; `truncatePath(p, max): string`, `wrapAnsi`, `ansiTokenize` passthroughs where consumed

- [ ] **Step 1: Write the failing test**

```ts
// src/test/contracts/compat.test.ts
import { describe, expect, test } from 'bun:test';
import { Box, Text, stringWidth } from '../../compat/ink.js';
import { truncatePath } from '../../compat/text.js';

describe('compat', () => {
  test('re-exports stock ink primitives', () => {
    expect(typeof Box).toBe('object');
    expect(typeof Text).toBe('function'); // ink's Text is a component fn (forwardRef object tolerated: assert non-null instead if this flakes)
    expect(stringWidth('héllo')).toBe(5);
  });
  test('truncatePath keeps tail visible', () => {
    const out = truncatePath('/Users/x/dev/brain/packages/brain-shell', 24);
    expect(out.length).toBeLessThanOrEqual(24);
    expect(out.endsWith('brain-shell')).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — `bun test src/test/contracts/compat.test.ts` → FAIL (modules missing)

- [ ] **Step 3: Implement**

```ts
// src/compat/ink.ts
/**
 * Single import point for terminal primitives. Stock MIT-licensed Ink.
 * Anything stock Ink lacks lands in ./hooks or ./text — never vendor paths.
 */
export {
  Box,
  Text,
  Static,
  useInput,
  useStdin,
  useStdout,
  useApp,
  render,
  useTheme as useInkTheme,
} from 'ink';
export type Key = Parameters<Parameters<typeof useInput>[0]>[0];
export { default as Ansi } from './AnsiText.js';      // tiny own ANSI-styled Text wrapper (Task step below)
export { usePreviewTheme, useTheme, useThemeSetting } from '../state/themeContext.js';
export { useTerminalFocus } from './focus.js';
export { createRoot } from './createRoot.js';
export { default as stringWidth } from 'string-width';
```

> Executor notes: (a) `string-width` may not be a dependency — check package.json; if absent implement locally: strip ANSI (`/\x1b\[[0-9;]*m/g`), strip zero-width, count code points east-asian-wide via a small table (accept ASCII-exact for Inc 0; document limitation at top of file). Prefer local impl to avoid new deps. (b) If `state/themeContext` doesn't exist yet, create minimal `src/state/themeContext.tsx` providing React context `{tokens: BrainThemeTokens, themeName}` seeded from `resolveThemeSetting(process.env.BRAIN_THEME ?? 'auto')` consuming `adapter/BrainTheme.ts` token maps — reuse whatever mapping exists there rather than inventing colors. (c) `./AnsiText`, `./focus.ts`, `./createRoot.ts`: write 5–15 line originals (createRoot delegates to ink's `render` returning `{unmount}`).

```ts
// src/compat/hooks.ts
import * as React from 'react';
import { useStdout } from './ink.js';

export function useTerminalSize(): { columns: number; rows: number } {
  const { stdout } = useStdout();
  const read = (): { columns: number; rows: number } => ({
    columns: stdout.columns ?? 80,
    rows: stdout.rows ?? 24,
  });
  const [size, setSize] = React.useState(read);
  React.useEffect(() => {
    const onResize = () => setSize(read());
    stdout.on('resize', onResize);
    return () => { stdout.off('resize', onResize); };
  }, [stdout]);
  return size;
}
```

```ts
// src/compat/text.ts
export function truncatePath(path: string, max: number): string {
  if (path.length <= max) return path;
  const parts = path.split('/');
  let out = parts.at(-1)!;
  for (let i = parts.length - 2; i >= 0; i--) {
    const next = `${parts[i]}/${out}`;
    if (next.length > max - 1) break; // reserve 1 col for ellipsis
    out = next;
  }
  return `…/${out}`.slice(-max);
}
export const wrapAnsi = (text: string, cols: number): string => text; // full wrap lands with markdown renderer (Inc 1); consumers in keeper layers don't call it
export { default as ansiTokenize } from '@alcalzone/ansi-tokenize';
```

> `@alcalzone/ansi-tokenize` is NOT currently a dependency of brain-shell — if absent, drop that export line and delete consumers in Task 10 (they are shim/tests).

- [ ] **Step 4: Run test to verify it passes** — `bun test src/test/contracts/compat.test.ts` → PASS (if the `typeof Text` assertion flakes because Ink exports a forwardRef object, relax to `expect(Text).toBeDefined()`)

- [ ] **Step 5: Commit**

```bash
git add src/compat/ src/state/themeContext.tsx src/test/contracts/compat.test.ts
git commit -m "feat(brain-shell): compat layer — stock-ink re-export, size hook, text utils

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: ui/shell placeholders — BrainMark logo & mode indicator

Keeper components `CanonicalWorkspace.tsx` / `CanonicalPrompt.tsx` import `Clawd` and `PromptInputModeIndicator` from vendor. Provide Brain originals.

**Files:**
- Create: `src/ui/shell/BrainMark.tsx`, `src/ui/shell/PromptModeIndicator.tsx`
- Test: extend `src/test/contracts/compat.test.ts`

**Interfaces:**
- Produces: `<BrainMark />` (small ASCII mark, ≤5 rows), `<PromptModeIndicator mode={'prompt'|'bash'} />` rendering `!` badge for bash mode

- [ ] **Step 1: Write the failing test**

```ts
// appended to src/test/contracts/compat.test.tsx   (note .tsx extension for JSX)
import { render } from '../../compat/ink.js'; // ink's render captures frames off-TTY
import * as React from 'react';
import { BrainMark } from '../../ui/shell/BrainMark.js';
import { PromptModeIndicator } from '../../ui/shell/PromptModeIndicator.js';

describe('ui/shell placeholders', () => {
  test('BrainMark renders non-empty and mentions nothing proprietary', () => {
    const app = render(React.createElement(BrainMark));
    const frame = app.lastFrame() ?? '';
    app.unmount();
    expect(frame.toLowerCase()).not.toContain('claude');
    expect(frame).toContain('BRAIN');
  });
  test('mode indicator shows bash prefix only in bash mode', () => {
    const bashApp = render(React.createElement(PromptModeIndicator, { mode: 'bash' }));
    const bashFrame = bashApp.lastFrame() ?? '';
    bashApp.unmount();
    expect(bashFrame).toContain('!');
    const promptApp = render(React.createElement(PromptModeIndicator, { mode: 'prompt' }));
    const promptFrame = promptApp.lastFrame() ?? '';
    promptApp.unmount();
    expect(promptFrame).not.toContain('!');
  });
});
```

(Ink components require Ink's reconciler — never assert on them via `react-dom/server`. Rename the file if it was created as `.ts` in Task 7.)

- [ ] **Step 2: Verify fail** — run → FAIL (modules missing)

- [ ] **Step 3: Implement**

```tsx
// src/ui/shell/BrainMark.tsx
import * as React from 'react';
import { Box, Text } from '../../compat/ink.js';

export function BrainMark(): React.ReactElement {
  return (
    <Box flexDirection="column">
      <Text bold color="magenta">◆ BRAIN</Text>
      <Text dimColor>memory-first agent workspace</Text>
    </Box>
  );
}
```

```tsx
// src/ui/shell/PromptModeIndicator.tsx
import * as React from 'react';
import { Text } from '../../compat/ink.js';

export function PromptModeIndicator({ mode }: { mode: 'prompt' | 'bash' }): React.ReactElement {
  return mode === 'bash'
    ? <Text bold color="yellow">! bash</Text>
    : <></>;
}
```

- [ ] **Step 4: Verify pass** → PASS · **Step 5: Commit**

```bash
git add src/ui/shell/ src/test/contracts/
git commit -m "feat(brain-shell): ui/shell BrainMark + mode indicator placeholders

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 9: Swap keeper layers off vendor (adapter, commands, components)

Mechanical specifier replacement + two semantic fixes. After this task, `grep -r vendor src/adapter src/commands src/components` returns ZERO matches.

**Files (Modify):**
- `src/adapter/brainCallModel.ts`, `src/adapter/BrainConfigStore.ts`, `src/adapter/BrainTheme.ts`
- `src/commands/config/ConfigCommand.tsx`, `src/commands/doctor/DoctorCommand.tsx`, `src/commands/memory/MemoryCommand.tsx`
- `src/components/*.tsx` (all 9)

**Interfaces:**
- Consumes: Tasks 2–8 outputs
- Produces: keeper layers vendor-free

- [ ] **Step 1: Codemod the uniform specifiers**

Run from `packages/brain-shell`:

```bash
files=$(grep -rlE "vendor/claude/(ink|hooks/useTerminalSize|types/ids|bootstrap/state|utils/cwd|utils/logoV2Utils)\.js" src/adapter src/commands src/components)
sed -i '' \
  -e "s#from '[^']*vendor/claude/ink.js'#from '../compat/ink.js'#g;s#from \"[^\"]*vendor/claude/ink.js\"#from '../compat/ink.js'#g" \
  -e "s#from '[^']*vendor/claude/hooks/useTerminalSize.js'#from '../compat/hooks.js'#g;s#from \"[^\"]*vendor/claude/hooks/useTerminalSize.js\"#from '../compat/hooks.js'#g" \
  -e "s#from '[^']*vendor/claude/types/ids.js'#from '../contracts/session.js'#g" \
  -e "s#from '[^']*vendor/claude/bootstrap/state.js'#from '../contracts/session.js'#g" \
  -e "s#from '[^']*vendor/claude/utils/cwd.js'#from '../contracts/session.js'#g" \
  -e "s#import { switchSession as switchClaudeSession }#import { switchSession }#g" \
  $files
# depth-3 variants (commands/*/*) need ../../ — fix with a second pass:
sed -i '' "s#from '\.\./compat/#from '../../compat/#g;s#from '\.\./contracts/#from '../../contracts/#g" \
  $(grep -rl "compat/\|contracts/" src/commands/config src/commands/doctor src/commands/memory)
```

Then hand-fix the four non-uniform imports:

- `brainCallModel.ts`: `createAssistantMessage/createAssistantAPIErrorMessage` ← `../contracts/messages.js`; `type {Message}` ← `../contracts/messages.js`; `type {QueryDeps}` ← `../contracts/query.js`; `type {Tool}` ← `../contracts/tools.js`; `ThinkingConfig` — define locally: `export interface ThinkingConfig { maxTokens?: number }` in `contracts/tools.ts` (append) and import from there; **replace line 128 fallback string** `'You are Claude Code, hosted in the Brain relational shell.'` → `'You are Brain, the memory-first agent runtime.'`
- `BrainConfigStore.ts`: `type {ThemeSetting}` ← `../contracts/theme.js`
- `BrainTheme.ts`: `resolveThemeSetting` ← `../contracts/theme.js` (delete the vendor systemTheme import; keep BrainTheme's own token maps untouched)
- `BrainWorkspaceDashboard.tsx`: `renderModelSetting` ← `../contracts/model.js`; `truncatePath` ← `../compat/text.js`; `switchSession` ← `../contracts/session.js`; `useMainLoopModel` ← `../contracts/model.js`
- `CanonicalPrompt.tsx` / `CanonicalWorkspace.tsx`: `PromptInputModeIndicator` ← `../ui/shell/PromptModeIndicator.js` (rename usages or export alias `export { PromptModeIndicator as PromptInputModeIndicator }`); `Clawd` ← `<BrainMark />` from `../ui/shell/BrainMark.js` (swap the JSX element; adjust surrounding props if the old component took none)

- [ ] **Step 2: Prove zero vendor imports in keepers**

Run: `grep -rE "vendor/claude" src/adapter src/commands src/components | wc -l`
Expected: `0`

- [ ] **Step 3: Run keeper-layer tests (baseline-known failures allowed)**

Run: `bun test src/test/theme_integration_brain.test.tsx src/test/udsTransportAdapter.test.ts 2>&1 | tail -3`
Expected: no NEW failures beyond the recorded baseline list; import errors would show as resolution failures — those are regressions, fix before proceeding.

- [ ] **Step 4: Commit**

```bash
git add src/adapter src/commands src/components src/contracts/tools.ts
git commit -m "refactor(brain-shell): keeper layers decoupled from vendor tree

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 10: Shim & test triage — delete Anthropic-specific, swap the rest

Of 45 shims + 31 importing test files, classify: **DELETE** (wraps CC product surfaces: tools/*, LogoV2 upsells/feedConfigs, claudeApiShim, claudeForChromeMcp, api-key verification, analytics/growthbook, autoUpdater, releaseNotes, Opus1mMergeNotice, GuestPass/Overage upsells, REPL shim, PromptInputFooter*, select/Tabs design-system shims, vim shims, permissions-rule UI shims…) vs **KEEP+SWAP** (Brain-original logic: brainQuery*, memory*/doctor*/permissionsCommand/resumeCommand/ThemePicker core, OffscreenFreeze, StatusNotices, UserPromptMessage, UserCommandMessage, UserLocalCommandOutputMessage, ListItem, LogSelector, HighlightedThinkingText, colorDiff, commandSuggestions, ShellCommand, ansiTokenize/wrapAnsi/logUpdate utils, useTextInput/useVimInput→defer-with-file-if-blocked).

**Files:** Delete: computed list below. Modify: keep-list import specifiers.

**Interfaces:** Consumes Tasks 2–8. Produces: `src/shims` reduced to Brain-relevant modules importing only contracts/compat; test suite compiling without vendor.

- [ ] **Step 1: Compute the doomed set deterministically**

```bash
cat > /tmp/triage.sh <<'EOF'
#!/bin/bash
cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell
# Modules we now provide locally:
PROVIDED="ink\.js|hooks/useTerminalSize|types/message|utils/messages|Tool|query/deps|query\.js|commands\.js|bootstrap/state|types/ids|utils/cwd|utils/systemTheme|utils/theme|utils/thinking|utils/model/model|hooks/useMainLoopModel|logoV2Utils|textInputTypes"
# Everything else a file imports from vendor ⇒ file is doomed unless listed in KEEP_EXTRA.
KEEP_EXTRA="src/shims/(OffscreenFreeze|StatusNotices|ListItem|LogSelector|HighlightedThinkingText|colorDiff|commandSuggestions|ShellCommand|memory|memoryCommand|doctorCommand|doctorSkill|permissionsCommand|resumeCommand|ThemePicker|ThemeProvider|UserPromptMessage|UserCommandMessage|UserLocalCommandOutputMessage|useTextInput|useVimInput|wrapAnsi|logUpdate|ansiTokenize)\."
for f in $(grep -rl "vendor/claude" src/shims src/test); do
  mods=$(grep -hoE "vendor/claude/[A-Za-z0-9/_.-]+" "$f" | sed 's/vendor\/claude\///' | sort -u)
  unprovided=0
  for m in $mods; do echo "$m" | grep -qE "^($PROVIDED)$" || unprovided=1; done
  if [ $unprovided -eq 1 ] && ! echo "$f" | grep -qE "$KEEP_EXTRA"; then echo "$f"; fi
done
EOF
chmod +x /tmp/triage.sh && /tmp/triage.sh | tee /tmp/doomed.txt | wc -l
```

- [ ] **Step 2: Review the doomed list against the KEEP rules above**

Every file in `/tmp/doomed.txt` must be either (a) deleted via `git rm`, or (b) moved to a written keep-list line added to `KEEP_EXTRA` with one-line justification committed into `docs/superpowers/plans/inc0-triage-notes.md`. Files testing removed CC features die WITH their features — do not stub them to keep counts up.

- [ ] **Step 3: Execute deletions and swap survivors**

```bash
git rm $(cat /tmp/doomed.txt)
# Survivors: same codemod pairs as Task 9 Step 1 (both depth-2 ../.. and depth-3 ../../../ variants):
for f in $(grep -rl "vendor/claude" src/shims src/test); do
  sed -i '' \
    -e "s#\.\./\.\./vendor/claude/ink\.js#\.\./compat/ink.js#g" \
    -e "s#\.\./\.\./\.\./vendor/claude/ink\.js#\.\./\.\./compat/ink.js#g" \
    -e "s#\.\./\.\./vendor/claude/hooks/useTerminalSize\.js#\.\./compat/hooks.js#g" \
    -e "s#\.\./\.\./vendor/claude/types/message\.js#\.\./contracts/messages.js#g" \
    -e "s#\.\./\.\./\.\./vendor/claude/types/message\.js#\.\./\.\./contracts/messages.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/messages\.js#\.\./contracts/messages.js#g" \
    -e "s#\.\./\.\./vendor/claude/Tool\.js#\.\./contracts/tools.js#g" \
    -e "s#\.\./\.\./vendor/claude/query/deps\.js#\.\./contracts/query.js#g" \
    -e "s#\.\./\.\./vendor/claude/query\.js#\.\./contracts/query.js#g" \
    -e "s#\.\./\.\./vendor/claude/commands\.js#\.\./commands/registry.js#g" \
    -e "s#\.\./\.\./vendor/claude/bootstrap/state\.js#\.\./contracts/session.js#g" \
    -e "s#\.\./\.\./vendor/claude/types/ids\.js#\.\./contracts/session.js#g" \
    -e "s#\.\./\.\./vendor/claude/types/textInputTypes\.js#\.\./contracts/input.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/cwd\.js#\.\./contracts/session.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/theme\.js#\.\./contracts/theme.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/systemTheme\.js#\.\./contracts/theme.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/thinking\.js#\.\./contracts/input.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/model/model\.js#\.\./contracts/model.js#g" \
    -e "s#\.\./\.\./vendor/claude/hooks/useMainLoopModel\.js#\.\./contracts/model.js#g" \
    -e "s#\.\./\.\./vendor/claude/utils/logoV2Utils\.js#\.\./compat/text.js#g" \
    "$f"; done
```

Create `src/contracts/input.ts` if not present from earlier tasks:

```ts
// src/contracts/input.ts
export type PromptInputMode = 'prompt' | 'bash';
export type VimMode = 'INSERT' | 'NORMAL';
export interface VimInputState { mode: VimMode; pending?: string }
export interface ThinkingConfig { maxTokens?: number }
```

Then run `grep -rl "vendor/claude" src/` — any stragglers: apply the same pair-swap by hand or delete the file if its remaining imports are all doomed-module consumers.

- [ ] **Step 4: Compile-and-test sweep**

Run: `bun test 2>&1 | tail -4`
Expected: suite RUNS (no module-resolution errors). Pass count ≥ baseline-pass minus deleted suites' passes; failures ⊆ baseline-failing set. Record numbers.

- [ ] **Step 5: Commit**

```bash
git add -u src/shims src/test src/contracts 2>/dev/null; git add docs/superpowers/plans/inc0-triage-notes.md
git commit -m "refactor(brain-shell): triage shims/tests — drop CC-product wrappers, swap survivors to contracts

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 11: Entrypoint swap — skeleton AppShell boots Brain shell

**Files:**
- Modify: `src/main.tsx`, `src/preload.ts`
- Create: `src/ui/shell/AppSkeleton.tsx`
- Modify: `package.json` (`start` script unchanged path-wise)

**Interfaces:** Consumes compat/contracts. Produces: `main()` launching `<AppSkeleton/>` via ink `render`; PTY-visible frame.

- [ ] **Step 1: Implement AppSkeleton**

```tsx
// src/ui/shell/AppSkeleton.tsx
import * as React from 'react';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { BrainMark } from './BrainMark.js';
import { useMainLoopModel } from '../../contracts/model.js';

export function AppSkeleton(): React.ReactElement {
  const { columns } = useTerminalSize();
  const model = useMainLoopModel(); // hoisted — hooks never inside JSX
  return (
    <Box flexDirection="column" width={columns} borderStyle="round">
      <BrainMark />
      <Box marginTop={1}>
        <Text>› </Text><Text dimColor>composer arrives in increment 1</Text>
      </Box>
      <Box marginTop={1}><Text dimColor>model: {model} · ctrl+c exit</Text></Box>
    </Box>
  );
}
```

(Create `src/compat/index.ts` re-exporting `./ink.js` + `./hooks.js` + `./text.js`.)

```ts
// src/main.tsx
import { render } from './compat/ink.js';
import * as React from 'react';
import { AppSkeleton } from './ui/shell/AppSkeleton.js';

export async function main(): Promise<void> {
  const app = render(React.createElement(AppSkeleton), { patchConsole: false });
  process.on('SIGINT', () => { app.unmount(); process.exit(0); });
}
await main();
```

Trim `preload.ts`: keep ONLY the `__BRAIN_PRELOAD_LOADED` guard + `AUTO_THEME` system-theme detection writing `globalThis.__BRAIN_SYSTEM_THEME`; delete any preload lines referencing vendor paths.

- [ ] **Step 2: Bundle resolves without vendor**

Run: `bun build src/main.tsx --target=bun --outdir="$CLAUDE_JOB_DIR/tmp/cc-src/build-check2" 2>&1 | tail -3`
Expected: success, ZERO references to `vendor/` in output listing.

- [ ] **Step 3: Commit**

```bash
git add src/main.tsx src/preload.ts src/ui/shell/AppSkeleton.tsx src/compat/index.ts
git commit -m "feat(brain-shell): Brain-owned entrypoint boots skeleton shell

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 12: Delete vendor, stale dirs, final verification gates

- [ ] **Step 1: Final no-vendor proof**

Run: `grep -rE "vendor/claude" src bunfig.toml package.json tsconfig.json | grep -v Binary | wc -l` → Expected: `0`

- [ ] **Step 2: Remove the tree and stale scaffolding**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git rm -r -q packages/brain-shell/vendor
git rm -rq packages/brain-shell/src/transport packages/brain-shell/src/stores packages/brain-shell/src/model 2>/dev/null || rmdir packages/brain-shell/src/transport packages/brain-shell/src/stores packages/brain-shell/src/model 2>/dev/null
```

- [ ] **Step 3: Full gates**

```bash
cd packages/brain-shell
bun test 2>&1 | tail -4        # Gate A: runs green-ish per Global Constraints baseline rule
bun build src/main.tsx --target=bun >/dev/null && echo BUILD_OK   # Gate B
python3 - <<'EOF'              # Gate C: PTY smoke of skeleton frame
import pty, os, sys, time
pid, fd = pty.fork()
if pid == 0:
    os.execvp("bun", ["bun", "run", "src/main.tsx"])
time.sleep(3)
try:
    out = os.read(fd, 65536).decode(errors='ignore')
except OSError:
    out = ''
os.write(fd, b'\x03')
print('PTY_OK' if ('BRAIN' in out and 'increment 1' in out) else 'PTY_FAIL:\n' + out[:2000])
EOF
```
Expected: Gate A per baseline rule; Gate B prints `BUILD_OK`; Gate C prints `PTY_OK`.

- [ ] **Step 4: Commit**

```bash
cd /Users/ritikpathania/Developer/PyCharm/brain
git commit -q -m "chore(brain-shell)!: remove vendored Claude Code tree — shell stands on owned contracts

Increment 0 complete: packages/brain-shell compiles, tests, and boots
without vendor/claude. Presentation rebuild continues in increment 1.

Co-Authored-By: Claude <noreply@anthropic.com>" -- packages/brain-shell/vendor packages/brain-shell/src/transport packages/brain-shell/src/stores packages/brain-shell/src/model
git log --oneline -1
```

---

## Completion criteria for Increment 0

1. `grep -r "vendor/claude" packages/brain-shell/src` → 0 matches; `vendor/` gone from HEAD.
2. `bun build src/main.tsx` succeeds; PTY smoke renders the Brain skeleton frame.
3. `bun test` behaves per the baseline rule (no NEW failures).
4. Every commit contains only explicitly-added paths.
