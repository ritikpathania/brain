/** Pure core for the /resume picker: ordering, age labels, decisions. */
import type { BrainSessionSummary } from '../../client/BrainBackendClient.js';
import { overlayListDecision, type OverlayListDecision } from './overlayLogic.js';

export interface ResumeVM {
  id: string;
  title: string;
  age: string;
  pinned: boolean;
}

export const RESUME_MAX_ITEMS = 8;

export function formatAge(nowMs: number, updatedAtMs: number): string {
  const dt = Math.max(0, nowMs - updatedAtMs);
  if (dt < 60_000) return 'just now';
  if (dt < 3_600_000) return `${Math.floor(dt / 60_000)}m ago`;
  if (dt < 86_400_000) return `${Math.floor(dt / 3_600_000)}h ago`;
  if (dt < 7 * 86_400_000) return `${Math.floor(dt / 86_400_000)}d ago`;
  const d = new Date(updatedAtMs);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/**
 * B5: case-insensitive greedy subsequence score. Returns null when query
 * isn't a subsequence of text. Score rewards contiguous runs (+3 vs +1)
 * and word-boundary starts (+2) so "alp" prefers "Alpha Groove".
 */
export function fuzzyScore(query: string, text: string): number | null {
  if (query.length === 0) return 0;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  let score = 0;
  let searchFrom = 0;
  let prevIdx = -2;
  for (let qi = 0; qi < q.length; qi++) {
    const idx = t.indexOf(q[qi], searchFrom);
    if (idx === -1) return null;
    score += idx === prevIdx + 1 ? 3 : 1;
    if (idx === 0 || /[\s\-_/]/.test(t[idx - 1])) score += 2;
    prevIdx = idx;
    searchFrom = idx + 1;
  }
  return score;
}

/** B5: pure reducer turning overlay keyboard actions into query edits. */
export function applyQueryEdit(query: string, action: string, input: string): string {
  if (action === 'overlay:insert') return query + input;
  if (action === 'overlay:backspace') return query.slice(0, -1);
  return query;
}

function toVm(nowMs: number) {
  return (s: BrainSessionSummary): ResumeVM => ({
    id: s.id,
    title: s.title,
    age: formatAge(nowMs, s.updatedAtMs),
    pinned: s.pinned,
  });
}

const byPinnedThenRecency = (a: BrainSessionSummary, b: BrainSessionSummary): number =>
  (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || b.updatedAtMs - a.updatedAtMs;

export function resumeChoices(
  summaries: BrainSessionSummary[],
  nowMs: number,
  query: string = '',
): ResumeVM[] {
  const active = summaries.filter((s) => !s.archived);
  if (query.length === 0) {
    return active.sort(byPinnedThenRecency).slice(0, RESUME_MAX_ITEMS).map(toVm(nowMs));
  }
  return active
    .flatMap((s) => {
      const score = fuzzyScore(query, s.title);
      return score === null ? [] : [{ s, score }];
    })
    .sort((a, b) => b.score - a.score || b.s.updatedAtMs - a.s.updatedAtMs)
    .slice(0, RESUME_MAX_ITEMS)
    .map(({ s }) => toVm(nowMs)(s));
}

export function resumeListDecision(
  action: string | null,
  selected: number,
  count: number,
): OverlayListDecision {
  return overlayListDecision(action, selected, count);
}
