import type { TypewriterQueue } from '../../contracts/streaming.js';

/** Drain cadence used by SessionController's interval. */
export const TYPEWRITER_TICK_MS = 16;
export const TYPEWRITER_CHARS_PER_TICK = 32;

/** FIFO char-buffer queue implementing the contracts/streaming.ts seam. */
export class TwoStageTypewriterQueue implements TypewriterQueue {
  private buffer = '';

  get pending(): number {
    return this.buffer.length;
  }

  push(text: string): void {
    if (text.length > 0) this.buffer += text;
  }

  end(): void {
    // Completion is tracked by the caller (turn loop); the queue only holds text.
  }

  drain(maxChars: number): string {
    if (maxChars <= 0 || this.buffer.length === 0) return '';
    const out = this.buffer.slice(0, maxChars);
    this.buffer = this.buffer.slice(maxChars);
    return out;
  }
}
