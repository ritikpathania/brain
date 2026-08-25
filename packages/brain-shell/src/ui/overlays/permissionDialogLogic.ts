/**
 * Decision table for the permission dialog. Options are fixed:
 * index 0 = Allow, index 1 = Deny, index 2 = Always allow (grant and
 * persist a rule, Inc 17). Arrows move relatively and clamp; esc always
 * denies — a permission the user dismisses is a permission not granted.
 */
export type DialogDecision =
  | { type: 'allow' }
  | { type: 'deny' }
  | { type: 'always' }
  | { type: 'move'; index: 0 | 1 | 2 }
  | { type: 'passthrough' };

export function dialogDecision(action: string | null, selected: number): DialogDecision {
  if (action === null) return { type: 'passthrough' };
  switch (action) {
    case 'dialog:allow':
      return { type: 'allow' };
    case 'dialog:always':
      return { type: 'always' };
    case 'dialog:deny':
    case 'dialog:cancel':
      return { type: 'deny' };
    case 'dialog:left':
      return { type: 'move', index: Math.max(0, selected - 1) as 0 | 1 | 2 };
    case 'dialog:right':
      return { type: 'move', index: Math.min(2, selected + 1) as 0 | 1 | 2 };
    case 'dialog:commit':
      return selected === 0 ? { type: 'allow' } : selected === 2 ? { type: 'always' } : { type: 'deny' };
    default:
      return { type: 'passthrough' };
  }
}
