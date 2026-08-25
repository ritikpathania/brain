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

describe('sessionToRows: persisted tool events (Inc 9)', () => {
  const envelope = (over: Record<string, unknown>) =>
    JSON.stringify({ type: 'tool_event', v: 1, ...over });

  test('executed successful event becomes a completed frozen tool card', () => {
    const rows = sessionToRows(
      session([
        {
          id: 'm1',
          role: 'tool',
          content: envelope({
            call_id: 'c1',
            name: 'bash',
            input: { command: 'echo hi' },
            outcome: 'executed',
            is_error: false,
            exit_code: 0,
            output: 'hi\n',
            duration_ms: 12,
          }),
        },
      ]),
    );
    expect(rows).toEqual([
      {
        kind: 'tool',
        id: 'm1',
        tool: {
          callId: 'c1',
          toolName: 'bash',
          input: { command: 'echo hi' },
          status: 'completed',
          output: 'hi\n',
          isError: false,
          durationMs: 12,
          exitCode: 0,
        },
      },
    ]);
  });

  test('executed failing event keeps error details for the failed card', () => {
    const rows = sessionToRows(
      session([
        {
          id: 'm2',
          role: 'tool',
          content: envelope({
            call_id: 'c2',
            name: 'bash',
            input: { command: 'false' },
            outcome: 'executed',
            is_error: true,
            exit_code: 2,
            output: 'boom',
            duration_ms: 5,
          }),
        },
      ]),
    );
    expect(rows).toEqual([
      {
        kind: 'tool',
        id: 'm2',
        tool: {
          callId: 'c2',
          toolName: 'bash',
          input: { command: 'false' },
          status: 'failed',
          output: 'boom',
          isError: true,
          durationMs: 5,
          exitCode: 2,
        },
      },
    ]);
  });

  test('denied event becomes a denied card without execution fields', () => {
    const rows = sessionToRows(
      session([
        {
          id: 'm3',
          role: 'tool',
          content: envelope({
            call_id: 'c3',
            name: 'bash',
            input: { command: 'rm -rf /' },
            outcome: 'denied',
          }),
        },
      ]),
    );
    expect(rows).toEqual([
      {
        kind: 'tool',
        id: 'm3',
        tool: {
          callId: 'c3',
          toolName: 'bash',
          input: { command: 'rm -rf /' },
          status: 'denied',
        },
      },
    ]);
  });

  test('non-envelope tool content falls back to a system row with raw text', () => {
    const raw = JSON.stringify({ foo: 1 });
    const rows = sessionToRows(
      session([
        { id: 'm4', role: 'tool', content: 'not json at all' },
        { id: 'm5', role: 'tool', content: raw },
        { id: 'm6', role: 'tool', content: JSON.stringify({ type: 'other', v: 1 }) },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'system', id: 'm4', text: 'not json at all' },
      { kind: 'system', id: 'm5', text: raw },
      { kind: 'system', id: 'm6', text: JSON.stringify({ type: 'other', v: 1 }) },
    ]);
  });
});

describe('sessionToRows: persisted thinking blocks (Inc 19)', () => {
  const thinkingEnvelope = (over: Record<string, unknown>) =>
    JSON.stringify({ type: 'thinking_block', v: 1, ...over });

  test('valid envelope becomes a collapsed thinking row keeping text and duration', () => {
    const rows = sessionToRows(
      session([
        { id: 'u1', role: 'user', content: 'hello' },
        {
          id: 't1',
          role: 'thinking',
          content: thinkingEnvelope({ text: 'secret reasoning', duration_ms: 800 }),
        },
        { id: 'a1', role: 'assistant', content: 'answer' },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'user', id: 'u1', text: 'hello' },
      { kind: 'thinking', id: 't1', text: 'secret reasoning', durationMs: 800, collapsed: true },
      { kind: 'assistant', id: 'a1', markdown: 'answer' },
    ]);
  });

  test('envelope without duration yields a collapsed row without durationMs', () => {
    const rows = sessionToRows(
      session([
        { id: 't2', role: 'thinking', content: thinkingEnvelope({ text: 'bare' }) },
      ]),
    );
    expect(rows).toEqual([{ kind: 'thinking', id: 't2', text: 'bare', collapsed: true }]);
  });

  test('malformed thinking content falls back to a visible system row', () => {
    const raw = JSON.stringify({ type: 'other', v: 1 });
    const rows = sessionToRows(
      session([
        { id: 'x1', role: 'thinking', content: 'not json' },
        { id: 'x2', role: 'thinking', content: raw },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'system', id: 'x1', text: 'not json' },
      { kind: 'system', id: 'x2', text: raw },
    ]);
  });
});
