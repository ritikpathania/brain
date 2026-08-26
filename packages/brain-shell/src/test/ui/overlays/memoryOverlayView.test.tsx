import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { MemoryOverlayView } from '../../../ui/overlays/MemoryOverlayView.js';
import type { RetrievedMemory } from '../../../client/BrainBackendClient.js';

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

const tokens = PALETTES.dark;

const row = (label: string, relations?: RetrievedMemory['relations']): RetrievedMemory => ({
  node_id: label.toLowerCase(),
  label,
  excerpt: `${label} excerpt`,
  channel: 'knowledge_graph',
  score: 90,
  timestamp: 0,
  scope: 'workspace',
  ...(relations ? { relations } : {}),
});

describe('MemoryOverlayView (Inc 21)', () => {
  test('ready state lists labels with scores and channels', () => {
    const out = textOf(MemoryOverlayView({
      query: '', state: 'ready',
      rows: [row('Alpha Cortex Node'), row('Beta Ledger')],
      selectedIndex: 0, expandedId: null, tokens,
    }));
    expect(out).toContain('Alpha Cortex Node');
    expect(out).toContain('90%');
    expect(out).toContain('[knowledge_graph]');
  });

  test('query line renders the live filter text', () => {
    const out = textOf(MemoryOverlayView({
      query: 'crtx', state: 'ready',
      rows: [row('Alpha Cortex Node')],
      selectedIndex: 0, expandedId: null, tokens,
    }));
    expect(out).toContain('› crtx');
  });

  test('expanded row shows excerpt and relations', () => {
    const out = textOf(MemoryOverlayView({
      query: '', state: 'ready',
      rows: [row('Alpha Cortex Node', [
        { target_id: 'b1', relation: 'supports', target_label: 'Beta Concept' },
      ])],
      selectedIndex: 0, expandedId: 'alpha cortex node', tokens,
    }));
    expect(out).toContain('Connected Relations:');
    expect(out).toContain('supports');
    expect(out).toContain('Beta Concept');
  });

  test('expanded row without relations shows the none-line', () => {
    const out = textOf(MemoryOverlayView({
      query: '', state: 'ready',
      rows: [row('Solo Node')],
      selectedIndex: 0, expandedId: 'solo node', tokens,
    }));
    expect(out).toContain('(No outgoing relations)');
  });

  test('offline, loading, and empty states', () => {
    expect(textOf(MemoryOverlayView({ query: '', state: 'offline', rows: [], selectedIndex: 0, expandedId: null, tokens })))
      .toContain('Brain daemon is offline or unreachable.');
    expect(textOf(MemoryOverlayView({ query: '', state: 'loading', rows: [], selectedIndex: 0, expandedId: null, tokens })))
      .toContain('Searching knowledge graph');
    expect(textOf(MemoryOverlayView({ query: '', state: 'ready', rows: [], selectedIndex: 0, expandedId: null, tokens })))
      .toContain('No concepts recorded in the Brain knowledge graph yet.');
  });
});
