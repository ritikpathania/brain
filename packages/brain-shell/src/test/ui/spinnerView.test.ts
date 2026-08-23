import { describe, it, expect } from 'bun:test';
import { spinnerFrameAt, spinnerLabel, spinnerFrames } from '../../ui/shell/Spinner.js';
import type { LiveStreamView } from '../../contracts/streaming.js';

const live = (patch: Partial<LiveStreamView>): LiveStreamView => ({
  phase: 'responding',
  thinkingText: '',
  responseText: '',
  ...patch,
});

describe('SpinnerView math', () => {
  it('cycles palindrome frames at 120ms', () => {
    expect(spinnerFrameAt(0)).toBe(spinnerFrames[0]);
    expect(spinnerFrameAt(119)).toBe(spinnerFrames[0]);
    expect(spinnerFrameAt(120)).toBe(spinnerFrames[1]);
    expect(spinnerFrameAt(120 * spinnerFrames.length)).toBe(spinnerFrames[0]); // wraps
    // Palindrome bounce: last frame mirrors frame[1].
    expect(spinnerFrames[spinnerFrames.length - 1]).toBe(spinnerFrames[1]);
  });

  it('labels phases including the active tool name', () => {
    expect(spinnerLabel(live({}))).toBe('Composing…');
    expect(spinnerLabel(live({ phase: 'thinking' }))).toBe('Thinking…');
    expect(spinnerLabel(live({ phase: 'tool', activeToolName: 'read_file' }))).toBe('read_file…');
    expect(spinnerLabel(live({ phase: 'error', errorText: 'x' }))).toBe('Failed');
    expect(spinnerLabel(live({ phase: 'idle' }))).toBe('');
  });
});
