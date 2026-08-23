import { describe, it, expect } from 'bun:test';
import * as React from 'react';
import { PaletteView } from '../../ui/composer/PaletteView.js';
import { PALETTES } from '../../state/palettes.js';

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

describe('PaletteView', () => {
  const items = [
    { name: 'help', description: 'List available slash commands' },
    { name: 'clear', description: 'Clear the transcript' },
    { name: 'quit', description: 'Exit Brain shell' },
  ];

  it('renders nothing for an empty list', () => {
    expect(PaletteView({ items: [], selectedIndex: 0, maxColumns: 80, tokens: TOKENS })).toBeNull();
  });

  it('marks the selected row with ❯ and shows descriptions', () => {
    const out = textOf(PaletteView({ items, selectedIndex: 1, maxColumns: 80, tokens: TOKENS })!);
    expect(out).toContain('❯ /clear');
    expect(out).toContain('Clear the transcript');
    expect(out).toContain('/help');
    expect(out).toContain('/quit');
  });

  it('windows long lists around the selection', () => {
    const many = Array.from({ length: 9 }, (_, i) => ({ name: `cmd${i}`, description: 'd' }));
    const out = textOf(PaletteView({ items: many, selectedIndex: 6, maxColumns: 80, tokens: TOKENS })!);
    expect(out).toContain('/cmd6');
    expect(out).not.toContain('/cmd0'); // scrolled off the front
    expect(out).toContain('❯');
  });

  it('truncates rows to maxColumns', () => {
    const wide = [{ name: 'help', description: 'x'.repeat(200) }];
    const out = textOf(PaletteView({ items: wide, selectedIndex: 0, maxColumns: 40, tokens: TOKENS })!);
    expect(out.length).toBeLessThanOrEqual(41); // 40 cols + ellipsis tolerance
  });
});
