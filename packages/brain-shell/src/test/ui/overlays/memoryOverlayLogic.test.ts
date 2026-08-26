import { describe, expect, test } from 'bun:test';
import { scorePercent, clampSelection } from '../../../ui/overlays/memoryOverlayLogic.js';

describe('memoryOverlayLogic (Inc 21)', () => {
  test('score clamps to 0..100 and rounds', () => {
    expect(scorePercent(99.4)).toBe(99);
    expect(scorePercent(-5)).toBe(0);
    expect(scorePercent(250)).toBe(100);
  });

  test('selection clamps into range, empty-safe', () => {
    expect(clampSelection(7, 3)).toBe(2);
    expect(clampSelection(1, 0)).toBe(0);
    expect(clampSelection(0, 5)).toBe(0);
  });
});
