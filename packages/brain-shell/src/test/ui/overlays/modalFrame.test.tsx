import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { ModalFrame } from '../../../ui/overlays/ModalFrame.js';

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

describe('ModalFrame (Inc 21)', () => {
  test('renders title, subtitle, children, footer hints', () => {
    const out = textOf(
      ModalFrame({
        title: 'Brain System Doctor',
        subtitle: 'Subsystem health probes',
        footerHints: 'Enter / Esc to dismiss',
        width: 80,
        children: React.createElement('ink-box', null, 'BODY CONTENT'),
      }),
    );
    expect(out).toContain('Brain System Doctor');
    expect(out).toContain('Subsystem health probes');
    expect(out).toContain('BODY CONTENT');
    expect(out).toContain('Enter / Esc to dismiss');
  });

  test('omits subtitle/footer when absent', () => {
    const out = textOf(
      ModalFrame({ title: 'Only Title', width: 40, children: null }),
    );
    expect(out).toContain('Only Title');
    expect(out).not.toContain('dismiss');
  });
});
