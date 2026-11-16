import { describe, expect, test } from 'bun:test';
import { renderToString } from 'ink';

import { BrainTerminalScreen, dispatchTerminalKey } from '../../src/presentation/terminal/app.js';
import { defaultBrainTheme } from '../../src/presentation/theme.js';
import type { FrontendInteractionSnapshot } from '../../src/state/interactions.js';
import { toTerminalModel } from '../../src/presentation/models.js';

describe('Brain terminal host', () => {
  test('renders a conversation-first composer and session overlay without transport details', () => {
    const output = renderToString(
      <BrainTerminalScreen
        model={state({
          sessionPickerOpen: true,
          focus: 'session-picker',
          sessions: [{ sessionId: 'opaque-session', title: 'Research', messageCount: 2, pinned: false, archived: false }],
          transcript: [{ kind: 'assistant', id: 'a1', text: 'Loaded from Brain', status: 'complete' }],
        })}
        theme={defaultBrainTheme}
      />,
    );

    expect(output).toContain('Loaded from Brain');
    expect(output).toContain('Sessions');
    expect(output).toContain('Research');
    expect(output).toContain('Enter to send');
    expect(output).not.toContain('session_id');
    expect(output).not.toContain('sequence');
  });

  test('routes typing, newline, submit, cancellation, and picker focus with explicit precedence', async () => {
    const calls: string[] = [];
    const controller = {
      appendDraft: (input: string) => calls.push(`append:${input}`),
      deleteDraftCharacter: () => calls.push('delete'),
      insertNewline: () => calls.push('newline'),
      submit: async () => { calls.push('submit'); return 'submitted' as const; },
      cancel: async () => { calls.push('cancel'); return 'requested' as const; },
      openSessionPicker: async () => { calls.push('open-picker'); },
      closeSessionPicker: () => calls.push('close-picker'),
      moveSessionSelection: () => calls.push('move-picker'),
      selectHighlightedSession: async () => { calls.push('select-picker'); },
      scrollUp: () => calls.push('scroll-up'),
      scrollToBottom: () => calls.push('scroll-bottom'),
      moveComposerCursor: (direction: string) => calls.push(`cursor:${direction}`),
      resolvePermission: async (decision: 'allow' | 'deny') => { calls.push(`permission:${decision}`); return 'submitted' as const; },
      closeContext: () => calls.push('close-context'),
      toggleToolDetail: () => calls.push('toggle-tool-detail'),
    };

    await dispatchTerminalKey(key({ input: 'h' }), state(), controller);
    await dispatchTerminalKey(key({ return: true, shift: true }), state(), controller);
    await dispatchTerminalKey(key({ return: true }), state(), controller);
    await dispatchTerminalKey(key({ input: 'c', ctrl: true }), state({ generating: true }), controller);
    await dispatchTerminalKey(key({ input: 'o', ctrl: true }), state(), controller);
    await dispatchTerminalKey(key({ escape: true }), state({ focus: 'session-picker', sessionPickerOpen: true }), controller);

    expect(calls).toEqual(['append:h', 'newline', 'submit', 'cancel', 'open-picker', 'close-picker']);
  });

  test('keeps the composer visible in a resized terminal and preserves the scroll anchor', () => {
    const model = state({
      composer: { draft: 'draft survives resize', cursor: 21 },
      transcript: [
        { kind: 'user', id: 'old', text: 'Earlier question' },
        { kind: 'assistant', id: 'stream', text: 'stream output\n'.repeat(60), status: 'streaming' },
      ],
      scrollOffset: 1,
      followTranscript: false,
    });
    const projectedBefore = toTerminalModel(model).entries;
    const narrow = renderToString(<BrainTerminalScreen model={model} theme={defaultBrainTheme} rows={14} columns={50} />);
    const wide = renderToString(<BrainTerminalScreen model={model} theme={defaultBrainTheme} rows={30} columns={100} />);
    expect(narrow).toContain('draft survives resize');
    expect(wide).toContain('draft survives resize');
    expect(narrow).not.toContain('stream output');
    expect(toTerminalModel(model).entries).toEqual(projectedBefore);
    expect(model.focus).toBe('composer');
  });

  test('bounds a long wrapped draft and keeps its cursor visible in a compact terminal', () => {
    const draft = `${'x'.repeat(400)}END`;
    for (const sessionPickerOpen of [false, true]) {
      for (const rows of [8, 14, 18]) {
        const model = state({
          composer: { draft, cursor: draft.length }, sessionPickerOpen,
          sessions: Array.from({ length: 10 }, (_, index) => ({ sessionId: `session-${index}`, title: 'Long title '.repeat(20), messageCount: index, pinned: false, archived: false })),
        });
        const output = renderToString(<BrainTerminalScreen model={model} theme={defaultBrainTheme} rows={rows} columns={55} />);
        expect(output.split('\n').length).toBeLessThanOrEqual(rows);
        if (!sessionPickerOpen || rows >= 14) expect(output).toContain('END');
        expect(output).toContain('Connected');
        expect(model.composer.draft).toBe(draft);
      }
    }
  });
});

function state(change: Partial<FrontendInteractionSnapshot> = {}): FrontendInteractionSnapshot {
  return {
    connection: 'ready',
    sessions: [],
    transcript: [],
    composer: { draft: '', cursor: 0 },
    generating: false,
    cancelling: false,
    focus: 'composer',
    sessionPickerOpen: false,
    selectedSessionIndex: 0,
    followTranscript: true,
    scrollOffset: 0,
    ...change,
  };
}

function key(change: Partial<Parameters<typeof dispatchTerminalKey>[0]>): Parameters<typeof dispatchTerminalKey>[0] {
  return {
    input: '',
    return: false,
    escape: false,
    ctrl: false,
    shift: false,
    pageUp: false,
    pageDown: false,
    upArrow: false,
    downArrow: false,
    backspace: false,
    ...change,
  };
}
