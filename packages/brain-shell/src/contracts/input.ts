/**
 * Brain-owned input/composer vocabulary. Inc 0 carries only what current
 * consumers need; the vendor-era surface (ghost text, queue priorities,
 * paste tracking) is re-derived in the composer increment.
 */
export type PromptInputMode = 'prompt' | 'bash';
export type VimMode = 'INSERT' | 'NORMAL';

export interface VimInputState {
  mode: VimMode;
  pending?: string;
}

/** Thinking-budget configuration; canonical home is contracts/tools.ts. */
export type { ThinkingConfig } from './tools.js';
