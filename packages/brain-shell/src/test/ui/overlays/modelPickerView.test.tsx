/**
 * Inc 38: Component tests for ModelPickerView.
 */
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { ModelPickerView } from '../../../ui/overlays/ModelPickerView.js';
import type { ModelItemVM } from '../../../ui/overlays/modelPickerLogic.js';

function textOf(el: React.ReactElement): string {
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    if (typeof el2.type === 'function') {
      try {
        return walk((el2.type as Function)(el2.props));
      } catch {
        // fall through
      }
    }
    if (el2.props && typeof el2.props === 'object' && 'children' in el2.props) {
      return walk((el2.props as { children?: React.ReactNode }).children);
    }
    return '';
  };
  return walk(el);
}

const tokens = PALETTES.dark;

const MOCK_MODELS: ModelItemVM[] = [
  {
    id: 'brain-default',
    name: 'Brain Default',
    provider: 'local',
    contextWindow: 128000,
    maxOutputTokens: 8192,
    supportsThinking: true,
    supportsTools: true,
    isDefault: true,
    isActive: true,
    contextLabel: '128k ctx',
  },
  {
    id: 'claude-3-5-sonnet',
    name: 'Claude 3.5 Sonnet',
    provider: 'anthropic',
    contextWindow: 200000,
    maxOutputTokens: 8192,
    supportsThinking: true,
    supportsTools: true,
    isDefault: false,
    isActive: false,
    contextLabel: '200k ctx',
  },
];

describe('ModelPickerView (Inc 38)', () => {
  test('loading state renders discovering indicator', () => {
    const el = <ModelPickerView loading={true} models={[]} selectedIndex={0} tokens={tokens} />;
    const txt = textOf(el);
    expect(txt).toContain('Select Active Model');
    expect(txt).toContain('Discovering available models from gateway…');
  });

  test('error state renders error message', () => {
    const el = (
      <ModelPickerView
        loading={false}
        error="Could not connect to daemon."
        models={[]}
        selectedIndex={0}
        tokens={tokens}
      />
    );
    const txt = textOf(el);
    expect(txt).toContain('Could not connect to daemon.');
  });

  test('empty state renders no registered models', () => {
    const el = <ModelPickerView loading={false} models={[]} selectedIndex={0} tokens={tokens} />;
    const txt = textOf(el);
    expect(txt).toContain('No registered models found.');
  });

  test('ready state renders model list with active markers and capability badges', () => {
    const el = (
      <ModelPickerView
        loading={false}
        models={MOCK_MODELS}
        selectedIndex={0}
        tokens={tokens}
      />
    );
    const txt = textOf(el);
    expect(txt).toContain('Select Active Model');
    expect(txt).toContain('● Brain Default (local) [default] [thinking] [tools]');
    expect(txt).toContain('128k ctx');
    expect(txt).toContain('Claude 3.5 Sonnet (anthropic) [thinking] [tools]');
    expect(txt).toContain('200k ctx');
    expect(txt).toContain('↑↓ navigate · enter switch model · esc close');
  });
});
