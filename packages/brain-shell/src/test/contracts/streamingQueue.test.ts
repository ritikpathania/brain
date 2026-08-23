import { describe, it, expect } from 'bun:test';
import { TwoStageTypewriterQueue } from '../../adapter/streaming/TwoStageTypewriterQueue.js';

describe('TwoStageTypewriterQueue', () => {
  it('buffers pushes and releases up to maxChars per drain', () => {
    const q = new TwoStageTypewriterQueue();
    q.push('hello ');
    q.push('world');
    expect(q.pending).toBe(11);
    expect(q.drain(6)).toBe('hello ');
    expect(q.pending).toBe(5);
    expect(q.drain(100)).toBe('world');
    expect(q.pending).toBe(0);
    expect(q.drain(10)).toBe('');
  });

  it('end() does not auto-release but marks completion; drains still work after end()', () => {
    const q = new TwoStageTypewriterQueue();
    q.push('abc');
    q.end();
    expect(q.pending).toBe(3); // completion is decoupled from drain
    expect(q.drain(3)).toBe('abc');
    expect(q.pending).toBe(0);
  });

  it('handles multibyte-safe slicing by code units (ASCII contract)', () => {
    // Drain operates on UTF-16 code units; emoji may split across drains.
    // Contract: callers treat released text as opaque concatenation.
    const q = new TwoStageTypewriterQueue();
    q.push('a✻b');
    expect(q.drain(2) + q.drain(2)).toBe('a✻b');
  });
});
