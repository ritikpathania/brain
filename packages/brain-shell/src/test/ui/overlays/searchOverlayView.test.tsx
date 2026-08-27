/**
 * Inc 34: React view tests for SearchOverlayView.
 */
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { SearchOverlayView } from '../../../ui/overlays/SearchOverlayView.js';
import type { SearchItemVM } from '../../../ui/overlays/searchOverlayLogic.js';

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

const item = (sessionId: string, role: string, snippet: string, score?: number): SearchItemVM => ({
  id: `msg-${sessionId}`,
  sessionId,
  role,
  title: role,
  snippet,
  score,
});

describe('SearchOverlayView (Inc 34)', () => {
  test('idle state with empty query prompts user to search', () => {
    const out = textOf(SearchOverlayView({
      query: '',
      state: 'idle',
      matches: [],
      selectedIndex: 0,
      tokens,
    }));
    expect(out).toContain('Search Transcripts');
    expect(out).toContain('Type a query to search');
  });

  test('loading state renders searching indicator', () => {
    const out = textOf(SearchOverlayView({
      query: 'quantum',
      state: 'loading',
      matches: [],
      selectedIndex: 0,
      tokens,
    }));
    expect(out).toContain('Searching transcripts across sessions…');
  });

  test('ready state lists match items with session prefixes, roles, and snippets', () => {
    const out = textOf(SearchOverlayView({
      query: 'cryogenic',
      state: 'ready',
      matches: [
        item('01JKALPHA123456789', 'user', 'Cryogenic dilution refrigerators cool superconducting circuits below 15mK', 0.92),
        item('01JKBETA987654321', 'assistant', 'Trapped ion hardware operates using radiofrequency fields', 0.81),
      ],
      selectedIndex: 0,
      tokens,
      currentSessionId: '01JKALPHA123456789',
    }));
    expect(out).toContain('Search Transcripts');
    expect(out).toContain('[01JKALPH]');
    expect(out).toContain('(user)');
    expect(out).toContain('Cryogenic dilution refrigerators');
    expect(out).toContain('92%');
    expect(out).toContain('[01JKBETA]');
    expect(out).toContain('(assistant)');
  });

  test('ready state with 0 matches renders empty notice', () => {
    const out = textOf(SearchOverlayView({
      query: 'non_existent_query',
      state: 'ready',
      matches: [],
      selectedIndex: 0,
      tokens,
    }));
    expect(out).toContain('No transcript matches found for “non_existent_query”.');
  });
});
