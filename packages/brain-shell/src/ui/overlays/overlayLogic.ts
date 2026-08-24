/**
 * Shared decision table for arrow-navigable overlay lists (theme picker,
 * resume picker). Actions arrive as namespaced ids from the keybinding
 * framework ('overlay:*'); indexes clamp, never wrap.
 */
export type OverlayListDecision =
  | { type: 'move'; index: number }
  | { type: 'commit'; index: number }
  | { type: 'cancel' }
  | { type: 'passthrough' };

export function overlayListDecision(
  action: string | null,
  selected: number,
  count: number,
): OverlayListDecision {
  if (action === null || count === 0) return { type: 'passthrough' };
  switch (action) {
    case 'overlay:up':
      return { type: 'move', index: Math.max(0, selected - 1) };
    case 'overlay:down':
      return { type: 'move', index: Math.min(count - 1, selected + 1) };
    case 'overlay:commit':
      return { type: 'commit', index: Math.min(selected, count - 1) };
    case 'overlay:cancel':
      return { type: 'cancel' };
    default:
      return { type: 'passthrough' };
  }
}
