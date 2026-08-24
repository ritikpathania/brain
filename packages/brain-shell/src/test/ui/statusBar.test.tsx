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
