import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { ToolRowView } from '../../../ui/transcript/MessageRow.js';
import { turnToRows } from '../../../ui/transcript/toRows.js';

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

function row(output?: string, isError?: boolean) {
  return {
    kind: 'tool' as const,
    id: 't1',
    tool: {
      callId: 'call_tr',
      toolName: 'bash',
      input: { command: 'echo hi' },
      status: 'completed' as const,
      output,
      isError,
    },
  };
}

describe('ToolRowView output rendering', () => {
  test('collapsed shows a single truncated preview line', () => {
    const el = ToolRowView({
      row: row('x'.repeat(200)),
      expanded: false,
      tokens: PALETTES.dark,
    });
    const text = textOf(el);
    expect(text).toContain('x'.repeat(120));
    expect(text).not.toContain('x'.repeat(121));
  });

  test('expanded shows the full output after the input json', () => {
    const el = ToolRowView({
      row: row('line1\nline2'),
      expanded: true,
      tokens: PALETTES.dark,
    });
    const text = textOf(el);
    // Output renders indented under an ── output ── marker, after the JSON.
    expect(text).toContain('── output ──');
    expect(text).toContain('line1');
    expect(text).toContain('line2');
    expect(text.indexOf('"command"')).toBeLessThan(text.indexOf('── output ──'));
  });

  test('no output renders nothing beyond the status line', () => {
    const el = ToolRowView({ row: row(undefined), expanded: false, tokens: PALETTES.dark });
    const text = textOf(el);
    expect(text).not.toContain('[truncated]');
    expect(text).toContain('Done');
  });

  test('error output keeps the preview (coloring is status-driven)', () => {
    const el = ToolRowView({ row: row('segfault', true), expanded: false, tokens: PALETTES.dark });
    expect(textOf(el)).toContain('segfault');
  });
});

describe('toolCard projection preserves output', () => {
  test('turnToRows carries output and isError through to the card', () => {
    const vmTurn = {
      id: 'turn_1',
      content: '',
      thinking: undefined,
      tools: [
        {
          callId: 'call_tr',
          toolName: 'bash',
          input: { command: 'echo hi' },
          status: 'completed',
          output: 'hi\n',
          isError: false,
        },
      ],
    };
    const rows = turnToRows(vmTurn as never);
    const tool = rows.find((r) => r.kind === 'tool') as {
      tool: { output?: string; isError?: boolean };
    };
    expect(tool.tool.output).toBe('hi\n');
    expect(tool.tool.isError).toBe(false);
  });
});
