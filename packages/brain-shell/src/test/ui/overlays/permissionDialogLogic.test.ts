import { describe, expect, test } from 'bun:test';
import { dialogDecision } from '../../../ui/overlays/permissionDialogLogic.js';

describe('dialogDecision', () => {
  test('direct keys decide', () => {
    expect(dialogDecision('dialog:allow', 1)).toEqual({ type: 'allow' });
    expect(dialogDecision('dialog:deny', 0)).toEqual({ type: 'deny' });
    expect(dialogDecision('dialog:cancel', 0)).toEqual({ type: 'deny' }); // esc denies
  });

  test('arrows move within [Allow, Deny]; enter confirms selection', () => {
    expect(dialogDecision('dialog:left', 1)).toEqual({ type: 'move', index: 0 });
    expect(dialogDecision('dialog:left', 0)).toEqual({ type: 'move', index: 0 });
    expect(dialogDecision('dialog:right', 0)).toEqual({ type: 'move', index: 1 });
    expect(dialogDecision('dialog:right', 1)).toEqual({ type: 'move', index: 1 });
    expect(dialogDecision('dialog:commit', 0)).toEqual({ type: 'allow' });
    expect(dialogDecision('dialog:commit', 1)).toEqual({ type: 'deny' });
  });

  test('null and unrelated actions pass through', () => {
    expect(dialogDecision(null, 0)).toEqual({ type: 'passthrough' });
    expect(dialogDecision('overlay:up', 0)).toEqual({ type: 'passthrough' });
  });
});
