import { expect, test } from 'bun:test';
import { renderToString } from 'ink';
import { Transcript } from '../../src/presentation/components/transcript.js';
import { BrainTerminalScreen } from '../../src/presentation/terminal/app.js';
import { defaultBrainTheme } from '../../src/presentation/theme.js';
import { routeTerminalKey, type TerminalKey } from '../../src/presentation/input/keyRouter.js';
import type { FrontendInteractionSnapshot } from '../../src/state/interactions.js';

const key = (extra: Partial<TerminalKey>): TerminalKey => ({ input: '', return: false, escape: false, ctrl: false, shift: false, pageUp: false, pageDown: false, upArrow: false, downArrow: false, backspace: false, ...extra });
const state = (extra: Partial<FrontendInteractionSnapshot>): FrontendInteractionSnapshot => ({ connection: 'ready', sessions: [], transcript: [], composer: { draft: 'draft', cursor: 5 }, generating: false, cancelling: false, focus: 'composer', sessionPickerOpen: false, selectedSessionIndex: 0, followTranscript: true, scrollOffset: 0, ...extra });

test('permission input has priority without implicit Enter approval or composer leakage', () => {
  const context = { focus: 'permission' as const, generating: true };
  expect(routeTerminalKey(key({ input: 'y' }), context)).toBe('permission-allow');
  expect(routeTerminalKey(key({ input: 'n' }), context)).toBe('permission-deny');
  expect(routeTerminalKey(key({ escape: true }), context)).toBe('permission-deny');
  expect(routeTerminalKey(key({ return: true }), context)).toBe('ignore');
  expect(routeTerminalKey(key({ input: 'text' }), context)).toBe('ignore');
  expect(routeTerminalKey(key({ input: 'c', ctrl: true }), context)).toBe('cancel');
  expect(routeTerminalKey(key({ pageUp: true }), context)).toBe('scroll-up');
  expect(routeTerminalKey(key({ input: 't', ctrl: true }), context)).toBe('ignore');
});

test('context dismisses explicitly and detail expansion never enters composer text', () => {
  expect(routeTerminalKey(key({ escape: true }), { focus: 'context', generating: false })).toBe('dismiss-context');
  expect(routeTerminalKey(key({ return: true }), { focus: 'context', generating: false })).toBe('dismiss-context');
  expect(routeTerminalKey(key({ input: 'y' }), { focus: 'context', generating: false })).toBe('ignore');
  expect(routeTerminalKey(key({ input: 't', ctrl: true }), { focus: 'composer', generating: false })).toBe('toggle-tool-detail');
});

test('compact textual tool states reveal only deliberately expanded safe details', () => {
  const collapsed = renderToString(<Transcript theme={defaultBrainTheme} entries={[
    { kind: 'tool', id: 'call', label: 'bash', status: 'running', summary: 'Approved · awaiting Brain outcome', detail: 'safe output', expanded: false },
    { kind: 'tool', id: 'failed', label: 'bash', status: 'timed_out', summary: 'Brain reported a tool timeout' },
  ]} />);
  expect(collapsed).toContain('awaiting Brain outcome');
  expect(collapsed).toContain('timed out');
  expect(collapsed).not.toContain('safe output');
  const expanded = renderToString(<Transcript theme={defaultBrainTheme} entries={[
    { kind: 'tool', id: 'call', label: 'bash', status: 'succeeded', detail: 'safe output', expanded: true },
    { kind: 'permission', id: 'p', label: 'bash', status: 'allow_submitted' },
  ]} />);
  expect(expanded).toContain('safe output');
  expect(expanded).toContain('Allow submitted');
});

test('permission and quiet context remain readable at compact sizes without rendering correlation IDs', () => {
  for (const rows of [8, 14, 24]) {
    const permission = renderToString(<BrainTerminalScreen theme={defaultBrainTheme} rows={rows} columns={60} model={state({ focus: 'permission', generating: true, permission: { requestId: 'secret-id', label: 'bash', summary: 'printf hello' } })} />);
    expect(permission).toContain('Permission required');
    expect(permission).toContain('y allow');
    expect(permission).toContain('n / Esc deny');
    expect(permission).not.toContain('secret-id');
    expect(permission.split('\n').length).toBeLessThanOrEqual(rows);
  }
  const summary = renderToString(<BrainTerminalScreen theme={defaultBrainTheme} model={state({ focus: 'context', contextSummary: { retrievalCount: 1, contextTokenCount: 24 } })} />);
  expect(summary).toContain('Latest response used 1 retrieved source · 24 context tokens');
  const empty = renderToString(<BrainTerminalScreen theme={defaultBrainTheme} model={state({ focus: 'context' })} />);
  expect(empty).toContain('No retrieved context for this session');
  const recovering = renderToString(<BrainTerminalScreen theme={defaultBrainTheme} model={state({ connection: 'reconnecting', transcript: [{ kind: 'tool', id: 'old', label: 'bash', status: 'interrupted', summary: 'Outcome unknown' }] })} />);
  expect(recovering).toContain('Reconnecting');
  expect(recovering).toContain('interrupted');
  expect(recovering).not.toContain('Permission required');
});
