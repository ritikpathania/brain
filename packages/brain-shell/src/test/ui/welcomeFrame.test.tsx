import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../state/palettes.js';
import { WelcomeFrameView } from '../../ui/shell/WelcomeFrame.js';

function textOf(el: React.ReactElement): string {
  // Flatten ink Text/Box trees into plain text for assertion.
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

describe('WelcomeFrameView', () => {
  test('carries the wordmark, identity line, workspace, and hints', () => {
    const text = textOf(WelcomeFrameView({ tokens: PALETTES.dark, workspace: 'brain' }));
    expect(text).toContain('◆ BRAIN');
    expect(text).toContain('memory-first agent workspace');
    expect(text).toContain('workspace brain');
    expect(text).toContain('/help commands');
    expect(text).toContain('/resume sessions');
    expect(text).toContain('/theme appearance');
  });

  test('renders nothing proprietary', () => {
    const text = textOf(WelcomeFrameView({ tokens: PALETTES.dark, workspace: 'x' }));
    expect(text.toLowerCase()).not.toContain('claude');
    expect(text.toLowerCase()).not.toContain('anthropic');
  });
});
