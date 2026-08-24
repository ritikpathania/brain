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

export function resumeChoices(summaries: BrainSessionSummary[], nowMs: number): ResumeVM[] {
  return summaries
    .filter((s) => !s.archived)
    .sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || b.updatedAtMs - a.updatedAtMs)
    .slice(0, RESUME_MAX_ITEMS)
    .map((s) => ({ id: s.id, title: s.title, age: formatAge(nowMs, s.updatedAtMs), pinned: s.pinned }));
}

export function resumeListDecision(
  action: string | null,
  selected: number,
  count: number,
): OverlayListDecision {
  return overlayListDecision(action, selected, count);
}
