/** /memory overlay shared bits: score display and selection clamping.
 * The liveness result type lives in the client contract (MemorySearchResult). */

export function scorePercent(score: number): number {
  return Math.max(0, Math.min(100, Math.round(score)));
}

export function clampSelection(selected: number, count: number): number {
  if (count === 0) return 0;
  return Math.min(Math.max(0, selected), count - 1);
}
