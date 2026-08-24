import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { PermissionDialogView } from '../../../ui/overlays/PermissionDialog.js';

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

describe('PermissionDialogView', () => {
  test('shows tool, summarized input, and both options with selection', () => {
    const text = textOf(
      PermissionDialogView({
        req: {
          callId: 'c1',
          toolName: 'bash',
          input: { command: 'rm -rf build' },
          reason: 'destructive',
        },
        selected: 1,
        tokens: PALETTES.dark,
      }),
    );
    expect(text).toContain('Permission required');
    expect(text).toContain('bash');
    expect(text).toContain('rm -rf build');
    expect(text).toContain('[ Deny ]');
    expect(text).toContain('[ Allow ]');
    expect(text).toContain('esc denies');
  });
});
