import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { ThinkingRowView } from '../../../ui/transcript/MessageRow.js';

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

const row = (over: Record<string, unknown>) =>
  ({ kind: 'thinking', id: 't', text: 'hidden reasoning', ...over }) as React.ComponentProps<
    typeof ThinkingRowView
  >['row'];

describe('ThinkingRowView collapse (Inc 19)', () => {
  test('live-style row renders summary plus italic body', () => {
    const out = textOf(ThinkingRowView({ row: row({ durationMs: 3200 }), tokens: PALETTES.dark }));
    expect(out).toContain('Thought for 3.2s');
    expect(out).toContain('hidden reasoning');
  });

  test('collapsed replay row renders ONLY the summary line', () => {
    const out = textOf(
      ThinkingRowView({ row: row({ durationMs: 800, collapsed: true }), tokens: PALETTES.dark }),
    );
    expect(out).toContain('✻ Thought for 0.8s');
    expect(out).not.toContain('hidden reasoning');
  });

  test('collapsed row without duration renders nothing but stays mounted', () => {
    const out = textOf(ThinkingRowView({ row: row({ collapsed: true }), tokens: PALETTES.dark }));
    expect(out).toBe('');
  });
});
