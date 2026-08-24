import { describe, it, expect } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../state/palettes.js';
import {
  UserRowView,
  ThinkingRowView,
  ToolRowView,
  ErrorRowView,
  SystemRowView,
  summarizeToolInput,
} from '../../ui/transcript/MessageRow.js';
import type { TranscriptRow } from '../../contracts/messages.js';

// Pure-view pattern (see src/test/contracts/shell.test.tsx): invoke the view
// functions directly with resolved tokens; hooked wrappers only get validity
// checks here, live behavior is covered by the PTY smoke.

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

describe('row views', () => {
  it('user row echoes with the ❯ glyph', () => {
    const out = textOf(UserRowView({ row: { kind: 'user', id: 'u1', text: 'hello there' }, tokens: TOKENS }));
    expect(out).toContain('❯');
    expect(out).toContain('hello there');
  });

  it('thinking row renders the ✻ marker and duration when complete', () => {
    const out = textOf(
      ThinkingRowView({
        row: { kind: 'thinking', id: 't1', text: 'hmm', durationMs: 1500 },
        tokens: TOKENS,
      }),
    );
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
    const out = textOf(ToolRowView({ row, expanded: false, tokens: TOKENS }));
    expect(out).toContain('read_file');
    expect(out).toContain('/tmp/brain-demo.txt');
    expect(out).toContain('Running…');
    expect(out).not.toContain('"path"'); // collapsed hides structured input
  });

  it('tool row expanded reveals pretty-printed input JSON', () => {
    const row: TranscriptRow = {
      kind: 'tool',
      id: 'c1',
      tool: { callId: 'c1', toolName: 'read_file', input: { path: '/tmp/brain-demo.txt' }, status: 'pending' },
    };
    const out = textOf(ToolRowView({ row, expanded: true, tokens: TOKENS }));
    expect(out).toContain('"path"');
    expect(out).toContain('/tmp/brain-demo.txt');
  });

  it('error row carries the warning glyph', () => {
    const out = textOf(ErrorRowView({ row: { kind: 'error', id: 'e1', text: 'socket lost' }, tokens: TOKENS }));
    expect(out).toContain('⚠');
    expect(out).toContain('socket lost');
  });

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
});

describe('summarizeToolInput', () => {
  it('prefers well-known keys and truncates to 60 chars', () => {
    expect(summarizeToolInput({ path: '/a/b.txt', other: 1 })).toBe('/a/b.txt');
    expect(summarizeToolInput({ command: 'x'.repeat(80) })).toHaveLength(60);
    expect(summarizeToolInput({ zebra: 'last resort' })).toBe('last resort');
    expect(summarizeToolInput({})).toBe('');
  });
});

describe('failed card exit code (Inc 9)', () => {
  const toolRow = (tool: Extract<TranscriptRow, { kind: 'tool' }>['tool']): TranscriptRow => ({
    kind: 'tool',
    id: 'x1',
    tool,
  });

  it('failed card surfaces the persisted exit code', () => {
    const out = textOf(
      ToolRowView({
        row: toolRow({
          callId: 'x1',
          toolName: 'bash',
          input: { command: 'false' },
          status: 'failed',
          exitCode: 2,
        }),
        expanded: false,
        tokens: TOKENS,
      }),
    );
    expect(out).toContain('Failed');
    expect(out).toContain('exit 2');
  });

  it('completed and denied cards stay free of exit codes', () => {
    for (const status of ['completed', 'denied'] as const) {
      const out = textOf(
        ToolRowView({
          row: toolRow({
            callId: 'x1',
            toolName: 'bash',
            input: { command: 'true' },
            status,
          }),
          expanded: false,
          tokens: TOKENS,
        }),
      );
      expect(out).not.toContain('exit');
    }
  });
});
