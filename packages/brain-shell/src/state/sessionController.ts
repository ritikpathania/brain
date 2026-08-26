/**
 * Turn-loop owner for the shell. Non-React class exposing an immutable
 * ShellSnapshot through subscribe/getSnapshot (useSyncExternalStore seam).
 * UI actions enter here; socket access stays behind the client seam.
 */
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainSessionSummary,
  BrainStreamChunk,
  MemorySearchResult,
} from '../client/BrainBackendClient.js';
import { sessionToRows } from './sessionReplay.js';
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
import {
  ConnectionMonitor,
  type ConnectionState,
} from './connectionMonitor.js';
import { probeDaemonSocket } from '../client/probeDaemonSocket.js';
import {
  addAllowRule,
  matchingRuleIndex,
  primaryInputString,
  readAllowRules,
} from './permissionRules.js';
import { BrainTurnTransformer } from '../adapter/BrainTurnTransformer.js';
import type { BrainTurnEvent } from '../adapter/BrainTurnEvents.js';
import { turnToRows } from '../ui/transcript/toRows.js';

export interface PendingPermissionView {
  callId: string;
  toolName: string;
  input: Record<string, unknown>;
  reason?: string;
}

export interface ShellSnapshot {
  rows: TranscriptRow[];
  live: LiveStreamView;
  busy: boolean;
  connectionError?: string;
  connection: ConnectionState;
  permission?: PendingPermissionView;
}

const IDLE_LIVE: LiveStreamView = { phase: 'idle', thinkingText: '', responseText: '' };
const CONNECTION_RE = /Could not connect|socket error|disconnected/i;
const CONNECTION_LOSS_RE =
  /Could not connect|socket not found|socket error|disconnected|connection closed|RPC timeout/i;
const ABORT_RE = /abort/i;

/** Inc 15: a classified connection loss arms the monitor; aborts never do. */
function isConnectionLoss(text: string): boolean {
  return !ABORT_RE.test(text) && CONNECTION_LOSS_RE.test(text);
}

