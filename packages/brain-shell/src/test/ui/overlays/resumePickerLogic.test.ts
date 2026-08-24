import { describe, expect, test } from 'bun:test';
import { formatAge, resumeChoices } from '../../../ui/overlays/resumePickerLogic.js';
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
