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
