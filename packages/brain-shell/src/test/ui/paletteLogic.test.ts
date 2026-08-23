import { describe, it, expect } from 'bun:test';
import {
  paletteWindow,
  paletteKeyDecision,
  PALETTE_MAX_ITEMS,
} from '../../ui/composer/paletteLogic.js';

describe('paletteWindow', () => {
  it('keeps all items visible when under the cap', () => {
    expect(paletteWindow(3, 0)).toEqual({ start: 0, end: 3 });
    expect(paletteWindow(PALETTE_MAX_ITEMS, PALETTE_MAX_ITEMS - 1)).toEqual({
      start: 0,
      end: PALETTE_MAX_ITEMS,
    });
  });
  it('scrolls to keep the selection inside the window', () => {
    expect(paletteWindow(9, 0)).toEqual({ start: 0, end: 5 });
    expect(paletteWindow(9, 4)).toEqual({ start: 0, end: 5 });
    expect(paletteWindow(9, 5)).toEqual({ start: 1, end: 6 });
    expect(paletteWindow(9, 8)).toEqual({ start: 4, end: 9 });
  });
});

describe('paletteKeyDecision', () => {
  const decide = (cmdType: string, extra?: Partial<Parameters<typeof paletteKeyDecision>[0]>) =>
    paletteKeyDecision({ open: true, cmdType, tab: false, selected: 0, count: 3, ...extra });

  it('moves within bounds without wrapping while open', () => {
    expect(decide('history_up')).toEqual({ kind: 'move', next: 0 }); // clamped at top
    expect(decide('history_up', { selected: 1 })).toEqual({ kind: 'move', next: 0 });
    expect(decide('history_down')).toEqual({ kind: 'move', next: 1 });
    expect(decide('history_down', { selected: 2 })).toEqual({ kind: 'move', next: 2 }); // clamped at bottom
  });

  it('completes on tab and closes on escape while open', () => {
    expect(decide('noop', { tab: true })).toEqual({ kind: 'complete', index: 0 });
    expect(decide('noop', { tab: true, selected: 2 })).toEqual({ kind: 'complete', index: 2 });
    expect(decide('abort')).toEqual({ kind: 'close' });
  });

  it('passes submit and editing keys through while open', () => {
    expect(decide('submit')).toEqual({ kind: 'passthrough' }); // enter runs the command
    expect(decide('insert')).toEqual({ kind: 'passthrough' });
    expect(decide('backspace')).toEqual({ kind: 'passthrough' });
    expect(decide('exit')).toEqual({ kind: 'passthrough' });   // ctrl+c still exits
  });

  it('passes everything through while closed', () => {
    const closed = (cmdType: string, extra?: Partial<Parameters<typeof paletteKeyDecision>[0]>) =>
      paletteKeyDecision({ open: false, cmdType, tab: false, selected: 0, count: 0, ...extra });
    expect(closed('history_up')).toEqual({ kind: 'passthrough' });
    expect(closed('abort')).toEqual({ kind: 'passthrough' }); // esc aborts the turn elsewhere
    expect(closed('noop', { tab: true })).toEqual({ kind: 'passthrough' });
  });
});
