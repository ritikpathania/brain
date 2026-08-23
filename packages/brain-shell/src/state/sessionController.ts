/**
 * Turn-loop owner for the shell. Non-React class exposing an immutable
 * ShellSnapshot through subscribe/getSnapshot (useSyncExternalStore seam).
 * UI actions enter here; socket access stays behind the client seam.
 */
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainStreamChunk,
} from '../client/BrainBackendClient.js';
import { normalizeMessagesForBrain } from '../adapter/brainCallModel.js';
import { createUserMessage } from '../contracts/messages.js';
import type { TranscriptRow } from '../contracts/messages.js';
import type { LiveStreamView } from '../contracts/streaming.js';
import {
  TwoStageTypewriterQueue,
  TYPEWRITER_TICK_MS,
  TYPEWRITER_CHARS_PER_TICK,
} from '../adapter/streaming/TwoStageTypewriterQueue.js';
import { chunkToTurnEvent } from '../adapter/chunkToTurnEvents.js';
import { BrainTurnTransformer } from '../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../adapter/BrainTurnEvents.js';
import { turnToRows } from '../ui/transcript/toRows.js';

export interface ShellSnapshot {
  rows: TranscriptRow[];
  live: LiveStreamView;
  busy: boolean;
  connectionError?: string;
}

const IDLE_LIVE: LiveStreamView = { phase: 'idle', thinkingText: '', responseText: '' };
const CONNECTION_RE = /Could not connect|socket error|disconnected/i;

export class SessionController {
  private listeners = new Set<() => void>();
  private rows: TranscriptRow[] = [];
  private live: LiveStreamView = IDLE_LIVE;
  private busy = false;
  private connectionError: string | undefined;
  private sessionId: string | undefined;
  private aborter: AbortController | null = null;
  private queue = new TwoStageTypewriterQueue();
  private ticker: ReturnType<typeof setInterval> | null = null;
  private events: BrainTurnEvent[] = [];
  private sawError = false;
  private turnSeq = 0;
  private snapshot: ShellSnapshot = { rows: [], live: IDLE_LIVE, busy: false };

  constructor(private client: BrainBackendClient) {}

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  getSnapshot = (): ShellSnapshot => this.snapshot;

  abort(): void {
    this.aborter?.abort();
  }

  /** Wipe the frozen transcript (slash command /clear). */
  clear(): void {
    this.rows = [];
    this.emit();
  }

  private sysSeq = 0;

  /** Locally-generated shell output (/help, warnings). Never hits the wire. */
  notice(text: string): void {
    this.rows = [...this.rows, { kind: 'system', id: `sys:${++this.sysSeq}`, text }];
    this.emit();
  }

  async submit(text: string): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.connectionError = undefined;
    const turnId = `turn_${++this.turnSeq}`;
    this.rows = [...this.rows, { kind: 'user', id: `user:${turnId}`, text }];
    this.events = [{ type: 'turn_start', turnId, role: 'assistant' }];
    this.sawError = false;
    this.queue = new TwoStageTypewriterQueue();
    this.live = { phase: 'responding', thinkingText: '', responseText: '' };
    this.aborter = new AbortController();
    this.emit();
    this.startTicker();
    try {
      if (this.sessionId === undefined) {
        this.sessionId = (await this.client.createSession()).sessionId;
      }
      const request: BrainGenerationRequest = {
        sessionId: this.sessionId,
        messages: normalizeMessagesForBrain([createUserMessage(text)]),
        signal: this.aborter.signal,
      };
      for await (const chunk of this.client.streamText(request)) {
        this.handleChunk(chunk);
      }
      // An error chunk doesn't throw — the stream just ends — but the turn
      // still failed and must settle its tools accordingly.
      this.finishTurn(this.sawError ? 'error' : 'completed');
    } catch (err) {
      this.finishTurn('error', err instanceof Error ? err.message : String(err));
    }
  }

  private handleChunk(chunk: BrainStreamChunk): void {
    if (chunk.type === 'error' && chunk.error && CONNECTION_RE.test(chunk.error)) {
      this.connectionError = chunk.error;
    }
    const event = chunkToTurnEvent(chunk);
    if (event === null) return;
    this.events.push(event);
    if (event.type === 'text_delta') {
      this.queue.push(event.delta);
      // First response text ends the thinking phase visually.
      if (this.live.phase === 'thinking' || this.live.phase === 'responding') {
        this.live = { ...this.live, phase: 'responding' };
      }
    } else if (event.type === 'thinking_delta') {
      this.live = {
        ...this.live,
        phase: 'thinking',
        thinkingText: this.live.thinkingText + event.delta,
      };
    } else if (event.type === 'tool_call_requested') {
      this.live = { ...this.live, phase: 'tool', activeToolName: event.toolName };
    } else if (event.type === 'turn_error') {
      this.sawError = true;
      this.live = { ...this.live, phase: 'error', errorText: event.error };
    }
    this.emit();
  }

  private startTicker(): void {
    this.ticker = setInterval(() => {
      if (this.queue.pending === 0) return;
      const out = this.queue.drain(TYPEWRITER_CHARS_PER_TICK);
      this.live = { ...this.live, responseText: this.live.responseText + out };
      this.emit();
    }, TYPEWRITER_TICK_MS);
  }

  private stopTicker(): void {
    if (this.ticker !== null) {
      clearInterval(this.ticker);
      this.ticker = null;
    }
  }

  private finishTurn(status: 'completed' | 'error', errorText?: string): void {
    this.stopTicker();
    // Flush any undrained typewriter text so frozen rows carry the whole answer.
    const remainder = this.queue.pending > 0 ? this.queue.drain(this.queue.pending) : '';
    if (remainder.length > 0) {
      this.events.push({ type: 'text_delta', delta: remainder });
    }
    // The wire protocol has no tool-result frame yet, so requested calls
    // would otherwise stay 'pending' forever on the frozen card. Settle them:
    // completed turns finish their tools, errored turns cancel them.
    const settled = new Set<string>();
    for (const e of this.events) {
      if (e.type === 'tool_result' || e.type === 'tool_cancelled') settled.add(e.callId);
    }
    for (const e of [...this.events]) {
      if (e.type === 'tool_call_requested' && !settled.has(e.callId)) {
        settled.add(e.callId);
        this.events.push(
          status === 'error'
            ? { type: 'tool_cancelled', callId: e.callId, reason: 'turn ended' }
            : { type: 'tool_result', callId: e.callId, output: '' },
        );
      }
    }
    if (status === 'error' && errorText !== undefined) {
      this.events.push({ type: 'turn_error', error: errorText });
    }
    this.events.push({ type: 'thinking_end' });
    this.events.push({ type: 'turn_complete' });
    try {
      const vm = BrainTurnTransformer.transform(this.events);
      const projected = turnToRows(vm).filter(
        (r) => !(r.kind === 'assistant' && r.markdown.trim().length === 0),
      );
      this.rows = [...this.rows, ...projected];
    } catch {
      // Transformer mismatch must never kill the shell; keep prior rows.
    }
    this.live = IDLE_LIVE;
    this.busy = false;
    this.aborter = null;
    this.emit();
  }

  private emit(): void {
    this.snapshot = {
      rows: this.rows,
      live: this.live,
      busy: this.busy,
      connectionError: this.connectionError,
    };
    for (const fn of this.listeners) fn();
  }
}