export const CONNECTION_LOSS_ROW = 'Connection lost — reconnecting…';
export const QUEUED_ROW = 'queued — will send on reconnect';

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
  private thinkingStartedAt: number | null = null;
  private turnSeq = 0;
  private connection: ConnectionState = { status: 'connected' };
  private queuedInputs: string[] = [];
  private monitor: ConnectionMonitor | null = null;
  private lostDuringTurn = false;
  private snapshot: ShellSnapshot = {
    rows: [],
    live: IDLE_LIVE,
    busy: false,
    connection: { status: 'connected' },
  };

  constructor(
    private client: BrainBackendClient,
    private probeOverride?: () => Promise<boolean>,
    private delayOverride?: (ms: number) => Promise<void>,
  ) {}

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  getSnapshot = (): ShellSnapshot => this.snapshot;

  /** B5: adopted session id, for the resume picker's current-session marker. */
  get activeSessionId(): string | undefined {
    return this.sessionId;
  }

  abort(): void {
    this.aborter?.abort();
  }

  /** Resolve the parked dialog manually. Local UX settles first; the wire
   * verdict rides v1/tool/resolve best-effort — legacy fakes and offline
   * daemons simply omit or reject the call, while live daemons gate
   * execution on it. */
  resolvePermission(callId: string, granted: boolean): void {
    if (this.pendingPermission?.callId !== callId) return;
    const toolName = this.pendingPermission.toolName;
    this.pendingPermission = undefined;
    if (!granted) {
      this.rows = this.rows.map((r) =>
        r.kind === 'tool' && r.tool.callId === callId
          ? { ...r, tool: { ...r.tool, status: 'denied' as const } }
          : r,
      );
    }
    // Best-effort wire round-trip; local UX above is already settled.
    void this.client.resolveToolPermission?.(callId, granted)?.catch(() => {});
    this.notice(`${granted ? 'Allowed' : 'Denied'} ${toolName}`);
  }

  /** Inc 17: grant AND persist an always-allow rule derived from the pending
   * view. A save failure only skips persistence — this call is still granted. */
  resolvePermissionAlways(callId: string): void {
    const view = this.pendingPermission;
    if (view?.callId !== callId) return;
    try {
      addAllowRule({
        tool: view.toolName,
        inputPrefix: primaryInputString(view.input),
      });
    } catch {
      this.notice('Could not save the always-allow rule.');
    }
    this.resolvePermission(callId, true);
  }

  /** Saved-rule hit: never park the dialog. If the wire verdict cannot be
   * delivered, re-park so the user decides manually — a silent drop would
   * leave the daemon's waiter parked until its deny-by-default timeout. */
  private autoAllow(view: PendingPermissionView, ruleNumber: number): void {
    this.notice(`Allowed ${view.toolName} (rule ${ruleNumber})`);
    const park = (): void => {
      this.pendingPermission = view;
      this.emit();
    };
    try {
      const p = this.client.resolveToolPermission?.(view.callId, true);
      if (p) p.catch(park);
    } catch {
      park();
    }
  }

  /** Inc 15: stop the reconnect loop (AppShell cleanup effect). */
  dispose(): void {
    this.monitor?.stop();
  }

  private ensureMonitor(): ConnectionMonitor {
    if (this.monitor === null) {
      const socketPath = (this.client as { socketPath?: string }).socketPath;
      const probe =
        this.probeOverride ??
        (typeof socketPath === 'string'
          ? () => probeDaemonSocket(socketPath)
          : async () => true); // fakes without a transport restore immediately
      const delay =
        this.delayOverride ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
      this.monitor = new ConnectionMonitor({
        probe,
        delay,
        onChange: (s) => {
          this.connection = s;
          this.emit();
        },
        onRestored: () => {
          void this.drainQueue();
        },
      });
    }
    return this.monitor;
  }

  /** Classified loss anywhere: show the banner, arm the loop once. */
  private handleConnectionLoss(): void {
    if (this.connection.status === 'reconnecting') return;
    if (this.busy) this.lostDuringTurn = true;
    this.ensureMonitor().start();
  }

  private async drainQueue(): Promise<void> {
    while (this.queuedInputs.length > 0 && this.connection.status === 'connected') {
      await this.submit(this.queuedInputs[0]);
      // A second outage during this submit leaves the item queued.
      if (this.connection.status === 'connected') this.queuedInputs.shift();
    }
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

  /** Prior sessions for the /resume picker. */
  async listSessions(): Promise<BrainSessionSummary[]> {
    return this.client.listSessions();
  }

  /** /memory data source. Liveness-discriminated so views can render
   * offline copy vs empty copy; every transport failure collapses to ok:false. */
  async searchMemories(query: string, limit = 20): Promise<MemorySearchResult> {
    try {
      const res = await this.client.searchMemory({ query, limit });
      return { ok: true, memories: res.memories };
    } catch {
      return { ok: false };
    }
  }

  /** Adopt a stored session and replay its messages as frozen rows. */
  async resumeSession(sessionId: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    try {
      const { session } = await this.client.loadSession(sessionId);
      this.sessionId = session.id;
      const replayed = sessionToRows(session);
      this.rows = [
        ...replayed,
        { kind: 'system', id: `sys:${++this.sysSeq}`, text: `Resumed “${session.title}”` },
      ];
      this.emit();
    } catch (e) {
      this.notice(`Could not resume session: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  /** Inc 11: `!` bash passthrough — a standalone turn that never reaches a
   * provider. Rendered through the same reducer/projection path as live
   * agentic cards, so local and replayed rows agree by construction. */
  async runShellCommand(command: string): Promise<void> {
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    const trimmed = command.trim();
    if (trimmed.length === 0) return;
    this.busy = true;
    this.connectionError = undefined;
    const turnId = `turn_${++this.turnSeq}`;
    this.rows = [...this.rows, { kind: 'user', id: `user:${turnId}`, text: `! ${trimmed}` }];
    this.aborter = new AbortController();
    this.emit();
    try {
      if (this.sessionId === undefined) {
        this.sessionId = (await this.client.createSession()).sessionId;
      }
      const result = await this.client.execShell?.(this.sessionId, trimmed, this.aborter.signal);
      if (!result) {
        this.notice('This backend cannot execute shell commands.');
        return;
      }
      const callId = result.callId || `shell_${turnId}`;
      const vm = BrainTurnTransformer.transform([
        { type: 'tool_call_requested', callId, toolName: 'bash', input: result.input },
        {
          type: 'tool_result',
          callId,
          output: result.output,
          isError: result.isError,
          exitCode: result.exitCode,
          durationMs: result.durationMs,
        },
      ]);
      const projected = turnToRows(vm).filter(
        (r) => !(r.kind === 'assistant' && r.markdown.trim().length === 0),
      );
      this.rows = [...this.rows, ...projected];
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.notice(/abort/i.test(msg) ? 'Shell command cancelled.' : `Could not run command: ${msg}`);
    } finally {
      this.busy = false;
      this.aborter = null;
      this.emit();
    }
  }

  async submit(text: string): Promise<void> {
    // Inc 14: a submit during a live turn gets the same feedback as every
    // other busy-path entry point instead of vanishing.
    if (this.busy) {
      this.notice('Busy — wait for the current turn to finish.');
      return;
    }
    // Inc 15: offline submits join the replay queue instead of failing.
    if (this.connection.status !== 'connected') {
      this.queuedInputs.push(text);
      this.notice(QUEUED_ROW);
      return;
    }
    this.busy = true;
    this.connectionError = undefined;
    this.lostDuringTurn = false;
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
      this.finishTurn(
        this.sawError ? 'error' : 'completed',
        this.lostDuringTurn ? CONNECTION_LOSS_ROW : undefined,
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (isConnectionLoss(msg)) this.handleConnectionLoss();
      this.finishTurn('error', isConnectionLoss(msg) ? CONNECTION_LOSS_ROW : msg);
    }
  }

  private pendingPermission: PendingPermissionView | undefined;

  private handleChunk(chunk: BrainStreamChunk): void {
    if (chunk.type === 'error' && chunk.error) {
      if (CONNECTION_RE.test(chunk.error)) {
        this.connectionError = chunk.error;
      }
      if (isConnectionLoss(chunk.error)) {
        this.handleConnectionLoss();
      }
    }
    // Permission requests either auto-allow from a saved rule (Inc 17) or
    // park a dialog on the snapshot; both paths end in a wire verdict via
    // v1/tool/resolve, which the daemon parks the stream awaiting.
    if (chunk.type === 'permission_request' && typeof chunk.callId === 'string') {
      const view: PendingPermissionView = {
        callId: chunk.callId,
        toolName: chunk.toolName ?? 'tool',
        input: chunk.input ?? {},
        reason: chunk.reason,
      };
      const ruleNumber =
        matchingRuleIndex(view.toolName, view.input, readAllowRules()) + 1;
      if (ruleNumber > 0) this.autoAllow(view, ruleNumber);
      else {
        this.pendingPermission = view;
        this.emit();
      }
      return;
    }
    const event = chunkToTurnEvent(chunk);
    if (event === null) return;
    // Inc 13: thinking lifecycle. The wire's daemon-measured duration wins;
    // a local bracket covers daemons that omit it.
    if (event.type === 'thinking_start') {
      this.thinkingStartedAt = Date.now();
      this.live = { ...this.live, phase: 'thinking' };
    } else if (event.type === 'thinking_end') {
      if (event.durationMs === undefined && this.thinkingStartedAt !== null) {
        event.durationMs = Math.max(0, Date.now() - this.thinkingStartedAt);
      }
      this.thinkingStartedAt = null;
    }
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
    // Frozen rows are built solely from `events`, which already holds every
    // delta verbatim (handleChunk records them before queueing for pacing).
    // The typewriter queue feeds only the live view, which freeze discards —
    // flushing it here would re-push its pending tail (Inc 16 dedup fix).
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
      connection: this.connection,
      permission: this.pendingPermission,
    };
    for (const fn of this.listeners) fn();
  }
}
