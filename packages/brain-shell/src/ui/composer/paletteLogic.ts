/**
 * Pure decision layer for the slash-command palette. The component applies
 * these verdicts; all branching lives here so it stays unit-testable.
 */
export const PALETTE_MAX_ITEMS = 5;

export interface PaletteWindow {
  start: number;
  end: number; // exclusive
}

export function paletteWindow(
  itemCount: number,
  selected: number,
  max: number = PALETTE_MAX_ITEMS,
): PaletteWindow {
  if (itemCount <= max) return { start: 0, end: itemCount };
  const start = Math.max(0, Math.min(selected - max + 1, itemCount - max));
  return { start, end: start + max };
}

export type PaletteDecision =
  | { kind: 'move'; next: number }
  | { kind: 'complete'; index: number }
  | { kind: 'close' }
  | { kind: 'passthrough' };

export function paletteKeyDecision(opts: {
  open: boolean;
  cmdType: string;
  tab: boolean;
  selected: number;
  count: number;
}): PaletteDecision {
  const { open, cmdType, tab, selected, count } = opts;
  if (!open || count === 0) return { kind: 'passthrough' };
  if (cmdType === 'history_up') return { kind: 'move', next: Math.max(0, selected - 1) };
  if (cmdType === 'history_down')
    return { kind: 'move', next: Math.min(count - 1, selected + 1) };
  if (tab) return { kind: 'complete', index: Math.min(selected, count - 1) };
  if (cmdType === 'abort') return { kind: 'close' };
  return { kind: 'passthrough' };
}
