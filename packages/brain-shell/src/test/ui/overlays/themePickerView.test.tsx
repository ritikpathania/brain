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
      ThemePickerView({
        choices: THEME_CHOICES,
        selectedIndex: 2,
        current: 'light',
        tokens: PALETTES.dark,
      }),
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
