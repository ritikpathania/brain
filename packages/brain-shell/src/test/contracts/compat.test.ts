import { describe, expect, test } from 'bun:test';
import { Box, Text, stringWidth, useTheme } from '../../compat/ink.js';
import { useTerminalSize } from '../../compat/hooks.js';
import { truncatePath } from '../../compat/text.js';
import { PALETTES } from '../../state/palettes.js';

describe('compat', () => {
  test('re-exports stock ink primitives', () => {
    expect(Box).toBeDefined();
    expect(Text).toBeDefined();
    expect(useTerminalSize).toBeTypeOf('function');
    expect(useTheme).toBeTypeOf('function');
  });

  test('stringWidth counts display columns, ANSI-stripped', () => {
    expect(stringWidth('héllo')).toBe(5);
    expect(stringWidth('\x1b[31mred\x1b[0m')).toBe(3);
    expect(stringWidth('中文')).toBe(4);
  });

  test('truncatePath keeps tail visible within budget', () => {
    const out = truncatePath('/Users/x/dev/brain/packages/brain-shell', 24);
    expect(out.length).toBeLessThanOrEqual(24);
    expect(out.endsWith('brain-shell')).toBe(true);
  });

  test('truncatePath passes through short paths untouched', () => {
    expect(truncatePath('/a/b.txt', 40)).toBe('/a/b.txt');
  });

  test('all four palettes expose the full token set', () => {
    for (const [name, tokens] of Object.entries(PALETTES)) {
      for (const [role, value] of Object.entries(tokens)) {
        expect(value, `${name}.${role}`).toMatch(/^#[0-9A-F]{6}$/i);
      }
    }
  });
});
