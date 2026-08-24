/**
 * Decision table for the permission dialog. Options are fixed:
 * index 0 = Allow, index 1 = Deny. esc always denies — a permission the
 * user dismisses is a permission not granted.
 */
export type DialogDecision =
  | { type: 'allow' }
  | { type: 'deny' }
  | { type: 'move'; index: 0 | 1 }
  | { type: 'passthrough' };

export function dialogDecision(action: string | null, selected: number): DialogDecision {
  if (action === null) return { type: 'passthrough' };
  switch (action) {
    case 'dialog:allow':
      return { type: 'allow' };
    case 'dialog:deny':
    case 'dialog:cancel':
      return { type: 'deny' };
    case 'dialog:left':
      return { type: 'move', index: 0 };
    case 'dialog:right':
      return { type: 'move', index: 1 };
    case 'dialog:commit':
      return selected === 0 ? { type: 'allow' } : { type: 'deny' };
    default:
      return { type: 'passthrough' };
  }
}
