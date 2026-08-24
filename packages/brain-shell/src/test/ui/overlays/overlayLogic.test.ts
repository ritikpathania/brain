import { describe, expect, test } from 'bun:test';
import { overlayListDecision } from '../../../ui/overlays/overlayLogic.js';

describe('overlayListDecision', () => {
  test('closed or empty lists pass everything through', () => {
    expect(overlayListDecision('overlay:down', 0, 0)).toEqual({ type: 'passthrough' });
    expect(overlayListDecision(null, 0, 5)).toEqual({ type: 'passthrough' });
  });

  test('arrows clamp within bounds', () => {
    expect(overlayListDecision('overlay:up', 0, 3)).toEqual({ type: 'move', index: 0 });
    expect(overlayListDecision('overlay:up', 2, 3)).toEqual({ type: 'move', index: 1 });
    expect(overlayListDecision('overlay:down', 2, 3)).toEqual({ type: 'move', index: 2 });
    expect(overlayListDecision('overlay:down', 0, 3)).toEqual({ type: 'move', index: 1 });
  });

  test('commit carries the selected index and cancel cancels', () => {
    expect(overlayListDecision('overlay:commit', 1, 3)).toEqual({ type: 'commit', index: 1 });
    expect(overlayListDecision('overlay:cancel', 1, 3)).toEqual({ type: 'cancel' });
  });

  test('unrelated actions pass through', () => {
    expect(overlayListDecision('dialog:allow', 0, 2)).toEqual({ type: 'passthrough' });
  });
});
