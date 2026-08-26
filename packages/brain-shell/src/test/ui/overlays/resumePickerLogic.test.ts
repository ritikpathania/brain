import { describe, expect, test } from 'bun:test';
import {
  applyQueryEdit,
  formatAge,
  fuzzyScore,
  resumeChoices,
} from '../../../ui/overlays/resumePickerLogic.js';
import type { BrainSessionSummary } from '../../../client/BrainBackendClient.js';

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const NOW = 1_800_000_000_000;

const s = (over: Partial<BrainSessionSummary>): BrainSessionSummary => ({
  id: 's',
  title: 'T',
  updatedAtMs: NOW - HOUR,
  pinned: false,
  archived: false,
  ...over,
});

describe('formatAge', () => {
  test('buckets', () => {
    expect(formatAge(NOW, NOW - 30_000)).toBe('just now');
    expect(formatAge(NOW, NOW - 5 * MIN)).toBe('5m ago');
    expect(formatAge(NOW, NOW - 3 * HOUR)).toBe('3h ago');
    expect(formatAge(NOW, NOW - 2 * DAY)).toBe('2d ago');
    expect(formatAge(NOW, NOW - 30 * DAY)).toMatch(/^[A-Z][a-z]{2} \d{1,2}$/);
  });
});

describe('resumeChoices', () => {
  test('filters archived, pins first, newest first, caps at 8', () => {
    const list = [
      s({ id: 'a', updatedAtMs: NOW - DAY }),
      s({ id: 'b', archived: true }),
      s({ id: 'c', pinned: true, updatedAtMs: NOW - 5 * DAY }),
      s({ id: 'd', updatedAtMs: NOW - MIN }),
    ];
    const out = resumeChoices(list, NOW);
    expect(out.map((v) => v.id)).toEqual(['c', 'd', 'a']);
    expect(out[0]).toEqual({ id: 'c', title: 'T', age: '5d ago', pinned: true });
  });

  test('caps at eight entries', () => {
    const many = Array.from({ length: 12 }, (_, i) => s({ id: `x${i}`, updatedAtMs: NOW - i * MIN }));
    expect(resumeChoices(many, NOW)).toHaveLength(8);
  });
});

describe('fuzzyScore', () => {
  test('empty query matches everything neutrally', () => {
    expect(fuzzyScore('', 'anything')).toBe(0);
  });

  test('exact prefix beats scattered', () => {
    const exact = fuzzyScore('alp', 'Alpha Groove');
    const scattered = fuzzyScore('alp', 'xaxlxp other');
    expect(exact).not.toBeNull();
    expect(scattered).not.toBeNull();
    expect(exact! > scattered!).toBe(true);
  });

  test('word-boundary hits outrank mid-word hits', () => {
    const boundary = fuzzyScore('g', 'Alpha Groove'); // after space
    const midword = fuzzyScore('g', 'dogma'); // mid-word, no bonus
    expect(boundary! > midword!).toBe(true);
  });

  test('non-subsequence returns null', () => {
    expect(fuzzyScore('zx', 'Alpha Groove')).toBeNull();
    expect(fuzzyScore('alpha', 'groove')).toBeNull(); // case-insensitive subsequence only
  });

  test('match is case-insensitive', () => {
    expect(fuzzyScore('AG', 'alpha groove')).not.toBeNull();
  });
});

describe('applyQueryEdit', () => {
  test('insert appends the typed character', () => {
    expect(applyQueryEdit('al', 'overlay:insert', 'p')).toBe('alp');
  });

  test('backspace removes the last character', () => {
    expect(applyQueryEdit('alp', 'overlay:backspace', '')).toBe('al');
    expect(applyQueryEdit('', 'overlay:backspace', '')).toBe('');
  });

  test('unknown actions leave the query unchanged', () => {
    expect(applyQueryEdit('al', 'overlay:up', '')).toBe('al');
    expect(applyQueryEdit('al', 'overlay:commit', '')).toBe('al');
  });
});

describe('resumeChoices with query (B5)', () => {
  test('empty query reproduces legacy ordering byte-for-byte', () => {
    const list = [
      s({ id: 'a', updatedAtMs: NOW - DAY }),
      s({ id: 'b', archived: true }),
      s({ id: 'c', pinned: true, updatedAtMs: NOW - 5 * DAY }),
      s({ id: 'd', updatedAtMs: NOW - MIN }),
    ];
    const withArg = resumeChoices(list, NOW, '');
    const withoutArg = resumeChoices(list, NOW);
    expect(withArg).toEqual(withoutArg);
    expect(withArg.map((v) => v.id)).toEqual(['c', 'd', 'a']);
  });

  test('filters across ALL sessions by fuzzy score, ranked', () => {
    const list = [
      s({ id: 'old-groove', title: 'Groove old', updatedAtMs: NOW - 9 * DAY }),
      s({ id: 'unrelated', title: 'Totally different', updatedAtMs: NOW - MIN }),
      s({ id: 'best', title: 'Alpha Groove', updatedAtMs: NOW - HOUR }),
    ];
    const out = resumeChoices(list, NOW, 'groove');
    expect(out.map((v) => v.id)).toEqual(['best', 'old-groove']);
  });

  test('archived excluded and cap still applies while searching', () => {
    const many = Array.from({ length: 12 }, (_, i) =>
      s({ id: `m${i}`, title: `needle item ${i}`, updatedAtMs: NOW - i * MIN }),
    );
    many.push(s({ id: 'arch', title: 'needle archived', archived: true }));
    const out = resumeChoices(many, NOW, 'needle');
    expect(out).toHaveLength(8);
    expect(out.some((v) => v.id === 'arch')).toBe(false);
  });

  test('score ties break by recency', () => {
    const list = [
      s({ id: 'older', title: 'Beta Session', updatedAtMs: NOW - 2 * HOUR }),
      s({ id: 'newer', title: 'Beta Session', updatedAtMs: NOW - MIN }),
    ];
    expect(resumeChoices(list, NOW, 'beta').map((v) => v.id)).toEqual(['newer', 'older']);
  });

  test('no matches yields empty array', () => {
    expect(resumeChoices([s({ id: 'a', title: 'T' })], NOW, 'zzz')).toEqual([]);
  });
});
