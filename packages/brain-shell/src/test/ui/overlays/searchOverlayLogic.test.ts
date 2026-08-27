/**
 * Inc 34: Unit tests for search overlay logic and direct resume target resolution.
 */
import { describe, it, expect } from 'bun:test';
import {
  toSearchVM,
  applySearchQueryEdit,
  clampSearchSelection,
  searchListDecision,
} from '../../../ui/overlays/searchOverlayLogic.js';
import {
  resolveDirectResumeTarget,
} from '../../../ui/overlays/resumePickerLogic.js';
import type { BrainSessionSummary, TranscriptSearchMatch } from '../../../client/BrainBackendClient.js';

describe('searchOverlayLogic', () => {
  it('toSearchVM normalizes match DTO into presentation VM', () => {
    const match: TranscriptSearchMatch = {
      id: 'msg-123',
      kind: 'message',
      sessionId: '01JKXYZABC123',
      title: 'user',
      body: 'Superconducting qubit coherence times in dilution refrigerators',
      snippet: 'Superconducting qubit   coherence times',
      role: 'user',
      score: 0.95,
    };

    const vm = toSearchVM(match);
    expect(vm.id).toBe('msg-123');
    expect(vm.sessionId).toBe('01JKXYZABC123');
    expect(vm.role).toBe('user');
    expect(vm.snippet).toBe('Superconducting qubit coherence times');
    expect(vm.score).toBe(0.95);
  });

  it('applySearchQueryEdit inserts characters and handles backspace', () => {
    expect(applySearchQueryEdit('cryo', 'overlay:insert', 'g')).toBe('cryog');
    expect(applySearchQueryEdit('cryog', 'overlay:backspace', '')).toBe('cryo');
    expect(applySearchQueryEdit('cryo', 'unrelated:action', '')).toBe('cryo');
  });

  it('clampSearchSelection clamps index into safe bounds', () => {
    expect(clampSearchSelection(-1, 5)).toBe(0);
    expect(clampSearchSelection(2, 5)).toBe(2);
    expect(clampSearchSelection(10, 5)).toBe(4);
    expect(clampSearchSelection(0, 0)).toBe(0);
  });

  it('searchListDecision navigates, commits, and cancels', () => {
    expect(searchListDecision('overlay:down', 0, 5)).toEqual({ type: 'move', index: 1 });
    expect(searchListDecision('overlay:up', 2, 5)).toEqual({ type: 'move', index: 1 });
    expect(searchListDecision('overlay:commit', 2, 5)).toEqual({ type: 'commit', index: 2 });
    expect(searchListDecision('overlay:cancel', 2, 5)).toEqual({ type: 'cancel' });
  });
});

describe('resolveDirectResumeTarget', () => {
  const summaries: BrainSessionSummary[] = [
    {
      id: '01JKALPHA11111111111111111',
      title: 'Quantum Hardware Calibration',
      createdAtMs: 1000,
      updatedAtMs: 2000,
      pinned: true,
      archived: false,
    },
    {
      id: '01JKBETA222222222222222222',
      title: 'Quantum Error Correction',
      createdAtMs: 1000,
      updatedAtMs: 3000,
      pinned: false,
      archived: false,
    },
    {
      id: '01JKGAMMA33333333333333333',
      title: 'Cryogenic Refrigeration Systems',
      createdAtMs: 1000,
      updatedAtMs: 4000,
      pinned: false,
      archived: false,
    },
    {
      id: '01JKARCHIVED99999999999999',
      title: 'Archived Quantum Notes',
      createdAtMs: 1000,
      updatedAtMs: 1000,
      pinned: false,
      archived: true,
    },
  ];

  it('resolves exact session ID match', () => {
    const res = resolveDirectResumeTarget(summaries, '01JKALPHA11111111111111111');
    expect(res).toEqual({
      type: 'exact',
      sessionId: '01JKALPHA11111111111111111',
    });
  });

  it('resolves unique session ID prefix match', () => {
    const res = resolveDirectResumeTarget(summaries, '01jkg');
    expect(res).toEqual({
      type: 'unique',
      sessionId: '01JKGAMMA33333333333333333',
    });
  });

  it('identifies ambiguous session ID prefix matches', () => {
    const res = resolveDirectResumeTarget(summaries, '01jk');
    expect(res.type).toBe('ambiguous');
    if (res.type === 'ambiguous') {
      expect(res.candidates.length).toBe(3); // excludes archived
    }
  });

  it('resolves unique title match', () => {
    const res = resolveDirectResumeTarget(summaries, 'Cryogenic');
    expect(res).toEqual({
      type: 'unique',
      sessionId: '01JKGAMMA33333333333333333',
    });
  });

  it('identifies ambiguous title matches', () => {
    const res = resolveDirectResumeTarget(summaries, 'Quantum');
    expect(res.type).toBe('ambiguous');
    if (res.type === 'ambiguous') {
      expect(res.candidates.length).toBe(2); // 'Quantum Hardware' and 'Quantum Error', excludes archived
    }
  });

  it('returns none for unmatched queries', () => {
    const res = resolveDirectResumeTarget(summaries, 'nonexistent_target_string');
    expect(res).toEqual({
      type: 'none',
      query: 'nonexistent_target_string',
    });
  });
});
