/**
 * Stream presentation contract: live-region view-model shape and the
 * typewriter queue seam that decouples network completion from drain cadence.
 * Dependency-free (no transport, no React, no vendor).
 */

export type StreamPhase = 'idle' | 'thinking' | 'responding' | 'tool' | 'error';

export interface LiveStreamView {
  phase: StreamPhase;
  /** Accumulated thinking text for the active turn (dim preview above the response). */
  thinkingText: string;
  /** Typewriter-released response text for the active turn. */
  responseText: string;
  /** Tool name while phase === 'tool'. */
  activeToolName?: string;
  errorText?: string;
}

/** Two-stage typewriter queue: stage 1 buffers network deltas; stage 2 drains on cadence. */
export interface TypewriterQueue {
  push(text: string): void;
  /** Network side signals completion; does not itself release anything. */
  end(): void;
  /** Release at most maxChars buffered characters; '' when nothing pending. */
  drain(maxChars: number): string;
  readonly pending: number;
}
