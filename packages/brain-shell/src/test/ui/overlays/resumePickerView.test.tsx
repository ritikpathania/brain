import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { ResumePickerView } from '../../../ui/overlays/ResumePicker.js';
import type { ResumeVM } from '../../../ui/overlays/resumePickerLogic.js';

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

const vm = (id: string, title: string): ResumeVM => ({ id, title, age: '2m ago', pinned: false });
const tokens = PALETTES.dark;

describe('ResumePickerView (B5)', () => {
  test('renders the live query line', () => {
    const out = textOf(
      ResumePickerView({
        items: [vm('a', 'Alpha')],
        selectedIndex: 0,
        tokens,
        query: 'alp',
        currentSessionId: undefined,
      }),
    );
    expect(out).toContain('› alp');
  });

  test('marks the current session row with ●', () => {
    const out = textOf(
      ResumePickerView({
        items: [vm('live', 'Current'), vm('other', 'Other')],
        selectedIndex: 0,
        tokens,
        query: '',
        currentSessionId: 'live',
      }),
    );
    expect(out).toContain('●');
    expect(out).toContain('Current');
  });

  test('no marker when currentSessionId is absent or unmatched', () => {
    const plain = textOf(
      ResumePickerView({
        items: [vm('a', 'Alpha')],
        selectedIndex: 0,
        tokens,
        query: '',
      }),
    );
    expect(plain).not.toContain('●');
    const unmatched = textOf(
      ResumePickerView({
        items: [vm('a', 'Alpha')],
        selectedIndex: 0,
        tokens,
        query: '',
        currentSessionId: 'elsewhere',
      }),
    );
    expect(unmatched).not.toContain('●');
  });

  test('empty result renders the no-match line', () => {
    const out = textOf(
      ResumePickerView({
        items: [],
        selectedIndex: 0,
        tokens,
        query: 'zzz',
        currentSessionId: undefined,
      }),
    );
    expect(out).toContain('No sessions match.');
  });

  test('hint mentions type-to-filter', () => {
    const out = textOf(
      ResumePickerView({ items: [vm('a', 'Alpha')], selectedIndex: 0, tokens, query: '' }),
    );
    expect(out).toContain('type to filter');
  });
});
